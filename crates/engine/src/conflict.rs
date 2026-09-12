//! Exact, bounded residual checks over one complete enabled set. No footprint
//! pruning, rematching, scheduler mutation or cross-schedule unfolding is used.
use crate::causal::{AnalysisLimits, Budget};
use crate::recording::{atom_units, AtomId, AtomRecord, RedexId};
use crate::scheduling::candidate_key;
use crate::{Event, Execution, OperationalBinding, PreparedStep};
use graph_core::{AtomKey, Diagnostic, ErrorKind, Graph, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Residual {
    Survives,
    Invalidated(Diagnostic),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    /// Indices into this snapshot's alternatives, never EventIds.
    pub first: usize,
    pub second: usize,
    pub second_after_first: Residual,
    pub first_after_second: Residual,
    /// Present only when both residuals survive. Equality fixes all original
    /// identities and pairs fresh atoms by occurrence and RHS template order.
    pub commutes: Option<bool>,
    pub corresponding_events: Option<bool>,
}
impl Pair {
    pub fn conflicts(&self) -> bool {
        matches!(self.second_after_first, Residual::Invalidated(_))
            || matches!(self.first_after_second, Residual::Invalidated(_))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSnapshot {
    pub state: u64,
    pub pre_state: BTreeMap<AtomId, AtomRecord>,
    pub alternatives: Vec<RedexId>,
    pub pairs: Vec<Pair>,
}
impl LocalSnapshot {
    /// Any exhausted enumeration, residual check or comparison aborts the entire
    /// snapshot with ResourceExhaustion. It is not a conflict/independence result.
    pub fn analyse(execution: &Execution, limits: AnalysisLimits) -> Result<Self> {
        let mut budget = Budget::new(limits);
        charge_graph(execution.graph(), &mut budget)?;
        // Charge before retaining each complete candidate. A single candidate's
        // construction still uses the independent ExecutionLimits. No partial
        // enabled set is published if the retention callback exhausts analysis.
        let mut steps = execution
            .search()
            .enabled_retaining(|step| {
                charge_graph(execution.graph(), &mut budget)?;
                for atom in step.ground_rule().left().values() {
                    let n = atom_units(atom)?;
                    budget.work(n)?;
                    budget.retain(n)?;
                }
                for atom in step.ground_rule().fresh().values() {
                    let n = atom_units(atom)?;
                    budget.work(n)?;
                    budget.retain(n)?;
                    budget.retain(n)?;
                }
                let e = step.event();
                for set in [
                    &e.consumed,
                    &e.preserved,
                    &e.produced,
                    &e.read,
                    &e.entered,
                    &e.exited,
                    &e.altered,
                ] {
                    budget.work(set.len())?;
                    budget.retain(set.len())?;
                }
                Ok(())
            })?
            .into_steps();
        steps.sort_by_cached_key(candidate_key);
        for step in &steps {
            budget.work(1)?;
            budget.retain(4)?;
            for _ in step.binding().assignments() {
                budget.work(1)?;
                budget.retain(2)?;
            }
        }
        let alternatives = steps
            .iter()
            .map(|s| RedexId::new(execution.committed_steps(), s.binding(), s.event()))
            .collect();
        let mut pairs = Vec::new();
        for first in 0..steps.len() {
            for second in first + 1..steps.len() {
                budget.work(1)?;
                budget.retain(10)?;
                let a = ordered(execution, &steps[first], &steps[second], &mut budget)?;
                let b = ordered(execution, &steps[second], &steps[first], &mut budget)?;
                let (commutes, corresponding_events) = match (&a, &b) {
                    (Branch::Complete(ga, ea, eb), Branch::Complete(gb, fb, fa)) => {
                        let mut mapping = BTreeMap::new();
                        for (key, _) in execution.graph().iter() {
                            budget.work(1)?;
                            budget.retain(2)?;
                            mapping.insert(key, key);
                        }
                        for (left, right) in [(ea, fa), (eb, fb)] {
                            if left.produced.len() != right.produced.len() {
                                return Err(Diagnostic::new(
                                    ErrorKind::InvalidConfiguration,
                                    "same residual changed RHS fresh cardinality",
                                ));
                            }
                            for (&x, &y) in left.produced.iter().zip(&right.produced) {
                                budget.work(1)?;
                                budget.retain(2)?;
                                mapping.insert(x, y);
                            }
                        }
                        (
                            Some(equal_graphs(ga, gb, &mapping, &mut budget)?),
                            Some(
                                equal_events(ea, fa, &mapping, &mut budget)?
                                    && equal_events(eb, fb, &mapping, &mut budget)?,
                            ),
                        )
                    }
                    _ => (None, None),
                };
                pairs.push(Pair {
                    first,
                    second,
                    second_after_first: a.status(),
                    first_after_second: b.status(),
                    commutes,
                    corresponding_events,
                });
            }
        }
        Ok(Self {
            state: execution.committed_steps(),
            pre_state: execution
                .graph()
                .iter()
                .map(|(k, a)| (k.into(), a.into()))
                .collect(),
            alternatives,
            pairs,
        })
    }
}
enum Branch {
    Complete(Graph, Event, Event),
    Invalid(Diagnostic),
}
impl Branch {
    fn status(&self) -> Residual {
        match self {
            Self::Complete(..) => Residual::Survives,
            Self::Invalid(d) => Residual::Invalidated(d.clone()),
        }
    }
}
fn charge_graph(graph: &Graph, budget: &mut Budget) -> Result<()> {
    budget.work(1)?;
    for (_, a) in graph.iter() {
        let n = atom_units(a)?;
        budget.work(n)?;
        budget.retain(n)?;
    }
    Ok(())
}
fn ordered(
    original: &Execution,
    first: &PreparedStep,
    second: &PreparedStep,
    budget: &mut Budget,
) -> Result<Branch> {
    charge_graph(original.graph(), budget)?;
    let mut hypothetical = Execution::new(original.graph().clone(), original.limits())?;
    budget.work(1)?;
    let a = hypothetical.prepare(first.binding().clone())?;
    let ea = hypothetical.commit(a)?;
    charge_graph(hypothetical.graph(), budget)?;
    let source = hypothetical
        .view()?
        .rule(second.binding().source())
        .map(|r| (r.revision, r.active));
    // Revision identity is part of an occurrence even if a different live rule
    // could happen to accept the same old binding after the first transaction.
    if source.is_none_or(|(revision, active)| revision != second.event().source_revision || !active)
    {
        return Ok(Branch::Invalid(Diagnostic::new(
            ErrorKind::StaleCandidate,
            "residual source revision or activation changed",
        )));
    }
    budget.work(1)?;
    let b = match hypothetical.prepare(OperationalBinding::new(
        second.binding().source(),
        second.binding().assignments().clone(),
    )) {
        Ok(b) => b,
        Err(d) if d.kind() == ErrorKind::ResourceExhaustion => return Err(d),
        Err(d) => return Ok(Branch::Invalid(d)),
    };
    let eb = hypothetical.commit(b)?;
    charge_graph(hypothetical.graph(), budget)?;
    Ok(Branch::Complete(hypothetical.graph().clone(), ea, eb))
}
fn equal_graphs(
    a: &Graph,
    b: &Graph,
    map: &BTreeMap<AtomKey, AtomKey>,
    budget: &mut Budget,
) -> Result<bool> {
    if a.len() != b.len() {
        return Ok(false);
    }
    // The per-occurrence fresh ranges are disjoint, hence this mapping is
    // injective on the resulting graph. Check surjectivity via equal cardinality.
    for (key, atom) in a.iter() {
        budget.work(atom_units(atom)?)?;
        let Some(target) = map.get(&key).and_then(|k| b.get(*k)) else {
            return Ok(false);
        };
        if atom.kind != target.kind
            || atom.fields.len() != target.fields.len()
            || atom
                .fields
                .iter()
                .zip(&target.fields)
                .any(|(x, y)| map.get(x) != Some(y))
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn equal_events(
    a: &Event,
    b: &Event,
    map: &BTreeMap<AtomKey, AtomKey>,
    budget: &mut Budget,
) -> Result<bool> {
    budget.work(2)?;
    if map.get(&a.source) != Some(&b.source)
        || map.get(&a.source_revision) != Some(&b.source_revision)
    {
        return Ok(false);
    }
    for (x, y) in [
        (&a.consumed, &b.consumed),
        (&a.preserved, &b.preserved),
        (&a.produced, &b.produced),
        (&a.read, &b.read),
        (&a.entered, &b.entered),
        (&a.exited, &b.exited),
        (&a.altered, &b.altered),
    ] {
        if x.len() != y.len() {
            return Ok(false);
        }
        for key in x {
            budget.work(1)?;
            if !map.get(key).is_some_and(|k| y.contains(k)) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
