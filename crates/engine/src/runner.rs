//! Shared transaction-boundary control. Graph recurrence is diagnostic evidence,
//! never certification of convergence, oscillation, divergence or fairness.
use crate::observation::{
    isomorphic_budget, isomorphic_indexed_budget, Budget, Observation, ObservationLimits, Observer,
};
use crate::scheduling::{candidate_key, SchedulingStrategy};
use crate::{Execution, ExecutionLimits};
use graph_core::{AtomKey, Diagnostic, ErrorKind, Graph, LimitKind, Result};
use sha2::{Digest, Sha256};

/// A versioned algorithm and logical work contract, independent of the source
/// fingerprint identifying a particular build. Existing defaults stay reference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionProfile {
    #[default]
    #[serde(rename = "reference-v1")]
    ReferenceV1,
    #[serde(rename = "indexed-v2")]
    IndexedV2,
    #[serde(rename = "framed-v3")]
    FramedV3,
    #[serde(rename = "joined-v4")]
    JoinedV4,
}

impl ExecutionProfile {
    pub fn uses_frames(self) -> bool {
        matches!(self, Self::FramedV3 | Self::JoinedV4)
    }
}

enum Candidates {
    Full(Vec<crate::PreparedStep>),
    Compact(Vec<crate::OperationalBinding>),
    Certified(usize),
}
impl Candidates {
    fn is_empty(&self) -> bool {
        match self {
            Self::Full(v) => v.is_empty(),
            Self::Compact(v) => v.is_empty(),
            Self::Certified(n) => *n == 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NormalForm,
    RequestedRuleUnavailable,
    StepLimit,
    ResourceLimit,
    Cancelled,
    CycleDetected,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recurrence {
    pub first: u64,
    pub repeated: u64,
    pub observer: String,
    pub first_scheduler: String,
    pub repeated_scheduler: String,
    /// Whether the complete carrier also recurs, confirmed independently.
    pub complete_state_equal: bool,
}
#[derive(Clone, Debug)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug)]
struct HistoryEntry {
    step: u64,
    full: Observation,
    selected: Observation,
    scheduler: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunPlan {
    pub limits: RunLimits,
    pub requested: Option<AtomKey>,
    pub start_step: u64,
}
#[derive(Debug)]
// A transient return value, not a retained collection; avoid a terminal heap allocation.
#[allow(clippy::large_enum_variant)]
pub enum RunProgress {
    Yielded,
    Finished(RunReport),
}
/// Owns the history boundary. No mutable Execution escape is provided. A valid
/// replacement requires a freshly resolved observer and clears all run state.
pub struct Runner<S: SchedulingStrategy> {
    one_shot: Option<crate::one_shot::OneShot>,
    screened: crate::screened::History,
    produced_units: usize,
    transition_digest: [u8; 32],
    profile: ExecutionProfile,
    execution: Execution,
    initial_scheduler: S,
    scheduler: S,
    observer: Observer,
    history: Vec<HistoryEntry>,
    history_units: usize,
    initial_identity: String,
    recording: Option<crate::recording::Recording>,
    plan: Option<RunPlan>,
    active: bool,
    last_report: Option<RunReport>,
}
impl<S: SchedulingStrategy> Runner<S> {
    pub fn new(
        graph: Graph,
        limits: ExecutionLimits,
        scheduler: S,
        observer: Observer,
    ) -> Result<Self> {
        Self::with_profile(
            graph,
            limits,
            scheduler,
            observer,
            ExecutionProfile::ReferenceV1,
        )
    }
    pub fn with_profile(
        graph: Graph,
        limits: ExecutionLimits,
        scheduler: S,
        observer: Observer,
        profile: ExecutionProfile,
    ) -> Result<Self> {
        let execution = if profile.uses_frames() {
            Execution::with_frame_validation(graph, limits)?
        } else {
            Execution::new(graph, limits)?
        };
        let initial_identity = initial_identity(execution.graph());
        Ok(Self {
            one_shot: None,
            screened: Default::default(),
            produced_units: 0,
            transition_digest: [0; 32],
            profile,
            execution,
            initial_scheduler: scheduler.clone(),
            scheduler,
            observer,
            history: Vec::new(),
            history_units: 0,
            initial_identity,
            recording: None,
            plan: None,
            active: false,
            last_report: None,
        })
    }
    pub fn execution(&self) -> &Execution {
        &self.execution
    }
    pub fn profile(&self) -> ExecutionProfile {
        self.profile
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
    pub fn run_plan(&self) -> Option<RunPlan> {
        self.plan
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn last_report(&self) -> Option<&RunReport> {
        self.last_report.as_ref()
    }
    pub fn observer(&self) -> &Observer {
        &self.observer
    }
    /// Independent logical-run copy with a fresh transaction baseline. Changes
    /// in either copy do not publish into the other; used for staged UI commands.
    pub fn fork(&self) -> Self {
        let execution = self.execution.fork();
        let recording = self.recording.as_ref().map(|r| r.fork(&execution));
        Self {
            one_shot: self.one_shot.clone(),
            screened: self.screened.clone(),
            produced_units: self.produced_units,
            transition_digest: self.transition_digest,
            profile: self.profile,
            execution,
            recording,
            initial_scheduler: self.initial_scheduler.clone(),
            scheduler: self.scheduler.clone(),
            observer: self.observer.clone(),
            history: self.history.clone(),
            history_units: self.history_units,
            initial_identity: self.initial_identity.clone(),
            plan: self.plan,
            active: self.active,
            last_report: self.last_report.clone(),
        }
    }
    pub(crate) fn continuation_identity(&self) -> String {
        let mut hash = Sha256::new();
        if self.profile.uses_frames() {
            hash.update(if self.profile == ExecutionProfile::JoinedV4 {
                b"joined-v4;"
            } else {
                b"framed-v3;"
            });
            hash.update(self.initial_identity.as_bytes());
            hash.update(self.transition_digest);
            hash.update(format!(
                "{:?};{};{};{};{:?};{};{:?}",
                self.plan,
                self.active,
                self.execution.committed_steps(),
                self.execution.graph().next_identity(),
                self.screened.identity(),
                self.observer.identity(),
                self.last_report
            ));
            hash.update(self.scheduler.description());
            return format!("{:x}", hash.finalize());
        }
        if self.profile == ExecutionProfile::IndexedV2 {
            hash.update(b"indexed-v2;");
        }
        hash.update(format!(
            "{:?};{};{};{};{:?};{};{:?}",
            self.plan,
            self.active,
            self.execution.committed_steps(),
            self.execution.graph().next_identity(),
            self.history,
            self.observer.identity(),
            self.last_report
        ));
        hash.update(self.scheduler.description());
        hash.update(initial_identity(self.execution.graph()));
        format!("{:x}", hash.finalize())
    }
    pub(crate) fn checkpoint_units(&self) -> usize {
        self.retained_units()
            .saturating_add(self.recording.as_ref().map_or(0, |r| r.units()))
            .saturating_add(
                crate::recording::graph_units(self.execution.graph()).unwrap_or(usize::MAX),
            )
    }
    pub fn scheduler(&self) -> &S {
        &self.scheduler
    }
    pub fn retained_states(&self) -> usize {
        if self.profile.uses_frames() {
            self.screened.len()
        } else {
            self.history.len()
        }
    }
    fn retained_units(&self) -> usize {
        if self.profile.uses_frames() {
            self.screened.units()
        } else {
            self.history_units
        }
    }
    pub fn replace_graph(&mut self, graph: Graph, observer: Observer) -> Result<()> {
        // Stage the complete replacement, including recording capacity, before
        // publishing any new baseline or resetting scheduler/observation state.
        let replacement = if self.profile.uses_frames() {
            Execution::with_frame_validation(graph, self.execution.limits())?
        } else {
            Execution::new(graph, self.execution.limits())?
        };
        let recording = self
            .recording
            .as_ref()
            .map(|r| crate::recording::Recording::new(&replacement, r.limits()))
            .transpose()?;
        self.execution = replacement;
        self.one_shot = None;
        self.screened = Default::default();
        self.produced_units = 0;
        self.transition_digest = [0; 32];
        self.recording = recording;
        self.initial_identity = initial_identity(self.execution.graph());
        self.observer = observer;
        self.scheduler = self.initial_scheduler.clone();
        self.history = Vec::new();
        self.history_units = 0;
        self.plan = None;
        self.active = false;
        self.last_report = None;
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
        cancelled: impl FnMut() -> bool,
    ) -> Result<RunReport> {
        self.begin_run(limits, requested)?;
        let mut cancelled = cancelled;
        loop {
            if let RunProgress::Finished(report) = self.advance(u64::MAX, &mut cancelled)? {
                return Ok(report);
            }
        }
    }
    /// Begin a new logical run. A yielded run must be resumed or explicitly
    /// reset; beginning another run must not silently erase its recurrence state.
    pub fn begin_run(&mut self, limits: RunLimits, requested: Option<AtomKey>) -> Result<()> {
        if self.active {
            return Err(Diagnostic::new(
                ErrorKind::InvalidConfiguration,
                "a logical run is already active",
            ));
        }
        self.plan = Some(RunPlan {
            limits,
            requested,
            start_step: self.execution.committed_steps(),
        });
        self.active = true;
        self.last_report = None;
        self.history = Vec::new();
        self.history_units = 0;
        self.screened = Default::default();
        Ok(())
    }
    /// Execute at most batch_steps commits, yielding before the next state query.
    /// Yield is not an outcome. Cancellation finishes the logical run; pausing by
    /// not calling advance preserves it. Query-level interruption remains absent.
    pub fn advance(
        &mut self,
        batch_steps: u64,
        cancelled: impl FnMut() -> bool,
    ) -> Result<RunProgress> {
        self.advance_guarded(batch_steps, cancelled, |_| true)
    }
    /// Transaction-boundary debugger gate, evaluated only for the scheduler's
    /// selected candidate after terminal checks. False yields without committing,
    /// advancing the scheduler, or retaining an endpoint observation. Selection
    /// is recomputed on continuation from the unchanged scheduler and carrier;
    /// strategies must select identically from identical cloned state and input.
    /// The gate is external control, not part of graph or trace semantics.
    pub fn advance_guarded(
        &mut self,
        batch_steps: u64,
        mut cancelled: impl FnMut() -> bool,
        mut allow: impl FnMut(&crate::PreparedStep) -> bool,
    ) -> Result<RunProgress> {
        if !self.active || batch_steps == 0 {
            return Err(Diagnostic::new(
                ErrorKind::InvalidConfiguration,
                "advance requires an active run and positive batch size",
            ));
        }
        let RunPlan {
            limits, requested, ..
        } = self.plan.expect("active run has a plan");
        let start = self.execution.committed_steps();
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
            if self.execution.committed_steps() - start >= batch_steps {
                return Ok(RunProgress::Yielded);
            }
            // Never attach a preceding state's measure to the final carrier.
            report.payload = None;
            if cancelled() {
                report.outcome = Outcome::Cancelled;
                break;
            }
            let query: Result<Option<(Candidates, Vec<crate::RejectedBinding>)>> = (|| {
                if let Some(head) = requested {
                    if self.execution.view()?.rule(head).is_none() {
                        return Ok(None);
                    }
                }
                if self.profile.uses_frames() && self.scheduler.supports_count_selection() {
                    if self.one_shot.is_none() {
                        self.one_shot = crate::one_shot::OneShot::recognise(
                            &self.execution,
                            self.profile == ExecutionProfile::JoinedV4,
                        )?;
                    }
                    if let Some(pool) = &self.one_shot {
                        if requested.is_none_or(|head| head == pool.head) {
                            pool.check_query(&self.execution)?;
                            return Ok(Some((Candidates::Certified(pool.len()), Vec::new())));
                        }
                    }
                }
                if self.profile != ExecutionProfile::ReferenceV1 {
                    let (bindings, rejected) =
                        self.execution.compact_query(requested, true)?.into_parts();
                    Ok(Some((Candidates::Compact(bindings), rejected)))
                } else {
                    let set = match requested {
                        Some(head) => self.execution.enabled_for(head)?,
                        None => self.execution.enabled()?,
                    };
                    let rejected = set.rejected().to_vec();
                    Ok(Some((Candidates::Full(set.into_steps()), rejected)))
                }
            })();
            let (enabled, rejected) = match query {
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
            report.rejected = rejected.len();
            report.diagnostic = rejected.first().map(|r| r.diagnostic.clone());
            if enabled.is_empty() {
                report.outcome = if requested.is_some() {
                    Outcome::RequestedRuleUnavailable
                } else {
                    Outcome::NormalForm
                };
                break;
            }
            let history_boundary = (self.retained_states(), self.retained_units());
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
            let committed = match self.select_commit_guarded(enabled, &mut cancelled, &mut allow) {
                Err(error) if error.kind() == ErrorKind::ResourceExhaustion => {
                    report.outcome = Outcome::ResourceLimit;
                    report.diagnostic = Some(error);
                    break;
                }
                other => other?,
            };
            let Some(committed) = committed else {
                if self.profile.uses_frames() {
                    self.screened
                        .truncate(history_boundary.0, history_boundary.1);
                } else {
                    self.history.truncate(history_boundary.0);
                    self.history_units = history_boundary.1;
                }
                return Ok(RunProgress::Yielded);
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
        self.active = false;
        self.last_report = Some(report.clone());
        Ok(RunProgress::Finished(report))
    }
    #[cfg(test)]
    fn select_commit(
        &mut self,
        steps: Vec<crate::PreparedStep>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<bool> {
        self.select_commit_guarded(Candidates::Full(steps), cancelled, &mut |_| true)
            .map(|v| v.unwrap())
    }
    fn select_commit_guarded(
        &mut self,
        candidates: Candidates,
        cancelled: &mut impl FnMut() -> bool,
        allow: &mut impl FnMut(&crate::PreparedStep) -> bool,
    ) -> Result<Option<bool>> {
        let mut next_scheduler = self.scheduler.clone();
        let mut certified_index = None;
        let bad_selection = || {
            Diagnostic::new(
                ErrorKind::InvalidBinding,
                "scheduler did not select an enabled candidate",
            )
        };
        let prepared = match candidates {
            Candidates::Certified(count) => {
                let rank = next_scheduler
                    .select_rank(count)
                    .filter(|i| *i < count)
                    .ok_or_else(bad_selection)?;
                let (index, binding) = self.one_shot.as_ref().unwrap().select(rank);
                certified_index = Some(index);
                self.execution.prepare(binding)?
            }
            Candidates::Compact(bindings) => {
                if let Some(selected) = next_scheduler.select_bindings(&bindings) {
                    let index = selected
                        .filter(|i| *i < bindings.len())
                        .ok_or_else(bad_selection)?;
                    self.execution.prepare(bindings[index].clone())?
                } else {
                    // A non-opted-in strategy retains its original full-candidate
                    // interface. Discard any speculative clone mutations first.
                    next_scheduler = self.scheduler.clone();
                    let mut steps = bindings
                        .into_iter()
                        .map(|b| self.execution.prepare(b))
                        .collect::<Result<Vec<_>>>()?;
                    steps.sort_by_cached_key(candidate_key);
                    let index = next_scheduler
                        .select(&steps)
                        .filter(|i| *i < steps.len())
                        .ok_or_else(bad_selection)?;
                    steps.swap_remove(index)
                }
            }
            Candidates::Full(mut steps) => {
                steps.sort_by_cached_key(candidate_key);
                let index = next_scheduler
                    .select(&steps)
                    .filter(|i| *i < steps.len())
                    .ok_or_else(bad_selection)?;
                steps.swap_remove(index)
            }
        };
        if cancelled() {
            return Ok(Some(false));
        }
        if !allow(&prepared) {
            return Ok(None);
        }
        let mut next_digest = self.transition_digest;
        let mut produced_units = 0usize;
        if self.profile.uses_frames() {
            let mut hash = Sha256::new();
            hash.update(self.transition_digest);
            hash.update(format!(
                "{:?};{:?}",
                prepared.event(),
                prepared.ground_rule().fresh()
            ));
            next_digest = hash.finalize().into();
            for atom in prepared.ground_rule().fresh().values() {
                produced_units = produced_units.saturating_add(crate::recording::atom_units(atom)?);
            }
        }
        if let Some(recording) = &mut self.recording {
            recording.commit(&mut self.execution, prepared)?;
        } else {
            self.execution.commit(prepared)?;
        }
        self.scheduler = next_scheduler;
        self.transition_digest = next_digest;
        self.produced_units = produced_units;
        if let Some(index) = certified_index {
            self.one_shot.as_mut().unwrap().remove(index);
        } else {
            self.one_shot = None;
        }
        Ok(Some(true))
    }
    fn observe(&mut self, limits: ObservationLimits) -> Result<(usize, Option<Recurrence>)> {
        if self.profile.uses_frames() {
            return self.screened.observe(
                &self.execution,
                &self.observer,
                self.scheduler.description(),
                limits,
                self.produced_units,
                self.profile == ExecutionProfile::JoinedV4,
            );
        }
        let view = self.execution.view()?;
        let selected = self.observer.project(&view, limits)?;
        let full = Observer::all().project(&view, limits)?;
        let payload = selected.value_payload();
        let mut budget = Budget::new(limits.max_comparison_work, LimitKind::ComparisonWork);
        let compare = match self.profile {
            ExecutionProfile::ReferenceV1 => isomorphic_budget,
            ExecutionProfile::IndexedV2 => isomorphic_indexed_budget,
            ExecutionProfile::FramedV3 | ExecutionProfile::JoinedV4 => unreachable!(),
        };
        for previous in &self.history {
            if compare(&previous.selected, &selected, &mut budget)? {
                let complete_state_equal = compare(&previous.full, &full, &mut budget)?;
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
