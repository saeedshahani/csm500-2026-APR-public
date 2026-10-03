//! Exact-occurrence, order-preserving deletion interventions on verified traces.
//! This is a bounded experimental explanation, not an unfolding or actual-cause logic.
use crate::causal::{AnalysisLimits, Budget};
use crate::recording::{AtomId, EventId, RecordedEvent};
use crate::trace::VerifiedTrace;
use crate::{Execution, OperationalBinding, PreparedStep};
use graph_core::{AtomKey, Diagnostic, ErrorKind, LimitKind, Result};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "reflective-occurrence-interventions-v1";
pub const LIMITATION: &str = "Complete only for order-preserving subsets of this recorded prefix. Prefix events keep exact source revisions, transported bindings and checked effects. The target query explicitly chooses exact occurrence or existential source-revision admission. Not arbitrary schedules, prefix rematching, structural-model actual causality, independence or cache-invalidation evidence. Minimal supports are not an upward-closed enabling formula.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Query {
    ExactOccurrence,
    /// Existential matching is allowed only for the target, never prefix events.
    SourceRevision,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Explicit exponential bound; values greater than 12 are unsupported.
    pub max_prefix: usize,
    /// Bookkeeping work/storage only. Each native preparation separately uses
    /// the verified trace's execution bounds; these are not wall-clock limits.
    pub analysis: AnalysisLimits,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_prefix: 8,
            analysis: AnalysisLimits::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind")]
pub enum Outcome {
    Admitted,
    TargetUnavailable { reason: String },
    TargetRejected { reason: String },
    InfeasiblePrefix { at: EventId, reason: String },
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Trial {
    pub retained: Vec<EventId>,
    pub outcome: Outcome,
    /// One admitted target binding, expressed in ORIGINAL trace identities.
    pub witness_binding: Option<Vec<(AtomId, AtomId)>>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub limitation: &'static str,
    pub initial_sha256: String,
    pub target: EventId,
    pub query: Query,
    pub trials: Vec<Trial>,
    /// Alternative exact successful selections, not sufficient in all supersets.
    pub minimal_supports: Vec<Vec<EventId>>,
    pub necessary_in_this_universe: Vec<EventId>,
    pub native_queries: usize,
}

enum Attempt {
    Ready(Box<PreparedStep>),
    Unavailable(String),
    Rejected(String),
}

fn prepare(
    run: &Execution,
    event: &RecordedEvent,
    names: &BTreeMap<AtomId, AtomKey>,
    budget: &mut Budget,
    calls: &mut usize,
) -> Result<Attempt> {
    budget.work(event.redex.binding.len().saturating_add(1))?;
    let live = |id: &AtomId| {
        names
            .get(id)
            .copied()
            .filter(|k| run.graph().get(*k).is_some())
    };
    let Some(source) = live(&event.redex.source) else {
        return Ok(Attempt::Unavailable("recorded source is absent".into()));
    };
    let Some(rule) = run.configuration().rules.get(&source) else {
        return Ok(Attempt::Unavailable(
            "recorded source is not a live rule".into(),
        ));
    };
    if live(&event.redex.revision) != Some(rule.revision) {
        return Ok(Attempt::Unavailable(
            "exact source revision is absent or replaced".into(),
        ));
    }
    let mut binding = BTreeMap::new();
    for (a, b) in &event.redex.binding {
        let (Some(a), Some(b)) = (live(a), live(b)) else {
            return Ok(Attempt::Unavailable(
                "recorded binding identity is absent".into(),
            ));
        };
        budget.retain(2)?;
        binding.insert(a, b);
    }
    *calls += 1;
    match run.prepare(OperationalBinding::new(source, binding)) {
        Ok(step) => Ok(Attempt::Ready(Box::new(step))),
        Err(e) if e.kind() == ErrorKind::ResourceExhaustion => Err(e),
        Err(e) => Ok(Attempt::Rejected(e.to_string())),
    }
}

// Fresh allocation is ordered by RHS template identity. Omitting unrelated
// producers can shift numeric IDs; pair by allocation order, then CHECK the
// complete resulting fresh records and ground effect. Never guess by labels.
fn extend_names(
    event: &RecordedEvent,
    step: &PreparedStep,
    names: &mut BTreeMap<AtomId, AtomKey>,
    budget: &mut Budget,
) -> Result<()> {
    let unsupported = || {
        Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "unsupported intervention: recorded ground effect does not transport exactly",
        )
    };
    if event.fresh.len() != step.ground_rule().fresh().len() {
        return Err(unsupported());
    }
    for (old, new) in event.fresh.keys().zip(step.ground_rule().fresh().keys()) {
        budget.work(1)?;
        budget.retain(2)?;
        if names.insert(*old, *new).is_some() {
            return Err(unsupported());
        }
    }
    for (old, atom) in &event.fresh {
        budget.work(1 + atom.fields.len())?;
        let fresh = &step.ground_rule().fresh()[&names[old]];
        if atom.kind != fresh.kind
            || atom
                .fields
                .iter()
                .map(|id| names.get(id).copied())
                .collect::<Option<Vec<_>>>()
                .as_ref()
                != Some(&fresh.fields)
        {
            return Err(unsupported());
        }
    }
    for (old, new) in [
        (&event.footprint.consumed, &step.event().consumed),
        (&event.footprint.preserved, &step.event().preserved),
    ] {
        budget.work(old.len())?;
        budget.retain(old.len())?;
        if old
            .iter()
            .map(|id| names.get(id).copied())
            .collect::<Option<BTreeSet<_>>>()
            .as_ref()
            != Some(new)
        {
            return Err(unsupported());
        }
    }
    Ok(())
}

/// Enumerate the entire selected finite universe, or return an error without a
/// partial report. Original trace/graph and allocator are never mutated.
pub fn analyse(trace: &VerifiedTrace, target: EventId, limits: Limits) -> Result<Report> {
    analyse_query(trace, target, Query::ExactOccurrence, limits)
}

pub fn analyse_query(
    trace: &VerifiedTrace,
    target: EventId,
    query: Query,
    limits: Limits,
) -> Result<Report> {
    let doc = trace.document();
    let n = usize::try_from(target.0).map_err(|_| {
        Diagnostic::new(ErrorKind::InvalidConfiguration, "target event out of range")
    })?;
    if n >= doc.events.len() {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "target event out of range",
        ));
    }
    if n > limits.max_prefix || limits.max_prefix > 12 {
        return Err(Diagnostic::exhausted(
            LimitKind::AnalysisWork,
            "occurrence intervention prefix bound exceeded (hard maximum 12)",
        ));
    }
    let mut budget = Budget::new(limits.analysis);
    let mut trials = Vec::new();
    let mut successful = Vec::new();
    let mut calls = 0;
    let native = doc.settings.execution()?;
    for mask in 0..(1usize << n) {
        let initial_units = doc.initial.atoms.values().try_fold(0usize, |sum, atom| {
            let label = match &atom.kind {
                graph_core::Kind::Constructor(s)
                | graph_core::Kind::Record { label: Some(s), .. } => s.len(),
                _ => 0,
            };
            sum.checked_add(
                1usize
                    .saturating_add(atom.fields.len())
                    .saturating_add(label),
            )
            .ok_or_else(|| {
                Diagnostic::exhausted(LimitKind::AnalysisStorage, "initial graph size overflow")
            })
        })?;
        budget.work(initial_units.saturating_add(n + 1))?;
        budget.retain(initial_units)?;
        budget.retain(
            doc.initial
                .atoms
                .len()
                .saturating_mul(3)
                .saturating_add(n + 2),
        )?;
        let graph = doc.initial.restore(&native.graph)?;
        let mut names: BTreeMap<_, _> = graph.iter().map(|(k, _)| (AtomId::from(k), k)).collect();
        let mut run = Execution::new(graph, native)?;
        let retained: Vec<_> = (0..n)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| EventId(i as u64))
            .collect();
        let mut outcome = None;
        for id in &retained {
            let event = &doc.events[id.0 as usize];
            match prepare(&run, event, &names, &mut budget, &mut calls)? {
                Attempt::Ready(step) => {
                    extend_names(event, &step, &mut names, &mut budget)?;
                    run.commit(*step)?;
                }
                Attempt::Unavailable(reason) | Attempt::Rejected(reason) => {
                    outcome = Some(Outcome::InfeasiblePrefix { at: *id, reason });
                    break;
                }
            }
        }
        let mut witness_binding = None;
        let outcome = match outcome {
            Some(outcome) => outcome,
            None => {
                match target_attempt(&run, &doc.events[n], &names, query, &mut budget, &mut calls)?
                {
                    Attempt::Ready(step) => {
                        if query == Query::ExactOccurrence {
                            extend_names(&doc.events[n], &step, &mut names, &mut budget)?;
                        }
                        budget.work(names.len() + step.binding().assignments().len())?;
                        budget.retain(
                            names.len().saturating_mul(2)
                                + step.binding().assignments().len().saturating_mul(2),
                        )?;
                        let inverse: BTreeMap<_, _> = names.iter().map(|(a, b)| (*b, *a)).collect();
                        witness_binding = Some(
                            step.binding()
                                .assignments()
                                .iter()
                                .map(|(a, b)| (inverse[a], inverse[b]))
                                .collect(),
                        );
                        successful.push(mask);
                        Outcome::Admitted
                    }
                    Attempt::Unavailable(reason) => Outcome::TargetUnavailable { reason },
                    Attempt::Rejected(reason) => Outcome::TargetRejected { reason },
                }
            }
        };
        let reason_bytes = match &outcome {
            Outcome::Admitted => 0,
            Outcome::TargetUnavailable { reason }
            | Outcome::TargetRejected { reason }
            | Outcome::InfeasiblePrefix { reason, .. } => reason.len(),
        };
        budget.retain(reason_bytes)?;
        trials.push(Trial {
            retained,
            outcome,
            witness_binding,
        });
    }
    if successful.last() != Some(&((1usize << n) - 1)) {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "full recorded prefix failed intervention replay",
        ));
    }
    let mut minimal_supports = Vec::new();
    for &mask in &successful {
        let mut minimal = true;
        for &other in &successful {
            budget.work(1)?;
            if other != mask && other & mask == other {
                minimal = false;
                break;
            }
        }
        if minimal {
            budget.retain(n + 1)?;
            minimal_supports.push(trials[mask].retained.clone());
        }
    }
    let common = successful
        .iter()
        .fold((1usize << n) - 1, |acc, mask| acc & mask);
    budget.retain(n)?;
    Ok(Report {
        schema: SCHEMA,
        limitation: LIMITATION,
        initial_sha256: doc.initial_sha256.clone(),
        target,
        query,
        trials,
        minimal_supports,
        necessary_in_this_universe: (0..n)
            .filter(|i| common & (1 << i) != 0)
            .map(|i| EventId(i as u64))
            .collect(),
        native_queries: calls,
    })
}

fn target_attempt(
    run: &Execution,
    event: &RecordedEvent,
    names: &BTreeMap<AtomId, AtomKey>,
    query: Query,
    budget: &mut Budget,
    calls: &mut usize,
) -> Result<Attempt> {
    if query == Query::ExactOccurrence {
        return prepare(run, event, names, budget, calls);
    }
    budget.work(1)?;
    let Some(source) = names.get(&event.redex.source).copied() else {
        return Ok(Attempt::Unavailable("recorded source is absent".into()));
    };
    let Some(rule) = run.configuration().rules.get(&source) else {
        return Ok(Attempt::Unavailable(
            "recorded source is not a live rule".into(),
        ));
    };
    if names.get(&event.redex.revision).copied() != Some(rule.revision) {
        return Ok(Attempt::Unavailable(
            "exact source revision is absent or replaced".into(),
        ));
    }
    *calls += 1;
    let enabled = run.enabled_for(source)?;
    Ok(match enabled.into_steps().into_iter().next() {
        Some(step) => Attempt::Ready(Box::new(step)),
        None => Attempt::Rejected("no admitted binding for this source revision".into()),
    })
}
