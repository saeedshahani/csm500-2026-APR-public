//! Finite immutable carrier records and structural rule validation.
//!
//! `Graph` is a construction buffer: insertion checks shape and endpoints;
//! `validate` checks quotation and complete rule representations before execution.
//! Atom identities are scoped to one graph history. Clones preserve that history.

pub mod fixture;
mod validation;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_exhaustion_rejects_without_wraparound() {
        let mut graph = Graph {
            next_identity: u64::MAX,
            ..Graph::default()
        };
        let before = graph.snapshot();
        assert_eq!(
            graph.insert(Kind::Nat, &[]).unwrap_err().0,
            "atom identity space exhausted"
        );
        assert_eq!(graph.snapshot(), before);
        assert_eq!(graph.next_identity, u64::MAX);
    }
}

use slotmap::{new_key_type, SlotMap};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
pub use validation::{validate, Configuration, RuleView};

/// Never reused in a committed graph history; allocation fails on exhaustion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomKey(u64);

impl AtomKey {
    /// Runtime identity for diagnostics; not an isomorphism-invariant graph label.
    pub fn number(self) -> u64 {
        self.0
    }
}

new_key_type! { struct StorageKey; }

/// The eight semantic record roles; fields are ordered by their vector positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    ObjectLink,
    Lhs,
    Rhs,
    Active,
    Rank,
    ModeObject,
    ModeMeta,
    Revision,
}

impl Role {
    pub fn membership(self) -> bool {
        matches!(self, Self::Lhs | Self::Rhs)
    }
    pub fn arity(self) -> usize {
        if matches!(self, Self::ObjectLink | Self::Lhs | Self::Rhs | Self::Rank) {
            2
        } else {
            1
        }
    }
}

/// Refined kinds admitted by an operational pattern variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sort {
    Constructor,
    PatternVariable,
    RuleHead,
    Nat,
    Record(Role),
}

/// A free assignment or an assignment constrained to the applied rule's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Free,
    AppliedRule,
}

/// Immutable semantic attributes of an atom, independent of its runtime identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Constructor(String),
    PatternVariable {
        admits: BTreeSet<Sort>,
        binding: Binding,
    },
    RuleHead,
    Nat,
    Record {
        role: Role,
        label: Option<String>,
    },
}

impl Kind {
    /// Construct an unlabelled metadata record. Object links require an explicit label.
    pub fn record(role: Role) -> Self {
        Self::Record { role, label: None }
    }
    pub fn variable(sort: Sort, binding: Binding) -> Self {
        Self::PatternVariable {
            admits: BTreeSet::from([sort]),
            binding,
        }
    }
    pub fn sort(&self) -> Sort {
        match self {
            Self::Constructor(_) => Sort::Constructor,
            Self::PatternVariable { .. } => Sort::PatternVariable,
            Self::RuleHead => Sort::RuleHead,
            Self::Nat => Sort::Nat,
            Self::Record { role, .. } => Sort::Record(*role),
        }
    }
    pub fn role(&self) -> Option<Role> {
        if let Self::Record { role, .. } = self {
            Some(*role)
        } else {
            None
        }
    }
}

/// A stored value. Graph lookup exposes it immutably; cloning does not mutate storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    pub kind: Kind,
    pub fields: Vec<AtomKey>,
}

/// A batch permits fresh records to refer to other fresh records, including cycles.
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Existing(AtomKey),
    Fresh(usize),
}

/// One allocation request in an atomic batch; fresh targets index that same batch.
#[derive(Clone, Debug)]
pub struct NewAtom {
    pub kind: Kind,
    pub fields: Vec<Target>,
}

/// First failure encountered by construction or validation, suitable for display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic(pub String);
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Diagnostic {}
pub type Result<T> = std::result::Result<T, Diagnostic>;

pub fn ensure(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Diagnostic(message.into()))
    }
}

/// Finite construction buffer with immutable stored records and checked identities.
///
/// Insertion checks shape and resolved endpoints, not complete rule well-formedness.
/// Call [`validate`] before execution; the engine does so for every pre/post-state.
/// Clones preserve history but must not be merged by treating equal keys as global IDs.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    atoms: SlotMap<StorageKey, Atom>,
    identities: BTreeMap<AtomKey, StorageKey>,
    next_identity: u64,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.identities.len()
    }
    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
    /// Look up an atom; a consumed or unknown key resolves to `None`.
    pub fn get(&self, key: AtomKey) -> Option<&Atom> {
        self.identities
            .get(&key)
            .and_then(|slot| self.atoms.get(*slot))
    }
    pub fn atom(&self, key: AtomKey) -> Result<&Atom> {
        self.get(key)
            .ok_or_else(|| Diagnostic(format!("unresolved atom {key:?}")))
    }
    /// Iterate in increasing runtime identity order, not semantic canonical order.
    pub fn iter(&self) -> impl Iterator<Item = (AtomKey, &Atom)> {
        self.identities
            .iter()
            .map(|(id, slot)| (*id, &self.atoms[*slot]))
    }
    pub fn field_count(&self) -> usize {
        self.iter().map(|(_, a)| a.fields.len()).sum()
    }
    /// Scan for source atoms having at least one field targeting `target`.
    pub fn references_to(&self, target: AtomKey) -> BTreeSet<AtomKey> {
        self.iter()
            .filter(|(_, a)| a.fields.contains(&target))
            .map(|(id, _)| id)
            .collect()
    }
    pub fn of_sort(&self, sort: Sort) -> BTreeSet<AtomKey> {
        self.iter()
            .filter(|(_, a)| a.kind.sort() == sort)
            .map(|(id, _)| id)
            .collect()
    }
    pub fn insert(&mut self, kind: Kind, fields: &[AtomKey]) -> Result<AtomKey> {
        Ok(self.insert_batch(&[NewAtom {
            kind,
            fields: fields.iter().copied().map(Target::Existing).collect(),
        }])?[0])
    }
    /// Shape and reference errors leave both storage and the allocation counter unchanged.
    pub fn insert_batch(&mut self, batch: &[NewAtom]) -> Result<Vec<AtomKey>> {
        let end = self
            .next_identity
            .checked_add(batch.len() as u64)
            .ok_or_else(|| Diagnostic("atom identity space exhausted".into()))?;
        let ids: Vec<_> = (self.next_identity..end).map(AtomKey).collect();
        let mut records = Vec::new();
        for item in batch {
            check_shape(&item.kind, item.fields.len())?;
            let fields = item
                .fields
                .iter()
                .map(|t| match t {
                    Target::Existing(id) => self.atom(*id).map(|_| *id),
                    Target::Fresh(index) => ids
                        .get(*index)
                        .copied()
                        .ok_or_else(|| Diagnostic("invalid fresh reference".into())),
                })
                .collect::<Result<Vec<_>>>()?;
            records.push(Atom {
                kind: item.kind.clone(),
                fields,
            });
        }
        for (id, atom) in ids.iter().zip(records) {
            self.identities.insert(*id, self.atoms.insert(atom));
        }
        self.next_identity = end;
        Ok(ids)
    }
    /// Delete exactly the requested atoms. Incoming surviving references reject the whole deletion.
    pub fn remove_set(&mut self, consumed: &BTreeSet<AtomKey>) -> Result<()> {
        for id in consumed {
            self.atom(*id)?;
        }
        ensure(
            !self.iter().any(|(id, a)| {
                !consumed.contains(&id) && a.fields.iter().any(|t| consumed.contains(t))
            }),
            "dangling reference",
        )?;
        for id in consumed {
            if let Some(slot) = self.identities.remove(id) {
                self.atoms.remove(slot);
            }
        }
        Ok(())
    }
    /// Follow outgoing fields to a finite fixed set, tolerating carrier cycles.
    /// An unknown seed is an error; no atoms are allocated or copied.
    pub fn closure(&self, seeds: &BTreeSet<AtomKey>) -> Result<BTreeSet<AtomKey>> {
        let mut seen = BTreeSet::new();
        let mut todo: Vec<_> = seeds.iter().copied().collect();
        while let Some(id) = todo.pop() {
            if seen.insert(id) {
                todo.extend(self.atom(id)?.fields.iter().copied());
            }
        }
        Ok(seen)
    }
    /// Keep a closed induced subgraph for validating a freshly copied package.
    pub(crate) fn induced(&self, keep: &BTreeSet<AtomKey>) -> Result<Self> {
        ensure(self.closure(keep)? == *keep, "package not record-closed")?;
        let mut result = self.clone();
        let remove: Vec<_> = result
            .identities
            .keys()
            .filter(|id| !keep.contains(id))
            .copied()
            .collect();
        for id in remove {
            if let Some(slot) = result.identities.remove(&id) {
                result.atoms.remove(slot);
            }
        }
        Ok(result)
    }
    /// Identity-sensitive snapshot for transaction/replay tests; not graph isomorphism.
    pub fn snapshot(&self) -> Vec<(AtomKey, Atom)> {
        self.iter().map(|(id, a)| (id, a.clone())).collect()
    }
}

fn check_shape(kind: &Kind, arity: usize) -> Result<()> {
    let valid = match kind {
        Kind::Constructor(_) | Kind::RuleHead => arity == 0,
        Kind::PatternVariable { admits, binding } => {
            arity == 0
                && !admits.is_empty()
                && (*binding != Binding::AppliedRule || *admits == BTreeSet::from([Sort::RuleHead]))
        }
        Kind::Nat => arity <= 1,
        Kind::Record { role, label } => {
            arity == role.arity() && label.is_some() == (*role == Role::ObjectLink)
        }
    };
    ensure(valid, "invalid atom arity, label or variable declaration")
}
