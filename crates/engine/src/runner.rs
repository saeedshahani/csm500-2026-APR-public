//! Shared transaction-boundary control. Graph recurrence is diagnostic evidence,
//! never certification of convergence, oscillation, divergence or fairness.
use crate::observation::{isomorphic_budget, Budget, Observation, ObservationLimits, Observer};
use crate::scheduling::{candidate_key, SchedulingStrategy};
use crate::{Execution, ExecutionLimits};
use graph_core::{AtomKey, Diagnostic, ErrorKind, Graph, LimitKind, Result};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NormalForm,
    RequestedRuleUnavailable,
    StepLimit,
    ResourceLimit,
    Cancelled,
    CycleDetected,
}
#[derive(Clone, Debug)]
pub struct Recurrence {
    pub first: u64,
    pub repeated: u64,
    pub observer: String,
    pub first_scheduler: String,
    pub repeated_scheduler: String,
    /// Whether the complete carrier also recurs, confirmed independently.
    pub complete_state_equal: bool,
}
#[derive(Debug)]
pub struct RunReport {
    pub outcome: Outcome,
    pub rejected: usize,
    pub diagnostic: Option<Diagnostic>,
    pub recurrence: Option<Recurrence>,
    pub observer: String,
    pub payload: Option<usize>,
    pub nontriviality_threshold: usize,
    pub scheduler: String,
}
#[derive(Clone, Copy, Debug)]
pub struct RunLimits {
    pub max_steps: u64,
    pub observation: ObservationLimits,
    pub detect_cycles: bool,
}
impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_steps: 100,
            observation: ObservationLimits::default(),
            detect_cycles: false,
        }
    }
}
struct HistoryEntry {
    step: u64,
    full: Observation,
    selected: Observation,
    scheduler: String,
}
/// Owns the history boundary. No mutable Execution escape is provided. A valid
/// replacement requires a freshly resolved observer and clears all run state.
pub struct Runner<S: SchedulingStrategy> {
    execution: Execution,
    initial_scheduler: S,
    scheduler: S,
    observer: Observer,
    history: Vec<HistoryEntry>,
    history_units: usize,
    initial_identity: String,
    recording: Option<crate::recording::Recording>,
}
impl<S: SchedulingStrategy> Runner<S> {
    pub fn new(
        graph: Graph,
        limits: ExecutionLimits,
        scheduler: S,
        observer: Observer,
    ) -> Result<Self> {
        let execution = Execution::new(graph, limits)?;
        let initial_identity = initial_identity(execution.graph());
        Ok(Self {
            execution,
            initial_scheduler: scheduler.clone(),
            scheduler,
            observer,
            history: Vec::new(),
            history_units: 0,
            initial_identity,
            recording: None,
        })
    }
    pub fn execution(&self) -> &Execution {
        &self.execution
    }
    /// Start retaining events at state zero. Serialisation and replay are separate.
    pub fn enable_recording(&mut self, limits: crate::recording::RecordingLimits) -> Result<()> {
        if self.recording.is_some() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidConfiguration,
                "recording is already enabled",
            ));
        }
        self.recording = Some(crate::recording::Recording::new(&self.execution, limits)?);
        Ok(())
    }
    pub fn recording(&self) -> Option<&crate::recording::Recording> {
        self.recording.as_ref()
    }
    pub fn scheduler(&self) -> &S {
        &self.scheduler
    }
    pub fn retained_states(&self) -> usize {
        self.history.len()
    }
    pub fn replace_graph(&mut self, graph: Graph, observer: Observer) -> Result<()> {
        // Stage the complete replacement, including recording capacity, before
        // publishing any new baseline or resetting scheduler/observation state.
        let replacement = Execution::new(graph, self.execution.limits())?;
        let recording = self
            .recording
            .as_ref()
            .map(|r| crate::recording::Recording::new(&replacement, r.limits()))
            .transpose()?;
        self.execution = replacement;
        self.recording = recording;
        self.initial_identity = initial_identity(self.execution.graph());
        self.observer = observer;
        self.scheduler = self.initial_scheduler.clone();
        self.history = Vec::new();
        self.history_units = 0;
        Ok(())
    }
    /// Each call starts a fresh recurrence search, but resumes committed step and
    /// scheduler state. max_steps is an absolute bound since graph replacement.
    /// Cancellation is checked before queries and before commit. Query exhaustion
    /// is inconclusive. Complete empty query outranks cycle/step limits; recurrence
    /// outranks the step bound. RNG is published only with a successful commit.
    pub fn run(
        &mut self,
        limits: RunLimits,
        requested: Option<AtomKey>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<RunReport> {
        self.history = Vec::new();
        self.history_units = 0;
        let mut report = RunReport {
            outcome: Outcome::Cancelled,
            rejected: 0,
            diagnostic: None,
            recurrence: None,
            observer: format!(
                "initial-carrier-sha256-debug-v1={}; {}",
                self.initial_identity,
                self.observer.identity()
            ),
            payload: None,
            nontriviality_threshold: 2,
            scheduler: self.scheduler.description(),
        };
        loop {
            // Never attach a preceding state's measure to the final carrier.
            report.payload = None;
            if cancelled() {
                report.outcome = Outcome::Cancelled;
                break;
            }
            let query = (|| {
                if let Some(head) = requested {
                    if self.execution.view()?.rule(head).is_none() {
                        return Ok(None);
                    }
                }
                match requested {
                    Some(head) => self.execution.enabled_for(head).map(Some),
                    None => self.execution.enabled().map(Some),
                }
            })();
            let enabled = match query {
                Ok(Some(set)) => set,
                Ok(None) => {
                    report.outcome = Outcome::RequestedRuleUnavailable;
                    break;
                }
                Err(error) if error.kind() == ErrorKind::ResourceExhaustion => {
                    report.outcome = Outcome::ResourceLimit;
                    report.diagnostic = Some(error);
                    break;
                }
                Err(error) => return Err(error),
            };
            report.rejected = enabled.rejected().len();
            report.diagnostic = enabled.rejected().first().map(|r| r.diagnostic.clone());
            if enabled.steps().is_empty() {
                report.outcome = if requested.is_some() {
                    Outcome::RequestedRuleUnavailable
                } else {
                    Outcome::NormalForm
                };
                break;
            }
            if limits.detect_cycles {
                match self.observe(limits.observation) {
                    Ok((payload, recurrence)) => {
                        report.payload = Some(payload);
                        if let Some(recurrence) = recurrence {
                            report.outcome = Outcome::CycleDetected;
                            report.recurrence = Some(recurrence);
                            break;
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::ResourceExhaustion => {
                        report.outcome = Outcome::ResourceLimit;
                        report.diagnostic = Some(error);
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
            if self.execution.committed_steps() >= limits.max_steps {
                report.outcome = Outcome::StepLimit;
                break;
            }
            if cancelled() {
                report.outcome = Outcome::Cancelled;
                break;
            }
            let committed = match self.select_commit(enabled.into_steps(), &mut cancelled) {
                Err(error) if error.kind() == ErrorKind::ResourceExhaustion => {
                    report.outcome = Outcome::ResourceLimit;
                    report.diagnostic = Some(error);
                    break;
                }
                other => other?,
            };
            if !committed {
                report.outcome = Outcome::Cancelled;
                break;
            }
        }
        report.scheduler = self.scheduler.description();
        if let Some(recurrence) = &mut report.recurrence {
            recurrence.observer = report.observer.clone();
        }
        Ok(report)
    }
    fn select_commit(
        &mut self,
        mut steps: Vec<crate::PreparedStep>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<bool> {
        steps.sort_by_cached_key(candidate_key);
        let mut next_scheduler = self.scheduler.clone();
        let index = next_scheduler
            .select(&steps)
            .filter(|i| *i < steps.len())
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::InvalidBinding,
                    "scheduler did not select an enabled candidate",
                )
            })?;
        if cancelled() {
            return Ok(false);
        }
        let prepared = steps.swap_remove(index);
        if let Some(recording) = &mut self.recording {
            recording.commit(&mut self.execution, prepared)?;
        } else {
            self.execution.commit(prepared)?;
        }
        self.scheduler = next_scheduler;
        Ok(true)
    }
    fn observe(&mut self, limits: ObservationLimits) -> Result<(usize, Option<Recurrence>)> {
        let view = self.execution.view()?;
        let selected = self.observer.project(&view, limits)?;
        let full = Observer::all().project(&view, limits)?;
        let payload = selected.value_payload();
        let mut budget = Budget::new(limits.max_comparison_work, LimitKind::ComparisonWork);
        for previous in &self.history {
            if isomorphic_budget(&previous.selected, &selected, &mut budget)? {
                let complete_state_equal = isomorphic_budget(&previous.full, &full, &mut budget)?;
                return Ok((
                    payload,
                    Some(Recurrence {
                        first: previous.step,
                        repeated: self.execution.committed_steps(),
                        observer: self.observer.identity(),
                        first_scheduler: previous.scheduler.clone(),
                        repeated_scheduler: self.scheduler.description(),
                        complete_state_equal,
                    }),
                ));
            }
        }
        let units = selected
            .retention_units()
            .saturating_add(full.retention_units())
            .saturating_add(self.scheduler.description().len());
        let total = self
            .history_units
            .checked_add(units)
            .filter(|n| *n <= limits.max_history_units)
            .ok_or_else(|| {
                Diagnostic::exhausted(
                    LimitKind::ObservationHistory,
                    "retained observation history limit exhausted",
                )
            })?;
        self.history.push(HistoryEntry {
            step: self.execution.committed_steps(),
            full,
            selected,
            scheduler: self.scheduler.description(),
        });
        self.history_units = total;
        Ok((payload, None))
    }
}

// Versioned identity-bearing diagnostic encoding, not an isomorphism hash.
// Never used to decide graph equality; exact comparisons retain actual atoms.
fn initial_identity(graph: &Graph) -> String {
    let mut digest = Sha256::new();
    for (key, atom) in graph.iter() {
        digest.update(format!("{key:?}:{atom:?}\n").as_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduling::Scheduler;
    #[test]
    fn stale_selected_proposal_does_not_advance_rng_or_publish_graph() {
        let g=graph_core::fixture::load("h RuleHead\nz Nat\nr Record[Rank] s0=h s1=z\nv Record[Revision] s0=h\nm Record[ModeObject] s0=h\na Record[Active] s0=h").unwrap().graph;
        let mut r = Runner::new(
            g.clone(),
            ExecutionLimits::default(),
            Scheduler::seeded(3),
            Observer::all(),
        )
        .unwrap();
        let proposals = r.execution.enabled().unwrap().into_steps();
        r.replace_graph(g.clone(), Observer::all()).unwrap();
        let before = r.scheduler.description();
        assert_eq!(
            r.select_commit(proposals, &mut || false)
                .unwrap_err()
                .kind(),
            ErrorKind::StaleCandidate
        );
        assert_eq!(r.scheduler.description(), before);
        assert_eq!(r.execution.graph().snapshot(), g.snapshot());
        assert_eq!(r.execution.committed_steps(), 0);
    }
}
