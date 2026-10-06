//! Bounded positive-resource order, not complete enabling causality or an unfolding.
pub mod provenance;
use crate::recording::{AtomId, EventId, RecordedEvent};
use graph_core::{Diagnostic, ErrorKind, LimitKind, Result};
use std::collections::{BTreeMap, BTreeSet};

pub const RELATION: &str = "positive_resource_order";
pub const LIMITATION: &str = "Positive footprints only: dangling, visibility, quotation, global validity and lifecycle/guard condition-establishment provenance is not recorded; absent edges do not establish independence.";

#[derive(Clone, Copy, Debug)]
pub struct AnalysisLimits {
    pub max_work: usize,
    pub max_units: usize,
}
impl Default for AnalysisLimits {
    fn default() -> Self {
        Self {
            max_work: 2_000_000,
            max_units: 2_000_000,
        }
    }
}
pub(crate) struct Budget {
    work: usize,
    units: usize,
}
impl Budget {
    pub(crate) fn new(l: AnalysisLimits) -> Self {
        Self {
            work: l.max_work,
            units: l.max_units,
        }
    }
    pub(crate) fn work(&mut self, n: usize) -> Result<()> {
        self.work = self.work.checked_sub(n).ok_or_else(|| {
            Diagnostic::exhausted(LimitKind::AnalysisWork, "analysis work exhausted")
        })?;
        Ok(())
    }
    pub(crate) fn retain(&mut self, n: usize) -> Result<()> {
        self.units = self.units.checked_sub(n).ok_or_else(|| {
            Diagnostic::exhausted(LimitKind::AnalysisStorage, "analysis storage exhausted")
        })?;
        Ok(())
    }
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(deny_unknown_fields)]
pub enum DependencyKind {
    ProduceRead,
    ProduceConsume,
    ReadConsume,
    ExactRevision,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct Witness {
    pub kind: DependencyKind,
    pub atom: AtomId,
}
pub type Edges = BTreeMap<(EventId, EventId), BTreeSet<Witness>>;

/// Direct witnesses are retained even when another path connects the endpoints.
/// Construction incrementally indexes producers and previous readers. It returns
/// no partial result on exhaustion. Input records must come from checked recording;
/// this is not the portable trace validator required for untrusted import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyOrder {
    events: BTreeSet<EventId>,
    edges: Edges,
}
impl DependencyOrder {
    pub fn relation(&self) -> &'static str {
        RELATION
    }
    pub fn limitation(&self) -> &'static str {
        LIMITATION
    }
    pub fn edges(&self) -> &Edges {
        &self.edges
    }
    pub fn events(&self) -> &BTreeSet<EventId> {
        &self.events
    }
    pub fn extract(records: &[RecordedEvent], limits: AnalysisLimits) -> Result<Self> {
        let mut budget = Budget::new(limits);
        let mut events = BTreeSet::new();
        let mut edges = BTreeMap::new();
        let mut producers = BTreeMap::new();
        let mut readers: BTreeMap<AtomId, BTreeSet<EventId>> = BTreeMap::new();
        for (index, e) in records.iter().enumerate() {
            budget.work(1)?;
            budget.retain(1)?;
            if usize::try_from(e.id.0).ok() != Some(index) {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidConfiguration,
                    "events must be contiguous from zero",
                ));
            }
            events.insert(e.id);
            for (set, kind) in [
                (&e.footprint.read, DependencyKind::ProduceRead),
                (&e.footprint.consumed, DependencyKind::ProduceConsume),
            ] {
                for atom in set {
                    budget.work(1)?;
                    if let Some(&producer) = producers.get(atom) {
                        edge(
                            &mut edges,
                            producer,
                            e.id,
                            Witness { kind, atom: *atom },
                            &mut budget,
                        )?;
                        if kind == DependencyKind::ProduceRead && *atom == e.redex.revision {
                            edge(
                                &mut edges,
                                producer,
                                e.id,
                                Witness {
                                    kind: DependencyKind::ExactRevision,
                                    atom: *atom,
                                },
                                &mut budget,
                            )?;
                        }
                    }
                }
            }
            for atom in &e.footprint.consumed {
                budget.work(1)?;
                if let Some(previous) = readers.remove(atom) {
                    for reader in previous {
                        edge(
                            &mut edges,
                            reader,
                            e.id,
                            Witness {
                                kind: DependencyKind::ReadConsume,
                                atom: *atom,
                            },
                            &mut budget,
                        )?;
                    }
                }
                producers.remove(atom);
            }
            for atom in &e.footprint.read {
                budget.work(1)?;
                if !e.footprint.consumed.contains(atom) {
                    budget.retain(2)?;
                    readers.entry(*atom).or_default().insert(e.id);
                }
            }
            for atom in &e.footprint.produced {
                budget.work(1)?;
                budget.retain(2)?;
                producers.insert(*atom, e.id);
            }
        }
        Ok(Self { events, edges })
    }
    /// Strict closure; unlike reflexive reachability, no (e,e) pairs are included.
    pub fn closure(&self, limits: AnalysisLimits) -> Result<BTreeSet<(EventId, EventId)>> {
        let mut budget = Budget::new(limits);
        let mut adjacency: BTreeMap<EventId, Vec<EventId>> = BTreeMap::new();
        for &(a, b) in self.edges.keys() {
            budget.work(1)?;
            budget.retain(2)?;
            adjacency.entry(a).or_default().push(b);
        }
        let mut result = BTreeSet::new();
        for &start in &self.events {
            budget.work(1)?;
            budget.retain(1)?;
            let mut pending = vec![start];
            let mut seen = BTreeSet::new();
            while let Some(a) = pending.pop() {
                if let Some(next) = adjacency.get(&a) {
                    for &b in next {
                        budget.work(1)?;
                        if !seen.contains(&b) {
                            budget.retain(4)?;
                            seen.insert(b);
                            pending.push(b);
                            result.insert((start, b));
                        }
                    }
                }
            }
        }
        Ok(result)
    }
    pub fn ancestors(&self, event: EventId, limits: AnalysisLimits) -> Result<BTreeSet<EventId>> {
        self.query(event, limits, true)
    }
    pub fn descendants(&self, event: EventId, limits: AnalysisLimits) -> Result<BTreeSet<EventId>> {
        self.query(event, limits, false)
    }
    fn query(
        &self,
        event: EventId,
        limits: AnalysisLimits,
        ancestors: bool,
    ) -> Result<BTreeSet<EventId>> {
        if !self.events.contains(&event) {
            return Err(Diagnostic::new(ErrorKind::InvalidBinding, "unknown event"));
        }
        Ok(self
            .closure(limits)?
            .into_iter()
            .filter_map(|(a, b)| {
                if ancestors && b == event {
                    Some(a)
                } else if !ancestors && a == event {
                    Some(b)
                } else {
                    None
                }
            })
            .collect())
    }
}
fn edge(
    edges: &mut Edges,
    a: EventId,
    b: EventId,
    witness: Witness,
    budget: &mut Budget,
) -> Result<()> {
    budget.work(1)?;
    if a >= b {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "dependency must point forward",
        ));
    }
    if !edges.contains_key(&(a, b)) {
        budget.retain(2)?;
    }
    if !edges.get(&(a, b)).is_some_and(|w| w.contains(&witness)) {
        budget.retain(2)?;
    }
    edges.entry((a, b)).or_default().insert(witness);
    Ok(())
}
