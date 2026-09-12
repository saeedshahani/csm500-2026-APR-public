//! Finite immutable carrier records and structural rule validation.
//!
//! `Graph` is a construction buffer: insertion checks shape and endpoints;
//! `validate` checks quotation and complete rule representations before execution.
//! Atom identities are scoped to one graph history. Clones preserve that history.

pub mod fixture;
mod limits;
mod validation;

pub use limits::{GraphLimits, LimitKind};

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

use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SlotMap};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
pub use validation::{validate, validate_with_limits, Configuration, RuleView, ValidatedGraph};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Sort {
    Constructor,
    PatternVariable,
    RuleHead,
    Nat,
    Record(Role),
}

/// A free assignment or an assignment constrained to the applied rule's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Binding {
    Free,
    AppliedRule,
}

/// Immutable semantic attributes of an atom, independent of its runtime identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// Stable failure category; callers must not parse the display message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    MalformedInput,
    InvalidConfiguration,
    InvalidBinding,
    Dangling,
    Lifecycle,
    Guardedness,
    StaleCandidate,
    ResourceExhaustion,
}

/// Machine-readable context for the first reported failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorContext {
    pub kind: ErrorKind,
    pub atoms: Vec<AtomKey>,
    pub role: Option<Role>,
    pub slot: Option<usize>,
    pub batch_index: Option<usize>,
    pub limit: Option<LimitKind>,
}

/// First failure. Field zero retains the human-readable message for existing clients.
/// Structured consumers use [`Diagnostic::context`] or [`Diagnostic::kind`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic(pub String, ErrorContext);
impl Diagnostic {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self(
            message.into(),
            ErrorContext {
                kind,
                atoms: Vec::new(),
                role: None,
                slot: None,
                batch_index: None,
                limit: None,
            },
        )
    }
    pub fn kind(&self) -> ErrorKind {
        self.1.kind
    }
    pub fn context(&self) -> &ErrorContext {
        &self.1
    }
    pub fn with_kind(mut self, kind: ErrorKind) -> Self {
        self.1.kind = kind;
        self
    }
    pub fn at_atom(mut self, atom: AtomKey) -> Self {
        if !self.1.atoms.contains(&atom) {
            self.1.atoms.push(atom);
        }
        self
    }
    pub fn at_role(mut self, role: Option<Role>) -> Self {
        self.1.role = role;
        self
    }
    pub fn at_slot(mut self, slot: usize) -> Self {
        self.1.slot = Some(slot);
        self
    }
    pub fn at_batch(mut self, index: usize) -> Self {
        self.1.batch_index = Some(index);
        self
    }
    pub fn exhausted(limit: LimitKind, message: impl Into<String>) -> Self {
        let mut error = Self::new(ErrorKind::ResourceExhaustion, message);
        error.1.limit = Some(limit);
        error
    }
}
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
        Err(Diagnostic::new(ErrorKind::InvalidConfiguration, message))
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
    /// Allocator high-water mark, including identities no longer present.
    pub fn next_identity(&self) -> u64 {
        self.next_identity
    }
    /// Resolve a public number only when that atom exists in this graph.
    pub fn key(&self, number: u64) -> Option<AtomKey> {
        let key = AtomKey(number);
        self.identities.contains_key(&key).then_some(key)
    }
    /// Reconstruct a separate, fully validated identity-bearing snapshot. This
    /// does not mutate any existing execution or trust its claimed history.
    pub fn restore_identity_snapshot(
        atoms: &[(u64, Kind, Vec<u64>)],
        next_identity: u64,
        limits: &GraphLimits,
    ) -> Result<Self> {
        if atoms.len() > limits.max_atoms {
            return Err(Diagnostic::exhausted(
                LimitKind::Atoms,
                "snapshot atom limit exceeded",
            ));
        }
        let mut fields = 0usize;
        let mut bytes = 0usize;
        let mut previous = None;
        for (id, kind, targets) in atoms {
            check_shape(kind, targets.len()).map_err(|e| e.at_atom(AtomKey(*id)))?;
            ensure(
                *id < next_identity && previous.is_none_or(|p| p < *id),
                "snapshot identities must be unique, increasing and below allocator",
            )?;
            previous = Some(*id);
            fields = fields
                .checked_add(targets.len())
                .filter(|n| *n <= limits.max_fields)
                .ok_or_else(|| {
                    Diagnostic::exhausted(LimitKind::Fields, "snapshot field limit exceeded")
                })?;
            let n = match kind {
                Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => s.len(),
                _ => 0,
            };
            bytes = bytes
                .checked_add(n)
                .filter(|n| *n <= limits.max_label_bytes)
                .ok_or_else(|| {
                    Diagnostic::exhausted(LimitKind::LabelBytes, "snapshot label limit exceeded")
                })?;
        }
        for (id, _, targets) in atoms {
            for target in targets {
                ensure(
                    atoms
                        .binary_search_by_key(target, |(key, _, _)| *key)
                        .is_ok(),
                    "snapshot field references missing atom",
                )
                .map_err(|e| e.at_atom(AtomKey(*id)))?;
            }
        }
        let mut graph = Self {
            next_identity,
            ..Self::default()
        };
        for (id, kind, fields) in atoms {
            let slot = graph.atoms.insert(Atom {
                kind: kind.clone(),
                fields: fields.iter().map(|id| AtomKey(*id)).collect(),
            });
            graph.identities.insert(AtomKey(*id), slot);
        }
        validate_with_limits(&graph, limits)?;
        Ok(graph)
    }
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
    /// Stored fields cannot be modified through this reference.
    ///
    /// ```compile_fail
    /// use graph_core::{Graph, Kind};
    /// let mut graph = Graph::new();
    /// let id = graph.insert(Kind::Nat, &[]).unwrap();
    /// graph.get(id).unwrap().fields.push(id);
    /// ```
    ///
    /// ```compile_fail
    /// use graph_core::{Graph, Kind};
    /// let mut graph = Graph::new();
    /// let id = graph.insert(Kind::Nat, &[]).unwrap();
    /// graph.get(id).unwrap().kind = Kind::RuleHead;
    /// ```
    pub fn get(&self, key: AtomKey) -> Option<&Atom> {
        self.identities
            .get(&key)
            .and_then(|slot| self.atoms.get(*slot))
    }
    pub fn atom(&self, key: AtomKey) -> Result<&Atom> {
        self.get(key).ok_or_else(|| {
            Diagnostic::new(
                ErrorKind::InvalidConfiguration,
                format!("unresolved atom {key:?}"),
            )
            .at_atom(key)
        })
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
        self.insert_batch_with_limits(batch, &GraphLimits::default())
    }
    /// Resolve the whole batch, including forward/cyclic references, before allocating.
    /// This checks construction shape, not full configuration validity.
    pub fn insert_batch_with_limits(
        &mut self,
        batch: &[NewAtom],
        limits: &GraphLimits,
    ) -> Result<Vec<AtomKey>> {
        limits.check_batch(self, batch)?;
        let count = u64::try_from(batch.len()).map_err(|_| {
            Diagnostic::exhausted(LimitKind::Identities, "atom identity space exhausted")
        })?;
        let end = self.next_identity.checked_add(count).ok_or_else(|| {
            Diagnostic::exhausted(LimitKind::Identities, "atom identity space exhausted")
        })?;
        let ids: Vec<_> = (self.next_identity..end).map(AtomKey).collect();
        let mut records = Vec::new();
        for (index, item) in batch.iter().enumerate() {
            check_shape(&item.kind, item.fields.len())
                .map_err(|e| e.at_batch(index).at_role(item.kind.role()))?;
            let fields = item
                .fields
                .iter()
                .enumerate()
                .map(|(slot, t)| {
                    (match t {
                        Target::Existing(id) => self.atom(*id).map(|_| *id),
                        Target::Fresh(index) => ids.get(*index).copied().ok_or_else(|| {
                            Diagnostic::new(
                                ErrorKind::InvalidConfiguration,
                                "invalid fresh reference",
                            )
                        }),
                    })
                    .map_err(|e| e.at_batch(index).at_role(item.kind.role()).at_slot(slot))
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
        for (id, atom) in self.iter().filter(|(id, _)| !consumed.contains(id)) {
            if let Some((slot, target)) = atom
                .fields
                .iter()
                .enumerate()
                .find(|(_, t)| consumed.contains(t))
            {
                return Err(Diagnostic::new(ErrorKind::Dangling, "dangling reference")
                    .at_atom(id)
                    .at_atom(*target)
                    .at_role(atom.kind.role())
                    .at_slot(slot));
            }
        }
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
