use super::{bad, Result};
use crate::causal::{AnalysisLimits, Witness};
use crate::conflict::{LocalSnapshot, Residual};
use crate::observation::{ObservationLimits, Observer};
use crate::recording::{AtomId, AtomRecord, EventId, RecordedEvent, RecordingLimits, RedexId};
use crate::runner::{Outcome, RunLimits, RunReport};
use crate::{ExecutionLimits, SearchLimits};
use graph_core::{Graph, GraphLimits};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Number(#[serde(with = "super::decimal")] pub u64);
impl Number {
    pub fn size(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| bad("integer is unsupported on this target"))
    }
}

macro_rules! bounds {
    ($name:ident,$native:ident,{$($field:ident),+})=>{
        #[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name { $(pub $field:Number),+ }
        impl From<$native> for $name {fn from(n:$native)->Self{Self{$($field:Number(n.$field as u64)),+}}}
        impl $name {
            pub fn native(self)->Result<$native>{Ok($native{$($field:self.$field.size()?),+})}
            pub fn capped(self,cap:$native)->Result<$native>{let n=self.native()?;$(if n.$field>cap.$field{return Err(super::exhausted(concat!("embedded limit exceeds import policy: ",stringify!($field))));})+ Ok(n)}
        }
    }
}
bounds!(GraphBounds,GraphLimits,{max_input_bytes,max_atoms,max_fields,max_label_bytes,max_package_depth,max_validation_work});
bounds!(SearchBounds,SearchLimits,{max_candidates,max_work});
bounds!(ObservationBounds,ObservationLimits,{max_work,max_comparison_work,max_history_units});
bounds!(RecordingBounds,RecordingLimits,{max_events,max_units,max_work});
bounds!(AnalysisBounds,AnalysisLimits,{max_work,max_units});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphData {
    pub next_identity: Number,
    pub atoms: BTreeMap<AtomId, AtomRecord>,
}
impl GraphData {
    pub fn from_graph(g: &Graph) -> Self {
        Self {
            next_identity: Number(g.next_identity()),
            atoms: g.iter().map(|(k, a)| (k.into(), a.into())).collect(),
        }
    }
    pub fn restore(&self, limits: &GraphLimits) -> Result<Graph> {
        if self.atoms.len() > limits.max_atoms {
            return Err(super::exhausted("snapshot atom count"));
        }
        let mut fields = limits.max_fields;
        let mut labels = limits.max_label_bytes;
        for a in self.atoms.values() {
            fields = fields
                .checked_sub(a.fields.len())
                .ok_or_else(|| super::exhausted("snapshot fields"))?;
            let n = match &a.kind {
                graph_core::Kind::Constructor(s)
                | graph_core::Kind::Record { label: Some(s), .. } => s.len(),
                _ => 0,
            };
            labels = labels
                .checked_sub(n)
                .ok_or_else(|| super::exhausted("snapshot labels"))?;
        }
        let atoms = self
            .atoms
            .iter()
            .map(|(k, a)| {
                (
                    k.0,
                    a.kind.clone(),
                    a.fields.iter().map(|id| id.0).collect(),
                )
            })
            .collect::<Vec<_>>();
        Graph::restore_identity_snapshot(&atoms, self.next_identity.0, limits)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverData {
    pub declaration: String,
    pub inputs: BTreeMap<String, AtomId>,
    pub nontriviality_threshold: Number,
}
impl ObserverData {
    pub fn from_observer(o: &Observer) -> Self {
        Self {
            declaration: o.canonical().into(),
            inputs: o
                .initial_inputs()
                .iter()
                .map(|(n, k)| (n.clone(), (*k).into()))
                .collect(),
            nontriviality_threshold: Number(2),
        }
    }
    pub fn restore(&self, g: &Graph, max_bytes: usize) -> Result<Observer> {
        if self.nontriviality_threshold != Number(2) {
            return Err(bad("unsupported nontriviality threshold"));
        }
        let observer = Observer::parse(
            &self.declaration,
            |n| self.inputs.get(n).and_then(|id| g.key(id.0)),
            max_bytes,
        )?;
        if Self::from_observer(&observer) != *self {
            return Err(bad(
                "observer is not canonical or has unused/missing frozen inputs",
            ));
        }
        Ok(observer)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanData {
    pub max_steps: Number,
    pub detect_cycles: bool,
    pub observation: ObservationBounds,
    pub requested: Option<AtomId>,
    pub seed: Option<Number>,
}
impl PlanData {
    pub fn limits(&self) -> Result<RunLimits> {
        Ok(RunLimits {
            max_steps: self.max_steps.0,
            detect_cycles: self.detect_cycles,
            observation: self.observation.native()?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub graph: GraphBounds,
    pub search: SearchBounds,
    pub recording: RecordingBounds,
    pub analysis: AnalysisBounds,
    pub observer: ObserverData,
    pub plan: Option<PlanData>,
}
impl Settings {
    pub fn execution(&self) -> Result<ExecutionLimits> {
        Ok(ExecutionLimits {
            graph: self.graph.native()?,
            search: self.search.native()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecurrenceData {
    pub first: Number,
    pub repeated: Number,
    pub observer: String,
    pub first_scheduler: String,
    pub repeated_scheduler: String,
    pub complete_state_equal: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportData {
    pub outcome: String,
    pub payload: Option<Number>,
    pub recurrence: Option<RecurrenceData>,
    pub scheduler: String,
}
impl From<&RunReport> for ReportData {
    fn from(r: &RunReport) -> Self {
        let cancelled = r.outcome == Outcome::Cancelled;
        Self {
            outcome: format!("{:?}", r.outcome),
            payload: if cancelled {
                None
            } else {
                r.payload.map(|n| Number(n as u64))
            },
            scheduler: r.scheduler.clone(),
            recurrence: if cancelled {
                None
            } else {
                r.recurrence.as_ref().map(|c| RecurrenceData {
                    first: Number(c.first),
                    repeated: Number(c.repeated),
                    observer: c.observer.clone(),
                    first_scheduler: c.first_scheduler.clone(),
                    repeated_scheduler: c.repeated_scheduler.clone(),
                    complete_state_equal: c.complete_state_equal,
                })
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeData {
    pub before: EventId,
    pub after: EventId,
    pub witnesses: BTreeSet<Witness>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairData {
    pub first: Number,
    pub second: Number,
    pub second_after_first: Option<String>,
    pub first_after_second: Option<String>,
    pub commutes: Option<bool>,
    pub corresponding_events: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictData {
    pub pre_state_sha256: String,
    pub alternatives: Vec<RedexId>,
    pub pairs: Vec<PairData>,
}
impl ConflictData {
    pub fn from_snapshot(s: LocalSnapshot) -> Result<Self> {
        let reason = |r: Residual| match r {
            Residual::Survives => None,
            Residual::Invalidated(d) => Some(format!("{:?}: {}", d.kind(), d)),
        };
        Ok(Self {
            pre_state_sha256: super::hash_json(&s.pre_state)?,
            alternatives: s.alternatives,
            pairs: s
                .pairs
                .into_iter()
                .map(|p| PairData {
                    first: Number(p.first as u64),
                    second: Number(p.second as u64),
                    second_after_first: reason(p.second_after_first),
                    first_after_second: reason(p.first_after_second),
                    commutes: p.commutes,
                    corresponding_events: p.corresponding_events,
                })
                .collect(),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictAt {
    pub state: Number,
    pub analysis: Option<ConflictData>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointData {
    pub state: Number,
    pub graph: GraphData,
    pub scheduler: Option<String>,
    pub continuation_sha256: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceDocument {
    pub schema: String,
    pub profile: String,
    pub implementation: String,
    pub source_sha256: Option<String>,
    pub initial_sha256: String,
    pub initial: GraphData,
    pub relation: String,
    pub limitation: String,
    pub settings: Settings,
    /// recorded_prefix has no scheduler claim; yielded is resumable; finished has a report.
    pub status: String,
    pub report: Option<ReportData>,
    pub events: Vec<RecordedEvent>,
    /// null means derive, not a claim that the relation is empty.
    pub dependencies: Option<Vec<EdgeData>>,
    pub conflicts: Vec<ConflictAt>,
    pub checkpoint_interval: Number,
    pub checkpoints: Vec<CheckpointData>,
}
