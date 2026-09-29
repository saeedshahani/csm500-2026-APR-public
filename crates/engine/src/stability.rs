//! Complete finite-quotient recovery analysis, not a new execution policy.
//!
//! Only full-carrier isomorphism merges states. Successful exploration checks
//! every admitted reference transition, including transitions within legitimacy.
//! Resource exhaustion is an error (inconclusive), never a proof or refutation.
use crate::observation::{self, Budget, Observation, ObservationLimits};
use crate::{Execution, ExecutionLimits};
use graph_core::{AtomKey, Diagnostic, ErrorKind, Graph, Kind, LimitKind, Role};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One visible relationship per named payload, with a declared legitimate subset.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub from: String,
    pub targets: BTreeSet<String>,
    pub labels: BTreeSet<String>,
    pub legitimate_labels: BTreeSet<String>,
}

/// Labels identify unique visible constructors, not raw runtime identities.
/// All named constructors must survive with their original identity and payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema: String,
    pub preserve: BTreeSet<String>,
    pub requirements: Vec<Requirement>,
    pub stable_when_legitimate: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub execution: ExecutionLimits,
    pub max_states: usize,
    pub max_edges: usize,
    pub max_retained_units: usize,
    pub max_comparison_work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            execution: ExecutionLimits::default(),
            max_states: 128,
            max_edges: 4096,
            max_retained_units: 1_000_000,
            max_comparison_work: 10_000_000,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Edge {
    pub from: usize,
    /// None denotes an immediate contract-violating successor, not a retained state.
    pub to: Option<usize>,
    pub source: u64,
    pub source_revision: u64,
    pub binding: Vec<(u64, u64)>,
    pub altered_rules: Vec<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub verdict: &'static str,
    pub reason: String,
    pub states: usize,
    pub edges: Vec<Edge>,
    pub legitimate: Vec<bool>,
    /// Null within legitimacy; outside, maximum steps to first legitimacy.
    pub ranks: Vec<Option<usize>>,
    pub recovery_bound: Option<usize>,
    pub witness_states: Vec<usize>,
    pub stable_when_legitimate: bool,
}

type Result<T> = graph_core::Result<T>;
type Status = Vec<(String, String)>;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(ErrorKind::MalformedInput, message)
}
fn violation(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(ErrorKind::InvalidBinding, message)
}
fn exhausted(message: &str) -> Diagnostic {
    Diagnostic::exhausted(LimitKind::ObservationHistory, message)
}

impl Contract {
    fn names(&self) -> Result<BTreeSet<String>> {
        if self.schema != "recovery-contract-v1" || self.requirements.is_empty() {
            return Err(invalid(
                "expected recovery-contract-v1 and nonempty requirements",
            ));
        }
        let mut names = self.preserve.clone();
        for r in &self.requirements {
            if r.targets.is_empty()
                || r.labels.is_empty()
                || r.legitimate_labels.is_empty()
                || !r.legitimate_labels.is_subset(&r.labels)
            {
                return Err(invalid("empty or inconsistent relationship requirement"));
            }
            names.insert(r.from.clone());
            names.extend(r.targets.iter().cloned());
        }
        Ok(names)
    }

    fn inspect(
        &self,
        execution: &Execution,
        names: &BTreeSet<String>,
        anchors: Option<&BTreeMap<String, AtomKey>>,
    ) -> Result<(BTreeMap<String, AtomKey>, Status, bool)> {
        let view = execution.view()?;
        let hidden: BTreeSet<_> = view
            .configuration()
            .rules
            .values()
            .flat_map(|r| r.representation.iter().copied())
            .collect();
        let mut resolved = BTreeMap::new();
        for (key, atom) in execution.graph().iter() {
            if hidden.contains(&key) {
                continue;
            }
            if let Kind::Constructor(label) = &atom.kind {
                if names.contains(label) && resolved.insert(label.clone(), key).is_some() {
                    return Err(violation(format!(
                        "named visible payload is not unique: {label}"
                    )));
                }
            }
        }
        if resolved.len() != names.len() {
            return Err(violation("named payload is absent or quoted"));
        }
        if anchors.is_some_and(|a| *a != resolved) {
            return Err(violation("protected payload identity changed"));
        }
        let mut status = Vec::new();
        let mut legitimate = true;
        for r in &self.requirements {
            let links: Vec<_> = execution
                .graph()
                .iter()
                .filter_map(|(key, atom)| match &atom.kind {
                    Kind::Record {
                        role: Role::ObjectLink,
                        label: Some(label),
                    } if !hidden.contains(&key)
                        && atom.fields[0] == resolved[&r.from]
                        && r.labels.contains(label) =>
                    {
                        Some((label, atom.fields[1]))
                    }
                    _ => None,
                })
                .collect();
            if links.len() != 1 {
                return Err(violation(format!(
                    "expected exactly one status relationship for {}",
                    r.from
                )));
            }
            let (label, target) = links[0];
            let name = r.targets.iter().find(|name| resolved[*name] == target);
            let Some(name) = name else {
                return Err(violation(format!(
                    "status relationship has an unintended target: {}",
                    r.from
                )));
            };
            legitimate &= r.legitimate_labels.contains(label);
            status.push((label.clone(), name.clone()));
        }
        Ok((resolved, status, legitimate))
    }
}

struct State {
    execution: Execution,
    observation: Observation,
    status: Status,
    legitimate: bool,
    parent: Option<usize>,
}

fn path(states: &[State], mut at: usize) -> Vec<usize> {
    let mut result = vec![at];
    while let Some(parent) = states[at].parent {
        at = parent;
        result.push(at);
    }
    result.reverse();
    result
}

/// Verify an explicit finite transition graph and derive a common ranking.
/// This alone does NOT establish that its edges exhaust any carrier semantics.
/// `analyse` is the trusted complete-edge producer; imported edge lists are not
/// accepted as engine certificates.
pub fn rank_quotient(
    legitimate: &[bool],
    successors: &[Vec<usize>],
) -> std::result::Result<Vec<Option<usize>>, String> {
    let n = legitimate.len();
    if n == 0 || successors.len() != n || successors.iter().flatten().any(|x| *x >= n) {
        return Err("invalid finite transition graph".into());
    }
    let mut ranks = vec![None; n];
    let mut incoming = vec![Vec::new(); n];
    let mut remaining = vec![0; n];
    let mut ready = VecDeque::new();
    for i in 0..n {
        if legitimate[i] {
            if successors[i].iter().any(|j| !legitimate[*j]) {
                return Err("legitimate region is not closed".into());
            }
        } else {
            if successors[i].is_empty() {
                return Err("deadlock outside legitimacy".into());
            }
            for j in &successors[i] {
                if !legitimate[*j] {
                    remaining[i] += 1;
                    incoming[*j].push(i);
                }
            }
            if remaining[i] == 0 {
                ready.push_back(i);
            }
        }
    }
    while let Some(i) = ready.pop_front() {
        ranks[i] = Some(
            1 + successors[i]
                .iter()
                .filter_map(|j| ranks[*j])
                .max()
                .unwrap_or(0),
        );
        for p in &incoming[i] {
            remaining[*p] -= 1;
            if remaining[*p] == 0 {
                ready.push_back(*p);
            }
        }
    }
    if (0..n).any(|i| !legitimate[i] && ranks[i].is_none()) {
        return Err("cycle outside legitimacy (no scheduler fairness assumed)".into());
    }
    Ok(ranks)
}

/// Analyse all reachable carriers from this initial graph modulo exact full
/// isomorphism. A success concerns this closed reachable domain, not every size,
/// every structurally valid initial graph, or arbitrary environmental faults.
pub fn analyse(graph: Graph, contract: &Contract, limits: Limits) -> Result<Report> {
    let names = contract.names()?;
    let first = Execution::new(graph, limits.execution)?;
    let (anchors, status, legitimate) = contract.inspect(&first, &names, None)?;
    let observation_limits = ObservationLimits {
        max_work: limits.max_retained_units,
        max_comparison_work: limits.max_comparison_work,
        max_history_units: limits.max_retained_units,
    };
    let observation = observation::all(first.graph(), limits.execution.graph, observation_limits)?;
    let mut units = observation.retention_units();
    if limits.max_states == 0 || units > limits.max_retained_units {
        return Err(exhausted("stability state/retention bound"));
    }
    let mut states = vec![State {
        execution: first,
        observation,
        status,
        legitimate,
        parent: None,
    }];
    let mut report = Report {
        schema: "recovery-analysis-v1",
        verdict: "refuted",
        reason: String::new(),
        states: 1,
        edges: Vec::new(),
        legitimate: vec![legitimate],
        ranks: Vec::new(),
        recovery_bound: None,
        witness_states: Vec::new(),
        stable_when_legitimate: contract.stable_when_legitimate,
    };
    let mut comparison = Budget::new(limits.max_comparison_work, LimitKind::ComparisonWork);
    let mut cursor = 0;
    while cursor < states.len() {
        // Complete reference matching, not a scheduler-selected trace or cache.
        let enabled = states[cursor].execution.enabled()?;
        if enabled.steps().is_empty() && !states[cursor].legitimate {
            report.reason = "deadlock outside legitimacy".into();
            report.witness_states = path(&states, cursor);
            return Ok(report);
        }
        for step in enabled.into_steps() {
            if report.edges.len() >= limits.max_edges {
                return Err(exhausted("stability edge bound"));
            }
            let mut branch = states[cursor].execution.fork();
            let selected = branch.prepare(step.binding().clone())?;
            let event = branch.commit(selected)?;
            let inspected = contract.inspect(&branch, &names, Some(&anchors));
            let (_, status, legitimate) = match inspected {
                Ok(value) => value,
                Err(error) => {
                    if error.kind() != ErrorKind::InvalidBinding {
                        return Err(error);
                    }
                    report.reason = error.to_string();
                    report.witness_states = path(&states, cursor);
                    // The offending edge's binding is sufficient to reproduce
                    // the immediate successor from the named representative.
                    report.edges.push(Edge {
                        from: cursor,
                        to: None,
                        source: event.source.number(),
                        source_revision: event.source_revision.number(),
                        binding: step
                            .binding()
                            .assignments()
                            .iter()
                            .map(|(a, b)| (a.number(), b.number()))
                            .collect(),
                        altered_rules: event.altered.iter().map(|k| k.number()).collect(),
                    });
                    return Ok(report);
                }
            };
            if states[cursor].legitimate
                && (!legitimate
                    || (contract.stable_when_legitimate && status != states[cursor].status))
            {
                report.reason = "legitimate closure or declared output stability violated".into();
                report.witness_states = path(&states, cursor);
                report.edges.push(Edge {
                    from: cursor,
                    to: None,
                    source: event.source.number(),
                    source_revision: event.source_revision.number(),
                    binding: step
                        .binding()
                        .assignments()
                        .iter()
                        .map(|(a, b)| (a.number(), b.number()))
                        .collect(),
                    altered_rules: event.altered.iter().map(|k| k.number()).collect(),
                });
                return Ok(report);
            }
            let observation =
                observation::all(branch.graph(), limits.execution.graph, observation_limits)?;
            let mut found = None;
            for (index, state) in states.iter().enumerate() {
                // Unique visible named constructors are fixed by any full
                // isomorphism. Different declared statuses therefore exclude
                // equality, but equal statuses never prove it.
                comparison
                    .charge(1 + status.iter().map(|(a, b)| a.len() + b.len()).sum::<usize>())?;
                if state.status == status
                    && state.observation.signature() == observation.signature()
                    && observation::isomorphic_indexed_budget(
                        &state.observation,
                        &observation,
                        &mut comparison,
                    )?
                {
                    found = Some(index);
                    break;
                }
            }
            let destination = found.unwrap_or(states.len());
            if found.is_none() {
                if states.len() >= limits.max_states {
                    return Err(exhausted("stability state bound"));
                }
                units = units.saturating_add(observation.retention_units());
                if units > limits.max_retained_units {
                    return Err(exhausted("stability retention bound"));
                }
                states.push(State {
                    execution: branch,
                    observation,
                    status,
                    legitimate,
                    parent: Some(cursor),
                });
                report.legitimate.push(legitimate);
                report.states = states.len();
            }
            units = units
                .saturating_add(5 + 2 * step.binding().assignments().len() + event.altered.len());
            if units > limits.max_retained_units {
                return Err(exhausted("stability retention bound"));
            }
            report.edges.push(Edge {
                from: cursor,
                to: Some(destination),
                source: event.source.number(),
                source_revision: event.source_revision.number(),
                binding: step
                    .binding()
                    .assignments()
                    .iter()
                    .map(|(a, b)| (a.number(), b.number()))
                    .collect(),
                altered_rules: event.altered.iter().map(|k| k.number()).collect(),
            });
        }
        cursor += 1;
    }
    let mut successors = vec![Vec::new(); states.len()];
    for e in &report.edges {
        successors[e.from].push(e.to.expect("complete quotient has no violating successor"));
    }
    match rank_quotient(&report.legitimate, &successors) {
        Ok(ranks) => {
            report.recovery_bound = Some(ranks[0].unwrap_or(0));
            report.ranks = ranks;
            report.verdict = "certified";
            report.reason =
                "complete finite quotient: preservation, progress and legitimate closure".into();
        }
        Err(reason) => {
            report.reason = reason;
            // Follow nonlegitimate successors until a repeated vertex yields
            // a concrete quotient lasso. Closure/deadlocks were checked above.
            let mut live: BTreeSet<_> = (0..states.len())
                .filter(|i| !states[*i].legitimate)
                .collect();
            loop {
                let next: BTreeSet<_> = live
                    .iter()
                    .copied()
                    .filter(|i| successors[*i].iter().any(|j| live.contains(j)))
                    .collect();
                if next == live {
                    break;
                }
                live = next;
            }
            if let Some(start) = live.first().copied() {
                let mut seen = BTreeSet::new();
                let mut at = start;
                report.witness_states = path(&states, at);
                while seen.insert(at) {
                    at = *successors[at].iter().find(|j| live.contains(j)).unwrap();
                    report.witness_states.push(at);
                }
            }
        }
    }
    Ok(report)
}
