use crate::limits::ValidationBudget;
use crate::{
    ensure, AtomKey, Binding, Diagnostic, Graph, GraphLimits, Kind, LimitKind, Result, Role, Sort,
};
use std::collections::{BTreeMap, BTreeSet};

/// Detached decoded data. Editing a copy does not alter or validate its source graph.
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
    /// Exact successful full-validator charge. Derived metadata, not authority.
    pub validation_work: usize,
}

/// A read-only validated view tied to the borrowed graph's lifetime.
/// No caller can construct this certificate or mutate its graph while using it.
///
/// ```compile_fail
/// use graph_core::{Graph, GraphLimits, Kind, ValidatedGraph};
/// let mut graph = Graph::new();
/// let view = ValidatedGraph::new(&graph, &GraphLimits::default()).unwrap();
/// graph.insert(Kind::Nat, &[]).unwrap();
/// assert!(view.graph().is_empty());
/// ```
///
/// ```compile_fail
/// use graph_core::{Graph, GraphLimits, ValidatedGraph};
/// let graph = Graph::new();
/// let mut view = ValidatedGraph::new(&graph, &GraphLimits::default()).unwrap();
/// view.configuration().rules.clear();
/// ```
#[derive(Debug)]
pub struct ValidatedGraph<'g> {
    graph: &'g Graph,
    configuration: Configuration,
}
impl<'g> ValidatedGraph<'g> {
    pub fn new(graph: &'g Graph, limits: &GraphLimits) -> Result<Self> {
        Ok(Self {
            graph,
            configuration: validate_with_limits(graph, limits)?,
        })
    }
    pub fn graph(&self) -> &'g Graph {
        self.graph
    }
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }
    pub fn rule(&self, head: AtomKey) -> Option<&RuleView> {
        self.configuration.rules.get(&head)
    }
    /// All live heads, including inactive representations; quoted heads are absent.
    pub fn live_rules(&self) -> impl Iterator<Item = &RuleView> {
        self.configuration.rules.values()
    }
    /// Active live sources, not necessarily rules with an enabled match.
    pub fn active_rules(&self) -> impl Iterator<Item = &RuleView> {
        self.live_rules().filter(|rule| rule.active)
    }
}

/// Validate with default operational bounds; a resource error is not invalidity.
pub fn validate(graph: &Graph) -> Result<Configuration> {
    validate_with_limits(graph, &GraphLimits::default())
}

/// Validate all carrier, quotation, live-head and active-rule invariants with shared bounds.
pub fn validate_with_limits(graph: &Graph, limits: &GraphLimits) -> Result<Configuration> {
    limits.check_graph(graph)?;
    let mut budget = ValidationBudget::new(limits);
    let mut configuration = validate_inner(graph, limits, &mut budget, 0)?;
    configuration.validation_work = limits.max_validation_work - budget.remaining();
    Ok(configuration)
}

fn validate_inner(
    graph: &Graph,
    limits: &GraphLimits,
    budget: &mut ValidationBudget,
    depth: usize,
) -> Result<Configuration> {
    if depth > limits.max_package_depth {
        return Err(Diagnostic::exhausted(
            LimitKind::PackageDepth,
            "package nesting limit exceeded",
        ));
    }
    budget.charge(graph.len())?;
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
        )
        .map_err(|e| {
            e.at_atom(*id)
                .at_atom(owner)
                .at_atom(item)
                .at_role(atom.kind.role())
        })?;
        if let Some(previous) = result.quoted.insert(item, owner) {
            ensure(previous == owner, "shared quoted syntax").map_err(|e| {
                e.at_atom(item)
                    .at_atom(owner)
                    .at_atom(previous)
                    .at_role(atom.kind.role())
            })?;
        }
    }
    for id in memberships.difference(&result.operative_memberships) {
        ensure(
            result.quoted.contains_key(id),
            "rootless or indirectly owned membership",
        )
        .map_err(|e| {
            e.at_atom(*id)
                .at_role(graph.get(*id).and_then(|a| a.kind.role()))
        })?;
    }
    // Acyclic membership containment: a reachable cycle is invalid too.
    for start in &memberships {
        let mut seen = BTreeSet::new();
        let mut current = *start;
        while memberships.contains(&current) {
            budget.charge(1)?;
            ensure(seen.insert(current), "cyclic membership containment").map_err(|e| {
                e.at_atom(current)
                    .at_role(graph.get(current).and_then(|a| a.kind.role()))
            })?;
            current = graph.atom(current)?.fields[1];
        }
    }
    for (id, atom) in graph.iter() {
        budget.charge(1)?;
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
                ensure(compatible, format!("field typing at {id:?} slot {slot}")).map_err(|e| {
                    e.at_atom(id)
                        .at_atom(*target)
                        .at_role(atom.kind.role())
                        .at_slot(slot)
                })?;
            }
            if let Some(owner) = owner {
                ensure(
                    result.quoted.get(target) == Some(owner),
                    "quoted record points outside its owner",
                )
                .map_err(|e| {
                    e.at_atom(id)
                        .at_atom(*target)
                        .at_atom(*owner)
                        .at_role(atom.kind.role())
                        .at_slot(slot)
                })?;
            }
        }
    }
    let live: BTreeSet<_> = graph
        .of_sort(Sort::RuleHead)
        .difference(&result.quoted.keys().copied().collect())
        .copied()
        .collect();
    for owner in result.quoted.values() {
        ensure(live.contains(owner), "membership owner is not a live head")
            .map_err(|e| e.at_atom(*owner))?;
    }
    let mut rank_owners = BTreeMap::new();
    for head in &live {
        budget.charge(graph.len()).map_err(|e| e.at_atom(*head))?;
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
        let bad_role = if ranks.len() != 1 {
            Some(Role::Rank)
        } else if revisions.len() != 1 {
            Some(Role::Revision)
        } else if objects.len() > 1 {
            Some(Role::ModeObject)
        } else if metas.len() > 1 || (!objects.is_empty() && !metas.is_empty()) {
            Some(Role::ModeMeta)
        } else if objects.is_empty() && metas.is_empty() {
            None
        } else {
            Some(Role::Active)
        };
        ensure(
            ranks.len() == 1
                && revisions.len() == 1
                && objects.len() + metas.len() == 1
                && active.len() <= 1,
            "live head lacks unique rank/mode/revision/activity",
        )
        .map_err(|e| e.at_atom(*head).at_role(bad_role))?;
        let mut root = graph.atom(ranks[0])?.fields[1];
        let mut chain = BTreeSet::new();
        loop {
            budget.charge(1).map_err(|e| e.at_atom(*head))?;
            ensure(chain.insert(root), "cyclic rank")
                .map_err(|e| e.at_atom(*head).at_atom(root).at_role(Some(Role::Rank)))?;
            ensure(
                rank_owners.insert(root, *head).is_none(),
                "shared rank chain",
            )
            .map_err(|e| e.at_atom(*head).at_atom(root).at_role(Some(Role::Rank)))?;
            let atom = graph.atom(root)?;
            ensure(atom.kind == Kind::Nat, "rank contains non-Nat")
                .map_err(|e| e.at_atom(*head).at_atom(root).at_role(Some(Role::Rank)))?;
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
        budget.closure(graph)?;
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
        validate_syntax(graph, rule, limits, budget, depth).map_err(|e| e.at_atom(rule.head))?;
    }
    Ok(result)
}

fn validate_syntax(
    graph: &Graph,
    rule: &mut RuleView,
    limits: &GraphLimits,
    budget: &mut ValidationBudget,
    depth: usize,
) -> Result<()> {
    if rule.active {
        for (role, side) in [(Role::Lhs, &rule.lhs), (Role::Rhs, &rule.rhs)] {
            budget.closure(graph)?;
            ensure(graph.closure(side)? == *side, "unclosed rule side")
                .map_err(|e| e.at_role(Some(role)))?;
        }
    }
    let fresh: BTreeSet<_> = rule.rhs.difference(&rule.lhs).copied().collect();
    for id in &fresh {
        budget.charge(1)?;
        ensure(
            graph.atom(*id)?.fields.iter().all(|t| rule.rhs.contains(t)),
            "unclosed fresh RHS record",
        )
        .map_err(|e| {
            e.at_atom(*id)
                .at_role(graph.get(*id).and_then(|a| a.kind.role()))
        })?;
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
            budget.charge(fresh.len())?;
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
            budget.closure(graph)?;
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
        budget.closure(graph)?;
        let copied = graph.induced(&package)?;
        let next_depth = depth.checked_add(1).ok_or_else(|| {
            Diagnostic::exhausted(LimitKind::PackageDepth, "package depth overflow")
        })?;
        let validation =
            validate_inner(&copied, limits, budget, next_depth).map_err(|e| e.at_atom(head))?;
        ensure(
            validation.rules.contains_key(&head),
            "package head does not become live",
        )?;
        rule.package_atoms.extend(package);
    }
    for id in rule.lhs.union(&rule.rhs) {
        budget.charge(1)?;
        let atom = graph.atom(*id)?;
        ensure(
            !rule.lhs.contains(id) || atom.kind != Kind::RuleHead,
            "literal LHS head",
        )
        .map_err(|e| e.at_atom(*id))?;
        if !rule.active {
            continue;
        }
        if let Kind::PatternVariable { admits, binding } = &atom.kind {
            if !rule.package_atoms.contains(id) {
                ensure(
                    !rule.rhs.contains(id) || rule.lhs.contains(id),
                    "unbound RHS variable",
                )
                .map_err(|e| e.at_atom(*id))?;
                ensure(
                    *binding != Binding::AppliedRule || rule.meta,
                    "AppliedRule in object rule",
                )
                .map_err(|e| e.at_atom(*id))?;
            }
            if !rule.meta {
                ensure(
                    admits.iter().all(object_sort),
                    "meta variable in object rule",
                )
                .map_err(|e| e.at_atom(*id))?;
            }
        } else if !rule.meta {
            ensure(object_sort(&atom.kind.sort()), "meta syntax in object rule")
                .map_err(|e| e.at_atom(*id).at_role(atom.kind.role()))?;
        }
    }
    Ok(())
}

fn object_sort(sort: &Sort) -> bool {
    matches!(sort, Sort::Constructor | Sort::Record(Role::ObjectLink))
}
