use crate::{apply_candidate, AdmissionFailure, Event};
use graph_core::{
    Atom, AtomKey, Diagnostic, ErrorKind, Graph, GraphLimits, LimitKind, Result, ValidatedGraph,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// A proposed template assignment, not a typed graph morphism or admitted redex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalBinding {
    source: AtomKey,
    assignments: BTreeMap<AtomKey, AtomKey>,
}
impl OperationalBinding {
    pub fn new(source: AtomKey, assignments: BTreeMap<AtomKey, AtomKey>) -> Self {
        Self {
            source,
            assignments,
        }
    }
    pub fn source(&self) -> AtomKey {
        self.source
    }
    pub fn assignments(&self) -> &BTreeMap<AtomKey, AtomKey> {
        &self.assignments
    }
}

/// A checked ground span. Left atoms have host kinds/attributes, not variable kinds.
/// The interface is the preserved subset of left; right is that subset plus fresh.
/// Fields belong to their source atom, so no independent field map is required.
/// Constructed only by successful candidate preparation, and exposed read-only.
/// Fresh identities are provisional until the containing candidate commits.
///
/// ```compile_fail
/// use engine::GroundRule;
/// use std::collections::{BTreeMap, BTreeSet};
/// let forged = GroundRule { left: BTreeMap::new(), preserved: BTreeSet::new(), fresh: BTreeMap::new() };
/// ```
#[derive(Debug)]
pub struct GroundRule {
    left: BTreeMap<AtomKey, Atom>,
    preserved: BTreeSet<AtomKey>,
    fresh: BTreeMap<AtomKey, Atom>,
}
impl GroundRule {
    pub fn left(&self) -> &BTreeMap<AtomKey, Atom> {
        &self.left
    }
    pub fn preserved(&self) -> &BTreeSet<AtomKey> {
        &self.preserved
    }
    pub fn fresh(&self) -> &BTreeMap<AtomKey, Atom> {
        &self.fresh
    }
    pub fn right(&self) -> impl Iterator<Item = (AtomKey, &Atom)> {
        self.preserved
            .iter()
            .map(|id| (*id, &self.left[id]))
            .chain(self.fresh.iter().map(|(id, atom)| (*id, atom)))
    }
}

/// Checked injective inclusion of the completed left graph into the pre-state.
/// Each key maps to itself; this is distinct from the operational template binding.
/// Valid only for the execution state of the containing prepared step.
///
/// ```compile_fail
/// use engine::GroundMatch;
/// use std::collections::BTreeSet;
/// let forged = GroundMatch { image: BTreeSet::new() };
/// ```
#[derive(Debug)]
pub struct GroundMatch {
    image: BTreeSet<AtomKey>,
}
impl GroundMatch {
    pub fn image(&self) -> &BTreeSet<AtomKey> {
        &self.image
    }
}

/// Resource bounds for one enumeration/preparation search, not one execution history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchLimits {
    /// Number of complete bindings emitted or submitted for applicability checking.
    pub max_candidates: usize,
    /// Matcher work units plus one unit and the assignment size per preparation.
    pub max_work: usize,
}
impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_candidates: 1_000,
            max_work: 1_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub graph: GraphLimits,
    pub search: SearchLimits,
}

/// An admitted but uncommitted candidate. It is not cloneable and has no unchecked
/// constructor. Consuming it in `commit` prevents applying the same value twice.
/// Multiple candidates prepared at the same state are invalidated by the first commit.
#[derive(Debug)]
pub struct PreparedStep {
    baseline: Rc<()>,
    step: u64,
    binding: OperationalBinding,
    rule: GroundRule,
    ground_match: GroundMatch,
    candidate: Graph,
    event: Event,
}
impl PreparedStep {
    pub fn binding(&self) -> &OperationalBinding {
        &self.binding
    }
    pub fn ground_rule(&self) -> &GroundRule {
        &self.rule
    }
    pub fn ground_match(&self) -> &GroundMatch {
        &self.ground_match
    }
    /// Preview only: these produced identities do not yet exist in the execution.
    pub fn event(&self) -> &Event {
        &self.event
    }
}

/// Owns one closed execution baseline. The graph is available only immutably.
/// External edits must use a separate construction buffer and `replace_graph`,
/// which validates it and starts a new history even for an identical replacement.
/// Baseline tokens are process-local capabilities, not exported IDs or graph labels.
/// The engine is single-threaded; this boundary makes no concurrency guarantee.
///
/// A complete empty-side rule supplies a minimal preparation/commit example.
/// The ledger is test syntax, separate from the authored language.
///
/// ```
/// use engine::{Execution, ExecutionLimits, OperationalBinding};
/// use graph_core::fixture;
/// use std::collections::BTreeMap;
/// let f = fixture::load("h RuleHead\nz Nat\nr Record[Rank] s0=h s1=z\nv Record[Revision] s0=h\nm Record[ModeObject] s0=h\na Record[Active] s0=h")?;
/// let binding = OperationalBinding::new(f.names["h"], BTreeMap::new());
/// let mut run = Execution::new(f.graph, ExecutionLimits::default())?;
/// let proposal = run.prepare(binding)?;
/// assert_eq!(run.committed_steps(), 0);
/// run.commit(proposal)?;
/// assert_eq!(run.committed_steps(), 1);
/// # Ok::<(), graph_core::Diagnostic>(())
/// ```
///
/// ```compile_fail
/// use engine::{Execution, ExecutionLimits};
/// use graph_core::{Graph, Kind};
/// let mut run = Execution::new(Graph::new(), ExecutionLimits::default()).unwrap();
/// run.graph().insert(Kind::Nat, &[]).unwrap();
/// ```
#[derive(Debug)]
pub struct Execution {
    graph: Graph,
    limits: ExecutionLimits,
    baseline: Rc<()>,
    steps: u64,
}
impl Execution {
    pub fn new(graph: Graph, limits: ExecutionLimits) -> Result<Self> {
        ValidatedGraph::new(&graph, &limits.graph)?;
        Ok(Self {
            graph,
            limits,
            baseline: Rc::new(()),
            steps: 0,
        })
    }
    pub fn graph(&self) -> &Graph {
        &self.graph
    }
    pub fn limits(&self) -> ExecutionLimits {
        self.limits
    }
    pub fn committed_steps(&self) -> u64 {
        self.steps
    }
    /// Revalidate and decode the current carrier on every call; no rule cache or
    /// separate rule store is maintained. The borrow prevents a concurrent commit.
    pub fn view(&self) -> Result<ValidatedGraph<'_>> {
        ValidatedGraph::new(&self.graph, &self.limits.graph)
    }
    /// Rejected replacement leaves the graph, baseline and prepared candidates intact.
    /// Successful replacement invalidates all old candidates and resets the step count.
    pub fn replace_graph(&mut self, graph: Graph) -> Result<()> {
        ValidatedGraph::new(&graph, &self.limits.graph)?;
        self.graph = graph;
        self.baseline = Rc::new(());
        self.steps = 0;
        Ok(())
    }
    /// Begin one bounded search over this immutable state.
    pub fn search(&self) -> CandidateSearch<'_> {
        CandidateSearch {
            execution: self,
            candidates: 0,
            work: 0,
            exhausted: None,
        }
    }
    /// Convenience for a single supplied binding. Use one shared `search` when
    /// enumerating multiple candidates so cumulative limits cannot be reset per match.
    pub fn prepare(&self, binding: OperationalBinding) -> Result<PreparedStep> {
        self.search().prepare(binding)
    }
    /// Commit only a candidate admitted for this exact baseline and step.
    /// No-op transitions still increment the step counter and invalidate old proposals.
    /// Full preparation already checked validity, lifecycle and GC. No graph mutation
    /// is possible through this API between preparation and a successful stamped commit.
    pub fn commit(&mut self, prepared: PreparedStep) -> Result<Event> {
        if !Rc::ptr_eq(&self.baseline, &prepared.baseline) || self.steps != prepared.step {
            return Err(Diagnostic::new(
                ErrorKind::StaleCandidate,
                "candidate belongs to a different execution state",
            )
            .at_atom(prepared.binding.source));
        }
        let next = self.steps.checked_add(1).ok_or_else(|| {
            Diagnostic::exhausted(
                LimitKind::ExecutionSteps,
                "execution step counter exhausted",
            )
        })?;
        self.graph = prepared.candidate;
        self.steps = next;
        Ok(prepared.event)
    }
    fn prepare_checked(
        &self,
        binding: OperationalBinding,
    ) -> std::result::Result<PreparedStep, AdmissionFailure> {
        let mut candidate = self.graph.clone();
        let event = apply_candidate(
            &mut candidate,
            binding.source,
            &binding.assignments,
            &self.limits.graph,
        )?;
        // The reference transaction has completed checked grounding and admission.
        // Materialise its ground carrier, rather than reinterpreting variables here.
        let image: BTreeSet<_> = event.consumed.union(&event.preserved).copied().collect();
        let left = image
            .iter()
            .map(|id| self.graph.atom(*id).map(|a| (*id, a.clone())))
            .collect::<Result<_>>()?;
        let fresh = event
            .produced
            .iter()
            .map(|id| candidate.atom(*id).map(|a| (*id, a.clone())))
            .collect::<Result<_>>()?;
        let rule = GroundRule {
            left,
            preserved: event.preserved.clone(),
            fresh,
        };
        Ok(PreparedStep {
            baseline: Rc::clone(&self.baseline),
            step: self.steps,
            binding,
            rule,
            ground_match: GroundMatch { image },
            candidate,
            event,
        })
    }
}

/// A bounded search over a borrowed, immutable execution state. Inapplicable bindings
/// may be skipped, but resource exhaustion is sticky and must not become an empty
/// enabled set. Call `finish` before interpreting enumeration as complete.
/// A successful finish only certifies that limits were not exceeded; complete
/// enumeration is guaranteed only by successful bindings/enabled queries.
#[derive(Debug)]
pub struct CandidateSearch<'a> {
    pub(crate) execution: &'a Execution,
    candidates: usize,
    work: usize,
    exhausted: Option<Diagnostic>,
}
impl CandidateSearch<'_> {
    pub(crate) fn check(&self) -> Result<()> {
        match &self.exhausted {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    fn exhaust(&mut self, kind: LimitKind) -> Diagnostic {
        let error = Diagnostic::exhausted(kind, format!("{kind:?} limit exceeded"));
        self.exhausted = Some(error.clone());
        error
    }
    /// Reserve work before exploring a partial binding or other search operation.
    pub fn charge_work(&mut self, amount: usize) -> Result<()> {
        self.check()?;
        match self.work.checked_add(amount) {
            Some(next) if next <= self.execution.limits.search.max_work => {
                self.work = next;
                Ok(())
            }
            _ => Err(self.exhaust(LimitKind::SearchWork)),
        }
    }
    pub fn prepare(&mut self, binding: OperationalBinding) -> Result<PreparedStep> {
        self.prepare_classified(binding)
            .map_err(AdmissionFailure::diagnostic)
    }
    pub(crate) fn reserve_candidate(&mut self, size: usize) -> Result<()> {
        self.check()?;
        if self.candidates >= self.execution.limits.search.max_candidates {
            return Err(self.exhaust(LimitKind::Candidates));
        }
        self.candidates += 1;
        self.charge_work(1)?;
        self.charge_work(size)
    }
    pub(crate) fn prepare_classified(
        &mut self,
        binding: OperationalBinding,
    ) -> std::result::Result<PreparedStep, AdmissionFailure> {
        self.reserve_candidate(binding.assignments.len())?;
        let result = self.execution.prepare_checked(binding);
        if let Err(AdmissionFailure::Failed(error) | AdmissionFailure::Rejected(error)) = &result {
            if error.kind() == ErrorKind::ResourceExhaustion {
                self.exhausted = Some(error.clone());
            }
        }
        result
    }
    pub(crate) fn remember_failure<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            // A failed complete query cannot later be mistaken for a completed search.
            self.exhausted = Some(error.clone());
        }
        result
    }
    pub fn attempted_candidates(&self) -> usize {
        self.candidates
    }
    pub fn work_used(&self) -> usize {
        self.work
    }
    pub fn finish(self) -> Result<()> {
        self.check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn execution_counter_exhaustion_is_atomic() {
        let f =
            graph_core::fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
        let source = f.names["iw"];
        let mut execution = Execution::new(f.graph, ExecutionLimits::default()).unwrap();
        execution.steps = u64::MAX;
        let prepared = execution
            .prepare(OperationalBinding::new(source, BTreeMap::new()))
            .unwrap();
        let before = execution.graph.snapshot();
        let error = execution.commit(prepared).unwrap_err();
        assert_eq!(error.context().limit, Some(LimitKind::ExecutionSteps));
        assert_eq!(execution.graph.snapshot(), before);
        assert_eq!(execution.steps, u64::MAX);
    }
}
