//! Native committed-event history. Portable validation and replay live in [`crate::trace`].
//! IDs are local to one recording; no process-global or private slotmap identity.
//!
//! ```
//! use engine::{Execution, ExecutionLimits, OperationalBinding};
//! use engine::recording::{Recording, RecordingLimits};
//! use engine::causal::{AnalysisLimits, DependencyOrder};
//! let input = graph_core::fixture::load("h RuleHead\nz Nat\nr Record[Rank] s0=h s1=z\nv Record[Revision] s0=h\nm Record[ModeObject] s0=h\na Record[Active] s0=h")?;
//! let mut run = Execution::new(input.graph, ExecutionLimits::default())?;
//! let mut history = Recording::new(&run, RecordingLimits::default())?;
//! let candidate = run.prepare(OperationalBinding::new(input.names["h"], Default::default()))?;
//! history.commit(&mut run, candidate)?;
//! let order = DependencyOrder::extract(history.events(), AnalysisLimits::default())?;
//! assert_eq!(order.relation(), "positive_resource_order");
//! assert!(order.edges().is_empty());
//! # Ok::<(), graph_core::Diagnostic>(())
//! ```
use crate::{Event, Execution, ExecutionLimits, OperationalBinding, PreparedStep};
use graph_core::{Atom, AtomKey, Diagnostic, ErrorKind, Kind, LimitKind, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct AtomId(#[serde(with = "crate::trace::decimal")] pub u64);
impl From<AtomKey> for AtomId {
    fn from(key: AtomKey) -> Self {
        Self(key.number())
    }
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct EventId(#[serde(with = "crate::trace::decimal")] pub u64);

/// Content-addressed occurrence description, not a hash. The state index is
/// scoped to the recording/snapshot; the binding identifies the same occurrence
/// even when the enabled query is restricted to a particular source.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedexId {
    #[serde(with = "crate::trace::decimal")]
    pub state: u64,
    pub source: AtomId,
    pub revision: AtomId,
    pub binding: Vec<(AtomId, AtomId)>,
}
impl RedexId {
    pub(crate) fn new(state: u64, binding: &OperationalBinding, event: &Event) -> Self {
        Self {
            state,
            source: event.source.into(),
            revision: event.source_revision.into(),
            binding: binding
                .assignments()
                .iter()
                .map(|(a, b)| ((*a).into(), (*b).into()))
                .collect(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtomRecord {
    pub kind: Kind,
    pub fields: Vec<AtomId>,
}
impl From<&Atom> for AtomRecord {
    fn from(atom: &Atom) -> Self {
        Self {
            kind: atom.kind.clone(),
            fields: atom.fields.iter().copied().map(Into::into).collect(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footprint {
    pub consumed: BTreeSet<AtomId>,
    pub preserved: BTreeSet<AtomId>,
    pub produced: BTreeSet<AtomId>,
    pub read: BTreeSet<AtomId>,
    pub entered: BTreeSet<AtomId>,
    pub exited: BTreeSet<AtomId>,
    pub altered: BTreeSet<AtomId>,
}
impl From<&Event> for Footprint {
    fn from(e: &Event) -> Self {
        let ids = |s: &BTreeSet<AtomKey>| s.iter().copied().map(Into::into).collect();
        Self {
            consumed: ids(&e.consumed),
            preserved: ids(&e.preserved),
            produced: ids(&e.produced),
            read: ids(&e.read),
            entered: ids(&e.entered),
            exited: ids(&e.exited),
            altered: ids(&e.altered),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedEvent {
    pub id: EventId,
    pub redex: RedexId,
    #[serde(with = "crate::trace::decimal")]
    pub post_state: u64,
    pub footprint: Footprint,
    pub fresh: BTreeMap<AtomId, AtomRecord>,
}
#[derive(Clone, Copy, Debug)]
pub struct RecordingLimits {
    pub max_events: usize,
    pub max_units: usize,
    pub max_work: usize,
}
impl Default for RecordingLimits {
    fn default() -> Self {
        Self {
            max_events: 10_000,
            max_units: 2_000_000,
            max_work: 2_000_000,
        }
    }
}
/// Units count retained identities, fields, label bytes and fixed record overhead;
/// they are conservative data-volume proxies, not measured heap bytes.
pub(crate) fn atom_units(atom: &Atom) -> Result<usize> {
    let attributes = match &atom.kind {
        Kind::Constructor(s) => s.len(),
        Kind::Record { label: Some(s), .. } => s.len(),
        Kind::PatternVariable { admits, .. } => admits.len(),
        _ => 0,
    };
    atom.fields
        .len()
        .checked_add(attributes)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(storage_error)
}
pub(crate) fn graph_units(graph: &graph_core::Graph) -> Result<usize> {
    graph.iter().try_fold(0usize, |n, (_, a)| {
        n.checked_add(atom_units(a)?).ok_or_else(storage_error)
    })
}
fn storage_error() -> Diagnostic {
    Diagnostic::exhausted(LimitKind::TraceStorage, "recording capacity exhausted")
}
fn charge(remaining: &mut usize, n: usize) -> Result<()> {
    *remaining = remaining.checked_sub(n).ok_or_else(storage_error)?;
    Ok(())
}

/// Only this owner can append: a preview is not a committed event. External
/// replacement or unrecorded commits invalidate this recorder's baseline stamp.
#[derive(Debug)]
pub struct Recording {
    baseline: Rc<()>,
    next_step: u64,
    limits: RecordingLimits,
    units: usize,
    initial: BTreeMap<AtomId, AtomRecord>,
    events: Vec<RecordedEvent>,
    initial_allocator: u64,
    execution_limits: ExecutionLimits,
}
impl Recording {
    pub fn new(execution: &Execution, limits: RecordingLimits) -> Result<Self> {
        if execution.committed_steps() != 0 {
            return Err(Diagnostic::new(
                ErrorKind::InvalidConfiguration,
                "recording must begin at the initial state",
            ));
        }
        let mut remaining = limits.max_units;
        let mut work = limits.max_work;
        for (_, atom) in execution.graph().iter() {
            let n = atom_units(atom)?;
            charge(&mut remaining, n)?;
            charge(&mut work, n)?;
        }
        Ok(Self {
            baseline: execution.stamp().0,
            next_step: 0,
            limits,
            units: limits.max_units - remaining,
            initial: execution
                .graph()
                .iter()
                .map(|(k, a)| (k.into(), a.into()))
                .collect(),
            events: Vec::new(),
            initial_allocator: execution.graph().next_identity(),
            execution_limits: execution.limits(),
        })
    }
    pub fn limits(&self) -> RecordingLimits {
        self.limits
    }
    pub fn initial_allocator(&self) -> u64 {
        self.initial_allocator
    }
    pub fn execution_limits(&self) -> ExecutionLimits {
        self.execution_limits
    }
    pub(crate) fn fork(&self, execution: &Execution) -> Self {
        Self {
            baseline: execution.stamp().0,
            next_step: self.next_step,
            limits: self.limits,
            units: self.units,
            initial: self.initial.clone(),
            events: self.events.clone(),
            initial_allocator: self.initial_allocator,
            execution_limits: self.execution_limits,
        }
    }
    pub fn units(&self) -> usize {
        self.units
    }
    pub fn initial(&self) -> &BTreeMap<AtomId, AtomRecord> {
        &self.initial
    }
    pub fn events(&self) -> &[RecordedEvent] {
        &self.events
    }
    /// Historical lookup includes deleted atoms without storing every full state.
    pub fn atom(&self, id: AtomId) -> Option<&AtomRecord> {
        self.initial
            .get(&id)
            .or_else(|| self.events.iter().find_map(|e| e.fresh.get(&id)))
    }
    pub fn commit(&mut self, execution: &mut Execution, prepared: PreparedStep) -> Result<Event> {
        execution.check_prepared(&prepared)?;
        let (baseline, step) = execution.stamp();
        if !Rc::ptr_eq(&baseline, &self.baseline) || step != self.next_step {
            return Err(Diagnostic::new(
                ErrorKind::StaleCandidate,
                "recording is not attached to this execution state",
            ));
        }
        if self.events.len() >= self.limits.max_events {
            return Err(storage_error());
        }
        let e = prepared.event();
        let mut remaining = self.limits.max_units - self.units;
        let mut work = self.limits.max_work;
        let mut add = |n| {
            charge(&mut remaining, n)?;
            charge(&mut work, n)
        };
        add(8)?;
        for _ in prepared.binding().assignments() {
            add(2)?;
        }
        for set in [
            &e.consumed,
            &e.preserved,
            &e.produced,
            &e.read,
            &e.entered,
            &e.exited,
            &e.altered,
        ] {
            add(set.len())?;
        }
        for atom in prepared.ground_rule().fresh().values() {
            add(atom_units(atom)?)?;
        }
        // The opaque PreparedStep carries admitted freshness and lifecycle; these
        // checks also protect the historical lookup's pre-state interpretation.
        for key in e.consumed.iter().chain(&e.preserved).chain(&e.read) {
            execution.graph().atom(*key)?;
        }
        if e.produced
            .iter()
            .any(|k| execution.graph().get(*k).is_some())
        {
            return Err(Diagnostic::new(
                ErrorKind::InvalidConfiguration,
                "produced atom is not fresh",
            ));
        }
        let post_state = step.checked_add(1).ok_or_else(storage_error)?;
        let record = RecordedEvent::preview(step, &prepared)?;
        let units = self.limits.max_units - remaining;
        // No ordinary fallible recording operation follows the shared commit.
        let event = execution.commit(prepared)?;
        self.events.push(record);
        self.units = units;
        self.next_step = post_state;
        Ok(event)
    }
}
impl RecordedEvent {
    pub(crate) fn preview(step: u64, prepared: &PreparedStep) -> Result<Self> {
        Ok(Self {
            id: EventId(step),
            redex: RedexId::new(step, prepared.binding(), prepared.event()),
            post_state: step.checked_add(1).ok_or_else(storage_error)?,
            footprint: prepared.event().into(),
            fresh: prepared
                .ground_rule()
                .fresh()
                .iter()
                .map(|(k, a)| ((*k).into(), a.into()))
                .collect(),
        })
    }
}
