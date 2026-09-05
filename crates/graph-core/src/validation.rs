use crate::{ensure, AtomKey, Binding, Graph, Kind, Result, Role, Sort};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct RuleView {
    pub head: AtomKey,
    pub rank: usize,
    pub revision: AtomKey,
    pub active: bool,
    pub meta: bool,
    pub lhs: BTreeSet<AtomKey>,
    pub rhs: BTreeSet<AtomKey>,
    pub representation: BTreeSet<AtomKey>,
    pub body: BTreeSet<AtomKey>,
    /// Literal atoms within complete RHS-only packages, including nested variables.
    pub package_atoms: BTreeSet<AtomKey>,
}

#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub rules: BTreeMap<AtomKey, RuleView>,
    pub quoted: BTreeMap<AtomKey, AtomKey>,
    pub operative_memberships: BTreeSet<AtomKey>,
}

/// Validate all carrier, quotation, live-head and active-rule invariants.
pub fn validate(graph: &Graph) -> Result<Configuration> {
    let mut result = Configuration::default();
    let memberships: BTreeSet<_> = graph
        .iter()
        .filter(|(_, a)| a.kind.role().is_some_and(Role::membership))
        .map(|(id, _)| id)
        .collect();
    let named: BTreeSet<_> = memberships
        .iter()
        .map(|id| graph.atom(*id).map(|a| a.fields[1]))
        .collect::<Result<_>>()?;
    result.operative_memberships = memberships.difference(&named).copied().collect();
    let mut triples = BTreeSet::new();
    for id in &result.operative_memberships {
        let atom = graph.atom(*id)?;
        let (owner, item) = (atom.fields[0], atom.fields[1]);
        ensure(
            triples.insert((owner, atom.kind.role(), item)),
            "duplicate operative membership",
        )?;
        if let Some(previous) = result.quoted.insert(item, owner) {
            ensure(previous == owner, "shared quoted syntax")?;
        }
    }
    for id in memberships.difference(&result.operative_memberships) {
        ensure(
            result.quoted.contains_key(id),
            "rootless or indirectly owned membership",
        )?;
    }
    // Acyclic membership containment: a reachable cycle is invalid too.
    for start in &memberships {
        let mut seen = BTreeSet::new();
        let mut current = *start;
        while memberships.contains(&current) {
            ensure(seen.insert(current), "cyclic membership containment")?;
            current = graph.atom(current)?.fields[1];
        }
    }
    for (id, atom) in graph.iter() {
        let owner = result.quoted.get(&id);
        for (slot, target) in atom.fields.iter().enumerate() {
            let target_atom = graph.atom(*target)?;
            let expected = match atom.kind.role() {
                Some(Role::ObjectLink) => Some(Sort::Constructor),
                Some(Role::Lhs | Role::Rhs) if slot == 1 => None,
                Some(Role::Rank) if slot == 1 => Some(Sort::Nat),
                Some(_) => Some(Sort::RuleHead),
                None if atom.kind == Kind::Nat => Some(Sort::Nat),
                _ => None,
            };
            if let Some(expected) = expected {
                let compatible = match &target_atom.kind {
                    Kind::PatternVariable { admits, .. } if owner.is_some() => {
                        *admits == BTreeSet::from([expected])
                    }
                    _ => target_atom.kind.sort() == expected,
                };
                ensure(compatible, format!("field typing at {id:?} slot {slot}"))?;
            }
            if let Some(owner) = owner {
                ensure(
                    result.quoted.get(target) == Some(owner),
                    "quoted record points outside its owner",
                )?;
            }
        }
    }
    let live: BTreeSet<_> = graph
        .of_sort(Sort::RuleHead)
        .difference(&result.quoted.keys().copied().collect())
        .copied()
        .collect();
    for owner in result.quoted.values() {
        ensure(live.contains(owner), "membership owner is not a live head")?;
    }
    let mut rank_owners = BTreeMap::new();
    for head in &live {
        let owned: BTreeSet<_> = graph
            .iter()
            .filter(|(id, a)| {
                !result.quoted.contains_key(id)
                    && a.kind.role().is_some_and(|r| r != Role::ObjectLink)
                    && a.fields[0] == *head
            })
            .map(|(id, _)| id)
            .collect();
        let of_role = |role| -> Vec<AtomKey> {
            owned
                .iter()
                .copied()
                .filter(|id| graph.get(*id).and_then(|a| a.kind.role()) == Some(role))
                .collect()
        };
        let ranks = of_role(Role::Rank);
        let revisions = of_role(Role::Revision);
        let objects = of_role(Role::ModeObject);
        let metas = of_role(Role::ModeMeta);
        let active = of_role(Role::Active);
        ensure(
            ranks.len() == 1
                && revisions.len() == 1
                && objects.len() + metas.len() == 1
                && active.len() <= 1,
            "live head lacks unique rank/mode/revision/activity",
        )?;
        let mut root = graph.atom(ranks[0])?.fields[1];
        let mut chain = BTreeSet::new();
        loop {
            ensure(chain.insert(root), "cyclic rank")?;
            ensure(
                rank_owners.insert(root, *head).is_none(),
                "shared rank chain",
            )?;
            let atom = graph.atom(root)?;
            ensure(atom.kind == Kind::Nat, "rank contains non-Nat")?;
            if atom.fields.is_empty() {
                break;
            }
            root = atom.fields[0];
        }
        let lhs = of_role(Role::Lhs)
            .iter()
            .map(|id| graph.atom(*id).map(|a| a.fields[1]))
            .collect::<Result<BTreeSet<_>>>()?;
        let rhs = of_role(Role::Rhs)
            .iter()
            .map(|id| graph.atom(*id).map(|a| a.fields[1]))
            .collect::<Result<BTreeSet<_>>>()?;
        let mut representation = owned;
        representation.insert(*head);
        representation.extend(chain.iter());
        representation.extend(graph.closure(&lhs.union(&rhs).copied().collect())?);
        let mut body = representation.clone();
        body.remove(&revisions[0]);
        result.rules.insert(
            *head,
            RuleView {
                head: *head,
                rank: chain.len() - 1,
                revision: revisions[0],
                active: !active.is_empty(),
                meta: !metas.is_empty(),
                lhs,
                rhs,
                representation,
                body,
                package_atoms: BTreeSet::new(),
            },
        );
    }
    for rule in result.rules.values_mut() {
        validate_syntax(graph, rule)?;
    }
    Ok(result)
}

fn validate_syntax(graph: &Graph, rule: &mut RuleView) -> Result<()> {
    if rule.active {
        for side in [&rule.lhs, &rule.rhs] {
            ensure(graph.closure(side)? == *side, "unclosed rule side")?;
        }
    }
    let fresh: BTreeSet<_> = rule.rhs.difference(&rule.lhs).copied().collect();
    for id in &fresh {
        ensure(
            graph.atom(*id)?.fields.iter().all(|t| rule.rhs.contains(t)),
            "unclosed fresh RHS record",
        )?;
    }
    let literal_heads: Vec<_> = fresh
        .iter()
        .copied()
        .filter(|id| graph.get(*id).is_some_and(|a| a.kind == Kind::RuleHead))
        .collect();
    for head in literal_heads {
        if rule.package_atoms.contains(&head) {
            continue;
        }
        let mut package = BTreeSet::from([head]);
        loop {
            let old = package.clone();
            let heads: BTreeSet<_> = package
                .iter()
                .copied()
                .filter(|id| graph.get(*id).is_some_and(|a| a.kind == Kind::RuleHead))
                .collect();
            for id in &fresh {
                let atom = graph.atom(*id)?;
                if atom.kind.role().is_some_and(|r| r != Role::ObjectLink)
                    && heads.contains(&atom.fields[0])
                {
                    package.insert(*id);
                }
            }
            package = graph.closure(&package)?;
            if package == old {
                break;
            }
        }
        ensure(
            package.is_subset(&fresh),
            "package captures preserved or external atoms",
        )?;
        ensure(
            package.len() < graph.len(),
            "non-descending package validation",
        )?;
        let copied = graph.induced(&package)?;
        let validation = validate(&copied)?;
        ensure(
            validation.rules.contains_key(&head),
            "package head does not become live",
        )?;
        rule.package_atoms.extend(package);
    }
    for id in rule.lhs.union(&rule.rhs) {
        let atom = graph.atom(*id)?;
        ensure(
            !rule.lhs.contains(id) || atom.kind != Kind::RuleHead,
            "literal LHS head",
        )?;
        if !rule.active {
            continue;
        }
        if let Kind::PatternVariable { admits, binding } = &atom.kind {
            if !rule.package_atoms.contains(id) {
                ensure(
                    !rule.rhs.contains(id) || rule.lhs.contains(id),
                    "unbound RHS variable",
                )?;
                ensure(
                    *binding != Binding::AppliedRule || rule.meta,
                    "AppliedRule in object rule",
                )?;
            }
            if !rule.meta {
                ensure(
                    admits.iter().all(object_sort),
                    "meta variable in object rule",
                )?;
            }
        } else if !rule.meta {
            ensure(object_sort(&atom.kind.sort()), "meta syntax in object rule")?;
        }
    }
    Ok(())
}

fn object_sort(sort: &Sort) -> bool {
    matches!(sort, Sort::Constructor | Sort::Record(Role::ObjectLink))
}
