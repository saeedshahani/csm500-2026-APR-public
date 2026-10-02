//! Offline relative-termination analysis. No change to GC or runtime admission.
//! The native entry point builds every edge itself; supplied abstract edges or
//! potentials never authorise execution or assert matcher completeness.
use crate::observation::{self, Budget, Observation, ObservationLimits};
use crate::stability::{Contract, Limits, Status};
use crate::Execution;
use graph_core::{Binding, Diagnostic, ErrorKind, Graph, Kind, LimitKind, Result, Sort};
use serde::Serialize;
use std::collections::VecDeque;

#[derive(Clone, Debug, Serialize)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub source: u64,
    pub source_revision: u64,
    pub binding: Vec<(u64, u64)>,
    pub entered: Vec<u64>,
    pub exited: Vec<u64>,
    pub altered: Vec<u64>,
    pub meta_source: bool,
    pub object_progress: bool,
}
impl Edge {
    pub fn edits(&self) -> bool {
        !(self.entered.is_empty() && self.exited.is_empty() && self.altered.is_empty())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Bound {
    pub verdict: &'static str,
    pub initial_bound: Option<usize>,
    pub maximum_bound: Option<usize>,
    /// A nonincreasing credit on zero-cost steps; strict on unit-cost steps.
    pub potential: Vec<usize>,
    /// Indices into the supplied edge list forming a positive-cost cycle.
    pub cycle_edges: Vec<usize>,
}

/// Verify a supplied abstract potential, not the completeness of its edge list.
pub fn verify_potential(
    states: usize,
    edges: &[(usize, usize, bool)],
    potential: &[usize],
) -> bool {
    potential.len() == states
        && edges.iter().all(|&(a, b, w)| {
            a < states
                && b < states
                && potential[b]
                    .checked_add(usize::from(w))
                    .is_some_and(|n| potential[a] >= n)
        })
}

/// Weighted finite-graph criterion. All vertices are included; native callers
/// supply only reachable classes. Resource exhaustion never means refutation.
pub fn weighted_bound(
    states: usize,
    edges: &[(usize, usize, bool)],
    max_work: usize,
) -> Result<Bound> {
    let mut budget = Budget::new(max_work, LimitKind::ComparisonWork);
    bound(states, edges, &mut budget)
}

fn bound(states: usize, edges: &[(usize, usize, bool)], budget: &mut Budget) -> Result<Bound> {
    if states == 0 || edges.iter().any(|&(a, b, _)| a >= states || b >= states) {
        return Err(Diagnostic::new(
            ErrorKind::MalformedInput,
            "invalid finite weighted graph",
        ));
    }
    budget.charge(states.saturating_add(edges.len()))?;
    let mut potential = vec![0usize; states];
    for _ in 0..states {
        budget.charge(states)?;
        let old = potential.clone();
        let mut changed = false;
        for &(a, b, w) in edges {
            budget.charge(1)?;
            let value = old[b] + usize::from(w);
            if value > potential[a] {
                potential[a] = value;
                changed = true;
            }
        }
        if !changed {
            debug_assert!(verify_potential(states, edges, &potential));
            return Ok(Bound {
                verdict: "certified",
                initial_bound: Some(potential[0]),
                maximum_bound: potential.iter().copied().max(),
                potential,
                cycle_edges: Vec::new(),
            });
        }
    }
    // A positive cycle exists. Recover one using a positive edge plus a path
    // back to its source. The witness includes zero-cost administrative steps.
    let mut outgoing = vec![Vec::new(); states];
    for (i, &(a, _, _)) in edges.iter().enumerate() {
        outgoing[a].push(i);
    }
    for (first, &(a, b, w)) in edges.iter().enumerate() {
        if !w {
            continue;
        }
        budget.charge(states)?;
        let mut previous: Vec<Option<usize>> = vec![None; states];
        let mut seen = vec![false; states];
        seen[b] = true;
        let mut queue = VecDeque::from([b]);
        while let Some(at) = queue.pop_front() {
            if at == a {
                let mut path = Vec::new();
                let mut at = a;
                while at != b {
                    let i = previous[at].expect("reached by a path");
                    path.push(i);
                    at = edges[i].0;
                }
                path.reverse();
                let mut cycle = vec![first];
                cycle.extend(path);
                return Ok(Bound {
                    verdict: "refuted",
                    initial_bound: None,
                    maximum_bound: None,
                    potential: Vec::new(),
                    cycle_edges: cycle,
                });
            }
            for &i in &outgoing[at] {
                budget.charge(1)?;
                let to = edges[i].1;
                if !seen[to] {
                    seen[to] = true;
                    previous[to] = Some(i);
                    queue.push_back(to);
                }
            }
        }
    }
    unreachable!("a growing nth relaxation requires a positive cycle")
}

#[derive(Clone, Debug, Serialize)]
pub struct Recovery {
    pub verdict: &'static str,
    pub bound: Option<usize>,
    pub closed: bool,
    pub stable_output: bool,
    pub agreement: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Abstraction {
    pub kind: &'static str,
    pub label: String,
    pub initially_erased: usize,
    /// Sum over explored representative edges, NOT an execution's garbage count.
    pub edge_erasure_count: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstraction: Option<Abstraction>,
    pub states: usize,
    pub edges: Vec<Edge>,
    pub legitimate: Option<Vec<bool>>,
    pub total_edits: Bound,
    pub edit_burst: Bound,
    pub meta_invocations: Bound,
    pub meta_burst: Bound,
    pub edits_without_object_progress: Option<Bound>,
    pub recovery: Option<Recovery>,
    pub productive_modulation: Option<&'static str>,
}
struct State {
    execution: Execution,
    observation: Observation,
    status: Status,
    legitimate: bool,
    unresolved: usize,
}

/// Complete full-carrier exploration with optional protected-payload contract.
/// Progress means an object-mode step reduces the number of unmet requirements.
/// It is an explicitly chosen task measure, not an intrinsic notion of usefulness.
pub fn analyse(graph: Graph, contract: Option<&Contract>, limits: Limits) -> Result<Report> {
    analyse_impl(graph, contract, limits, None)
}

/// Erase only isolated constructors of a selected label in private analysis
/// representatives. Check every reachable state's active patterns for possible
/// access. This is not runtime GC, a payload-only projection or a fairness rule.
pub fn analyse_inert(
    graph: Graph,
    contract: Option<&Contract>,
    limits: Limits,
    label: &str,
) -> Result<Report> {
    analyse_impl(graph, contract, limits, Some(label))
}

/// Local obligation checker, also useful for conformance tests. Successful
/// projection alone does NOT prove future revisions preserve noninterference.
pub fn project_inert(execution: &Execution, label: &str, limits: Limits) -> Result<(Graph, usize)> {
    let mut budget = Budget::new(limits.max_comparison_work, LimitKind::ComparisonWork);
    project(execution, label, &mut budget)
}

fn project(execution: &Execution, label: &str, budget: &mut Budget) -> Result<(Graph, usize)> {
    let graph = execution.graph();
    budget.charge(graph.retention_units())?;
    // Active LHSs are record-closed. A contextual reference to a candidate atom
    // is impossible when that atom has zero incoming incidence.
    for rule in execution.view()?.active_rules() {
        for id in &rule.lhs {
            budget.charge(1)?;
            let unsafe_access = match &graph.atom(*id)?.kind {
                Kind::Constructor(s) => s == label,
                Kind::PatternVariable { admits, binding } => {
                    *binding == Binding::Free && admits.contains(&Sort::Constructor)
                }
                _ => false,
            };
            if unsafe_access {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidBinding,
                    "unsupported inert abstraction: an active pattern can read the erased label",
                )
                .at_atom(rule.head)
                .at_atom(*id));
            }
        }
    }
    let mut erased = std::collections::BTreeSet::new();
    for (id, atom) in graph.iter() {
        if matches!(&atom.kind, Kind::Constructor(s) if s == label) {
            budget.charge(1)?;
            let incoming = graph.references_to(id);
            budget.charge(incoming.len())?;
            if incoming.is_empty() {
                erased.insert(id);
            }
        }
    }
    let mut projected = graph.clone();
    projected.remove_set(&erased)?;
    Ok((projected, erased.len()))
}

fn analyse_impl(
    graph: Graph,
    contract: Option<&Contract>,
    limits: Limits,
    label: Option<&str>,
) -> Result<Report> {
    let names = contract.map(Contract::names).transpose()?;
    if label.is_some_and(|s| names.as_ref().is_some_and(|names| names.contains(s))) {
        return Err(Diagnostic::new(
            ErrorKind::InvalidBinding,
            "unsupported inert abstraction: declared payload uses erased label",
        ));
    }
    let mut budget = Budget::new(limits.max_comparison_work, LimitKind::ComparisonWork);
    let mut first = Execution::new(graph, limits.execution)?;
    let mut abstraction = None;
    if let Some(label) = label {
        let (g, n) = project(&first, label, &mut budget)?;
        first = Execution::new(g, limits.execution)?;
        abstraction = Some(Abstraction {
            kind: "isolated-inert-constructors-v1",
            label: label.into(),
            initially_erased: n,
            edge_erasure_count: 0,
        });
    }
    let (anchors, status, legitimate) = if let Some(c) = contract {
        c.inspect(&first, names.as_ref().unwrap(), None)?
    } else {
        Default::default()
    };
    let unresolved = count_unresolved(contract, &status)?;
    let ol = ObservationLimits {
        max_work: limits.max_retained_units,
        max_history_units: limits.max_retained_units,
        max_comparison_work: limits.max_comparison_work,
    };
    let observation = observation::all(first.graph(), limits.execution.graph, ol)?;
    let mut units = observation.retention_units() + status_units(&status);
    check_units(units, limits)?;
    if limits.max_states == 0 {
        return Err(exhausted("meta state bound"));
    }
    let mut states = vec![State {
        execution: first,
        observation,
        status,
        legitimate,
        unresolved,
    }];
    let mut edges = Vec::new();
    let mut cursor = 0;
    while cursor < states.len() {
        let enabled = states[cursor].execution.enabled()?;
        for step in enabled.into_steps() {
            if edges.len() >= limits.max_edges {
                return Err(exhausted("meta edge bound"));
            }
            let meta_source = states[cursor].execution.view()?.configuration().rules
                [&step.binding().source()]
                .meta;
            let mut branch = states[cursor].execution.fork();
            let prepared = branch.prepare(step.binding().clone())?;
            let event = branch.commit(prepared)?;
            if let Some(a) = &mut abstraction {
                let (g, n) = project(&branch, &a.label, &mut budget)?;
                a.edge_erasure_count = a
                    .edge_erasure_count
                    .checked_add(n)
                    .ok_or_else(|| exhausted("erasure count overflow"))?;
                branch = Execution::new(g, limits.execution)?;
            }
            let (_, status, legitimate) = if let Some(c) = contract {
                c.inspect(&branch, names.as_ref().unwrap(), Some(&anchors))?
            } else {
                Default::default()
            };
            let unresolved = count_unresolved(contract, &status)?;
            let object_progress = !meta_source && unresolved < states[cursor].unresolved;
            let observation = observation::all(branch.graph(), limits.execution.graph, ol)?;
            let mut found = None;
            for (i, s) in states.iter().enumerate() {
                budget.charge(1 + status_units(&status))?;
                if s.status == status
                    && s.observation.signature() == observation.signature()
                    && observation::isomorphic_indexed_budget(
                        &s.observation,
                        &observation,
                        &mut budget,
                    )?
                {
                    found = Some(i);
                    break;
                }
            }
            let to = found.unwrap_or(states.len());
            if found.is_none() {
                if states.len() >= limits.max_states {
                    return Err(exhausted("meta state bound"));
                }
                units = units
                    .saturating_add(observation.retention_units())
                    .saturating_add(status_units(&status));
                check_units(units, limits)?;
                states.push(State {
                    execution: branch,
                    observation,
                    status,
                    legitimate,
                    unresolved,
                });
            }
            units = units.saturating_add(
                9 + step.binding().assignments().len() * 2
                    + event.entered.len()
                    + event.exited.len()
                    + event.altered.len(),
            );
            check_units(units, limits)?;
            edges.push(Edge {
                from: cursor,
                to,
                source: event.source.number(),
                source_revision: event.source_revision.number(),
                binding: step
                    .binding()
                    .assignments()
                    .iter()
                    .map(|(a, b)| (a.number(), b.number()))
                    .collect(),
                entered: event.entered.iter().map(|k| k.number()).collect(),
                exited: event.exited.iter().map(|k| k.number()).collect(),
                altered: event.altered.iter().map(|k| k.number()).collect(),
                meta_source,
                object_progress,
            });
        }
        cursor += 1;
    }
    // Reserve result vectors and weighted edge copies before constructing them.
    units = units
        .saturating_add(states.len().saturating_mul(24))
        .saturating_add(edges.len().saturating_mul(16));
    check_units(units, limits)?;
    let weighted: Vec<_> = edges.iter().map(|e| (e.from, e.to, e.edits())).collect();
    let total_edits = bound(states.len(), &weighted, &mut budget)?;
    let edit_burst = bound(
        states.len(),
        &weighted.iter().copied().filter(|e| e.2).collect::<Vec<_>>(),
        &mut budget,
    )?;
    let meta_invocations = bound(
        states.len(),
        &edges
            .iter()
            .map(|e| (e.from, e.to, e.meta_source))
            .collect::<Vec<_>>(),
        &mut budget,
    )?;
    let meta_ids: Vec<_> = edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.meta_source)
        .map(|(i, _)| i)
        .collect();
    let mut meta_burst = bound(
        states.len(),
        &meta_ids
            .iter()
            .map(|&i| (edges[i].from, edges[i].to, true))
            .collect::<Vec<_>>(),
        &mut budget,
    )?;
    for i in &mut meta_burst.cycle_edges {
        *i = meta_ids[*i];
    }
    let mut report = Report {
        schema: if abstraction.is_some() {
            "meta-activity-analysis-v2"
        } else {
            "meta-activity-analysis-v1"
        },
        complete: true,
        abstraction,
        states: states.len(),
        edges,
        legitimate: None,
        total_edits,
        edit_burst,
        meta_invocations,
        meta_burst,
        edits_without_object_progress: None,
        recovery: None,
        productive_modulation: None,
    };
    // Translate filtered-graph witness indices back to the report's edge IDs.
    let edit_ids: Vec<_> = report
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.edits())
        .map(|(i, _)| i)
        .collect();
    for i in &mut report.edit_burst.cycle_edges {
        *i = edit_ids[*i];
    }
    if let Some(contract) = contract {
        let legitimate: Vec<_> = states.iter().map(|s| s.legitimate).collect();
        let mut successors = vec![Vec::new(); states.len()];
        for e in &report.edges {
            successors[e.from].push(e.to);
        }
        let closed = report
            .edges
            .iter()
            .all(|e| !legitimate[e.from] || legitimate[e.to]);
        let stable_output = report
            .edges
            .iter()
            .all(|e| !legitimate[e.from] || states[e.from].status == states[e.to].status);
        let ranks = crate::stability::rank_quotient(&legitimate, &successors).ok();
        let first_output = states.iter().find(|s| s.legitimate).map(|s| &s.status);
        let agreement = first_output.is_some()
            && states
                .iter()
                .filter(|s| s.legitimate)
                .all(|s| Some(&s.status) == first_output);
        let good = closed
            && (!contract.stable_when_legitimate || stable_output)
            && (!contract.require_agreement || agreement)
            && ranks.is_some();
        report.recovery = Some(Recovery {
            verdict: if good { "certified" } else { "refuted" },
            bound: if good {
                Some(ranks.as_ref().unwrap()[0].unwrap_or(0))
            } else {
                None
            },
            closed,
            stable_output,
            agreement,
        });
        let ids: Vec<_> = report
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| !legitimate[e.from] && !e.object_progress)
            .map(|(i, _)| i)
            .collect();
        let mut b = bound(
            states.len(),
            &ids.iter().map(|&i| weighted[i]).collect::<Vec<_>>(),
            &mut budget,
        )?;
        for i in &mut b.cycle_edges {
            *i = ids[*i];
        }
        report.productive_modulation = Some(if good && stable_output && b.verdict == "certified" {
            "certified"
        } else {
            "refuted"
        });
        report.edits_without_object_progress = Some(b);
        report.legitimate = Some(legitimate);
    }
    Ok(report)
}
fn count_unresolved(c: Option<&Contract>, status: &Status) -> Result<usize> {
    let Some(c) = c else { return Ok(0) };
    if status.iter().any(|s| s.len() != 1) {
        return Err(Diagnostic::new(
            ErrorKind::InvalidBinding,
            "meta progress requires singleton declared statuses",
        ));
    }
    Ok(c.requirements
        .iter()
        .zip(status)
        .filter(|(r, s)| !r.legitimate_labels.contains(&s[0].0))
        .count())
}
fn status_units(status: &Status) -> usize {
    status.iter().flatten().fold(status.len(), |n, (a, b)| {
        n.saturating_add(2)
            .saturating_add(a.len())
            .saturating_add(b.len())
    })
}
fn exhausted(s: &str) -> Diagnostic {
    Diagnostic::exhausted(LimitKind::ObservationHistory, s)
}
fn check_units(n: usize, l: Limits) -> Result<()> {
    if n > l.max_retained_units {
        Err(exhausted("meta retention bound"))
    } else {
        Ok(())
    }
}
