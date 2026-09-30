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
    /// v2: allow absent/duplicate declared statuses outside legitimacy.
    #[serde(default)]
    pub allow_status_faults: bool,
    /// v2: require identical declared output across all legitimate outcomes.
    #[serde(default)]
    pub require_agreement: bool,
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
    pub agreement: &'static str,
    pub legitimate_outputs: Vec<Status>,
    pub output_representatives: Vec<usize>,
}

type Result<T> = graph_core::Result<T>;
pub type Status = Vec<Vec<(String, String)>>;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(ErrorKind::MalformedInput, message)
}
fn violation(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(ErrorKind::InvalidBinding, message)
}
fn exhausted(message: &str) -> Diagnostic {
    Diagnostic::exhausted(LimitKind::ObservationHistory, message)
}

fn status_units(status: &Status) -> usize {
    status.iter().flatten().fold(status.len(), |n, (a, b)| {
        n.saturating_add(2)
            .saturating_add(a.len())
            .saturating_add(b.len())
    })
}

impl Contract {
    fn names(&self) -> Result<BTreeSet<String>> {
        if !["recovery-contract-v1", "recovery-contract-v2"].contains(&self.schema.as_str())
            || self.requirements.is_empty()
            || (self.schema == "recovery-contract-v1"
                && (self.allow_status_faults || self.require_agreement))
        {
            return Err(invalid(
                "expected supported recovery contract and nonempty requirements; extended obligations require v2",
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
            if links.len() != 1 && !self.allow_status_faults {
                return Err(violation(format!(
                    "expected exactly one status relationship for {}",
                    r.from
                )));
            }
            legitimate &= links.len() == 1;
            let mut values = Vec::new();
            for (label, target) in links {
                let name = r.targets.iter().find(|name| resolved[*name] == target);
                let Some(name) = name else {
                    return Err(violation(format!(
                        "status relationship has an unintended target: {}",
                        r.from
                    )));
                };
                legitimate &= r.legitimate_labels.contains(label);
                values.push((label.clone(), name.clone()));
            }
            values.sort();
            status.push(values);
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

/// External, contract-directed recovery receipt. This is not a guarded event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconstructionReceipt {
    pub schema: String,
    pub reset_label: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub removed: Vec<u64>,
    pub created: Vec<u64>,
}

fn digest(graph: &Graph) -> String {
    use sha2::{Digest, Sha256};
    let atoms: Vec<_> = graph
        .iter()
        .map(|(k, a)| {
            (
                k.number(),
                &a.kind,
                a.fields.iter().map(|k| k.number()).collect::<Vec<_>>(),
            )
        })
        .collect();
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(graph.next_identity(), atoms)).expect("graph records serialise")
        )
    )
}

/// Reconstruct declared status records from a trusted unambiguous contract.
/// Requires an already core-valid carrier and preserved unique named payloads.
/// Does not reconstruct rule logic, restore missing payload or establish closure.
/// The input is borrowed and unchanged on both success and failure.
pub fn reconstruct_statuses(
    graph: &Graph,
    contract: &Contract,
    reset_label: &str,
    limits: ExecutionLimits,
) -> Result<(Graph, ReconstructionReceipt)> {
    let names = contract.names()?;
    let before = Execution::new(graph.clone(), limits)?;
    let mut domain = contract.clone();
    domain.schema = "recovery-contract-v2".into();
    domain.allow_status_faults = true;
    let (anchors, _, _) = domain.inspect(&before, &names, None)?;
    let configuration = before.view()?.configuration().clone();
    let hidden: BTreeSet<_> = configuration
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    let mut selected = BTreeSet::new();
    let mut removed = BTreeSet::new();
    let mut replacements = Vec::new();
    for r in &contract.requirements {
        if r.targets.len() != 1
            || !r.labels.contains(reset_label)
            || r.legitimate_labels.contains(reset_label)
        {
            return Err(invalid(
                "reconstruction requires one target and an allowed non-legitimate retry label",
            ));
        }
        for label in &r.labels {
            if !selected.insert((r.from.clone(), label.clone())) {
                return Err(invalid("overlapping reconstruction requirements"));
            }
        }
        let label = reset_label.to_string();
        let from = anchors[&r.from];
        let to = anchors[r.targets.first().unwrap()];
        let links:Vec<_>=graph.iter().filter(|(k,a)| !hidden.contains(k) && matches!(&a.kind,Kind::Record{role:Role::ObjectLink,label:Some(l)} if a.fields[0]==from && r.labels.contains(l))).collect();
        if links.len() == 1 && links[0].1.fields[1] == to {
            continue;
        }
        removed.extend(links.into_iter().map(|(k, _)| k));
        replacements.push((label.clone(), from, to));
    }
    let mut repaired = graph.clone();
    repaired.remove_set(&removed)?;
    let mut created = Vec::new();
    for (label, from, to) in replacements {
        created.push(
            repaired
                .insert(
                    Kind::Record {
                        role: Role::ObjectLink,
                        label: Some(label),
                    },
                    &[from, to],
                )?
                .number(),
        );
    }
    let checked = Execution::new(repaired.clone(), limits)?;
    domain.allow_status_faults = false;
    domain.inspect(&checked, &names, Some(&anchors))?;
    let receipt = ReconstructionReceipt {
        schema: "status-reconstruction-v1".into(),
        reset_label: reset_label.into(),
        before_sha256: digest(graph),
        after_sha256: digest(&repaired),
        removed: removed.iter().map(|k| k.number()).collect(),
        created,
    };
    Ok((repaired, receipt))
}

/// Recompute recovery rather than trusting a receipt's assertions or hashes.
pub fn verify_reconstruction(
    before: &Graph,
    after: &Graph,
    contract: &Contract,
    receipt: &ReconstructionReceipt,
    reset_label: &str,
    limits: ExecutionLimits,
) -> Result<()> {
    let (expected, expected_receipt) = reconstruct_statuses(before, contract, reset_label, limits)?;
    if receipt != &expected_receipt
        || expected.snapshot() != after.snapshot()
        || expected.next_identity() != after.next_identity()
    {
        return Err(violation("reconstruction receipt or result mismatch"));
    }
    Ok(())
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
    let mut units = observation
        .retention_units()
        .saturating_add(status_units(&status));
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
        schema: "recovery-analysis-v2",
        verdict: "refuted",
        reason: String::new(),
        states: 1,
        edges: Vec::new(),
        legitimate: vec![legitimate],
        ranks: Vec::new(),
        recovery_bound: None,
        witness_states: Vec::new(),
        stable_when_legitimate: contract.stable_when_legitimate,
        agreement: "not-established",
        legitimate_outputs: Vec::new(),
        output_representatives: Vec::new(),
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
                comparison.charge(
                    1 + status
                        .iter()
                        .flatten()
                        .map(|(a, b)| a.len() + b.len())
                        .sum::<usize>(),
                )?;
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
                units = units
                    .saturating_add(observation.retention_units())
                    .saturating_add(status_units(&status));
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
            for (i, state) in states.iter().enumerate().filter(|(_, s)| s.legitimate) {
                if !report.legitimate_outputs.contains(&state.status) {
                    units = units
                        .saturating_add(status_units(&state.status))
                        .saturating_add(1);
                    if units > limits.max_retained_units {
                        return Err(exhausted("stability output retention bound"));
                    }
                    report.legitimate_outputs.push(state.status.clone());
                    report.output_representatives.push(i);
                }
            }
            report.agreement = if report.legitimate_outputs.len() == 1 {
                "agreed"
            } else {
                "disagreed"
            };
            if contract.require_agreement && report.agreement != "agreed" {
                report.verdict = "refuted";
                report.reason = "recovery holds but declared outputs disagree across reachable legitimate states".into();
                report.witness_states = report.output_representatives.clone();
            }
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
