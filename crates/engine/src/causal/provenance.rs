//! Selected birth/read provenance of a replay-verified history.
//! This analysis is observational: it never admits steps or certifies alternatives.
use super::{AnalysisLimits, Budget};
use crate::recording::{AtomId, EventId, RedexId};
use crate::trace::{AnalysisBounds, Number, VerifiedTrace, IMPLEMENTATION};
use graph_core::{Diagnostic, ErrorKind, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

pub const RELATION: &str = "selected_birth_read_ancestry_v1";
pub const LIMITATION: &str = "Positive birth/read ancestry in one verified history. Initial atoms have no event creator. Missing support does not establish independence, negative enabling, intervention causality or necessity across alternative executions.";

/// Explicit finite labelling. Class assignments are assertions of the analyst,
/// not inferred equivalence of events or execution semantics. Omitted events
/// have no selected labels. The exact assignment is retained in the report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "granularity", content = "assignment", rename_all = "snake_case")]
pub enum Selection {
    AllOccurrences,
    Occurrences(BTreeSet<EventId>),
    Classes(BTreeMap<EventId, BTreeSet<String>>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Label {
    Occurrence(EventId),
    Class(String),
}
type Labels = BTreeSet<Number>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputOrigin {
    pub atom: AtomId,
    pub created_by: EventId,
    pub selected_direct_creator: Labels,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Observation {
    pub event: EventId,
    pub redex: RedexId,
    pub selected_labels: Labels,
    pub selected_birth_read_ancestry: Labels,
    pub selected_pre_action_history: Labels,
    /// Only non-initial read atoms: initial origins and their support are empty.
    pub input_origins: Vec<InputOrigin>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub relation: &'static str,
    pub scope: &'static str,
    pub alternative_execution_coverage: &'static str,
    pub global_necessity: &'static str,
    pub limitation: &'static str,
    /// Hash of compact canonical TraceDocument JSON, not the input file bytes.
    pub canonical_trace_sha256: String,
    pub implementation: &'static str,
    pub analysis_limits: AnalysisBounds,
    pub selection: Selection,
    /// Stable sorted dictionary. Observation sets contain its zero-based indices.
    pub labels: Vec<Label>,
    pub observations: Vec<Observation>,
}

fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new(ErrorKind::InvalidConfiguration, message)
}
fn copy(set: &Labels, budget: &mut Budget) -> Result<Labels> {
    budget.work(set.len())?;
    budget.retain(set.len())?;
    Ok(set.clone())
}
fn union(target: &mut Labels, source: &Labels, budget: &mut Budget) -> Result<()> {
    for value in source {
        budget.work(1)?;
        if !target.contains(value) {
            budget.retain(1)?;
            target.insert(*value);
        }
    }
    Ok(())
}
struct HashWriter<'a> {
    digest: Sha256,
    budget: &'a mut Budget,
}
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.budget.work(bytes.len()).map_err(io::Error::other)?;
        self.digest.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Atomic result: exhaustion returns an error and cannot publish a partial report.
/// Limits count cumulative allocated annotation units and operation/hashed-byte
/// work, not exact heap bytes or CPU instructions. Trace validation has its own
/// separately enforced limits. No deserialiser treats a report as authority.
pub fn analyse(
    trace: &VerifiedTrace,
    selection: &Selection,
    limits: AnalysisLimits,
) -> Result<Report> {
    let doc = trace.document();
    let mut budget = Budget::new(limits);
    budget.retain(128)?;
    let mut dictionary = BTreeSet::new();
    let mut assigned = BTreeMap::<EventId, BTreeSet<Label>>::new();
    let mut assign = |event: EventId, label: Label| -> Result<()> {
        budget.work(1)?;
        if usize::try_from(event.0)
            .ok()
            .filter(|i| *i < doc.events.len())
            .is_none()
        {
            return Err(invalid("selected event does not belong to this trace"));
        }
        let bytes = match &label {
            Label::Class(s) => {
                if s.is_empty() || s.len() > 256 || s.chars().any(char::is_control) {
                    return Err(invalid(
                        "class label must contain 1..256 UTF-8 bytes without controls",
                    ));
                }
                s.len()
            }
            Label::Occurrence(_) => 0,
        };
        budget.work(bytes)?;
        // Assignment, dictionary, returned selection and dictionary copies.
        budget.retain(8 + 4 * bytes)?;
        dictionary.insert(label.clone());
        assigned.entry(event).or_default().insert(label);
        Ok(())
    };
    match selection {
        Selection::AllOccurrences => {
            for event in &doc.events {
                assign(event.id, Label::Occurrence(event.id))?;
            }
        }
        Selection::Occurrences(events) => {
            for event in events {
                assign(*event, Label::Occurrence(*event))?;
            }
        }
        Selection::Classes(events) => {
            for (event, labels) in events {
                if labels.is_empty() {
                    return Err(invalid(
                        "omit unlabelled events instead of empty class assignments",
                    ));
                }
                for label in labels {
                    if label.len() > 256 {
                        return Err(invalid("class label exceeds 256 UTF-8 bytes"));
                    }
                    assign(*event, Label::Class(label.clone()))?;
                }
            }
        }
    }
    let labels: Vec<_> = dictionary.into_iter().collect();
    let mut tags = BTreeMap::new();
    for (event, values) in assigned {
        let mut ids = BTreeSet::new();
        for value in values {
            budget.work(1)?;
            budget.retain(1)?;
            ids.insert(Number(
                labels.binary_search(&value).expect("registered label") as u64,
            ));
        }
        tags.insert(event, ids);
    }
    // Hash without allocating a second serialised trace; hashing is budgeted.
    let mut writer = HashWriter {
        digest: Sha256::new(),
        budget: &mut budget,
    };
    serde_json::to_writer(&mut writer, doc).map_err(|_| {
        Diagnostic::exhausted(
            graph_core::LimitKind::AnalysisWork,
            "provenance trace hashing exhausted",
        )
    })?;
    let canonical_trace_sha256 = format!("{:x}", writer.digest.finalize());

    let empty = Labels::new();
    let mut origins = BTreeMap::<AtomId, (EventId, Labels)>::new();
    let mut history = Labels::new();
    let mut observations = Vec::new();
    for event in &doc.events {
        budget.work(1 + event.redex.binding.len())?;
        budget.retain(12 + 2 * event.redex.binding.len())?;
        let selected = tags.get(&event.id).unwrap_or(&empty);
        let mut ancestry = Labels::new();
        let mut inputs = Vec::new();
        for atom in &event.footprint.read {
            budget.work(1)?;
            if let Some((creator, support)) = origins.get(atom) {
                union(&mut ancestry, support, &mut budget)?;
                budget.retain(3)?;
                inputs.push(InputOrigin {
                    atom: *atom,
                    created_by: *creator,
                    selected_direct_creator: copy(
                        tags.get(creator).unwrap_or(&empty),
                        &mut budget,
                    )?,
                });
            }
        }
        let prior = copy(&history, &mut budget)?;
        union(&mut history, selected, &mut budget)?;
        let mut support = copy(&ancestry, &mut budget)?;
        union(&mut support, selected, &mut budget)?;
        for atom in &event.footprint.consumed {
            budget.work(1)?;
            origins.remove(atom);
        }
        for atom in &event.footprint.produced {
            budget.work(1)?;
            budget.retain(3)?;
            origins.insert(*atom, (event.id, copy(&support, &mut budget)?));
        }
        observations.push(Observation {
            event: event.id,
            redex: event.redex.clone(),
            selected_labels: copy(selected, &mut budget)?,
            selected_birth_read_ancestry: ancestry,
            selected_pre_action_history: prior,
            input_origins: inputs,
        });
    }
    Ok(Report {
        schema: "selected-trace-provenance-v1",
        relation: RELATION,
        scope: "verified_recorded_prefix",
        alternative_execution_coverage: "not_explored",
        global_necessity: "unknown",
        limitation: LIMITATION,
        canonical_trace_sha256,
        implementation: IMPLEMENTATION,
        analysis_limits: limits.into(),
        selection: selection.clone(),
        labels,
        observations,
    })
}
