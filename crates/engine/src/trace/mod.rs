//! Portable, bounded, checked histories. Imported snapshots are never trusted.
mod canonical;
pub(crate) mod decimal;
mod storage;
mod strict;
mod wire;
use crate::causal::{AnalysisLimits, DependencyOrder, LIMITATION, RELATION};
use crate::observation::{ObservationLimits, Observer};
use crate::recording::{RecordedEvent, Recording, RecordingLimits};
use crate::runner::{RunProgress, Runner};
use crate::scheduling::{Scheduler, SchedulingStrategy};
use crate::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{Diagnostic, ErrorKind, Graph, LimitKind};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
pub use storage::SnapshotCost;
pub use wire::*;
type Result<T> = graph_core::Result<T>;
pub const SCHEMA: &str = "reflective-trace-v1";
pub const PROFILE: &str = "checked-binding-positive-resources-v1";
pub const IMPLEMENTATION: &str = concat!(
    "engine-",
    env!("CARGO_PKG_VERSION"),
    "-source-sha256=",
    env!("ENGINE_SOURCE_SHA256")
);
fn bad(message: &str) -> Diagnostic {
    Diagnostic::new(ErrorKind::MalformedInput, message)
}
fn exhausted(message: &str) -> Diagnostic {
    Diagnostic::exhausted(LimitKind::TraceStorage, message)
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hash_json<T: Serialize>(value: &T) -> Result<String> {
    struct Sink(Sha256);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.update(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink(Sha256::new());
    serde_json::to_writer(&mut sink, value).map_err(|e| bad(&e.to_string()))?;
    Ok(format!("{:x}", sink.0.finalize()))
}
#[derive(Clone, Copy, Debug)]
pub struct TraceLimits {
    pub max_bytes: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_events: usize,
    pub max_checkpoints: usize,
    pub max_checkpoint_units: usize,
    pub max_replay_work: usize,
    pub max_atoms_total: usize,
    pub max_fields_total: usize,
    pub max_bindings: usize,
    pub max_witnesses: usize,
    pub max_conflicts: usize,
    pub execution: ExecutionLimits,
    pub observation: ObservationLimits,
    pub recording: RecordingLimits,
    pub analysis: AnalysisLimits,
}
impl Default for TraceLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_nodes: 2_000_000,
            max_depth: 64,
            max_events: 10_000,
            max_checkpoints: 1000,
            max_checkpoint_units: 50_000_000,
            max_replay_work: 100_000_000,
            max_atoms_total: 200_000,
            max_fields_total: 400_000,
            max_bindings: 100_000,
            max_witnesses: 100_000,
            max_conflicts: 10_000,
            execution: ExecutionLimits::default(),
            observation: ObservationLimits::default(),
            recording: RecordingLimits::default(),
            analysis: AnalysisLimits::default(),
        }
    }
}
#[cfg(test)]
fn structural_limits(value: &serde_json::Value, limits: TraceLimits) -> Result<()> {
    fn visit(v: &serde_json::Value, left: &mut [usize; 5]) -> Result<()> {
        match v {
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    let slot = match key.as_str() {
                        "atoms" | "fresh" => Some(0),
                        "fields" => Some(1),
                        "binding" => Some(2),
                        "witnesses" => Some(3),
                        "pairs" => Some(4),
                        _ => None,
                    };
                    if let Some(i) = slot {
                        let n = value
                            .as_array()
                            .map_or_else(|| value.as_object().map_or(0, |m| m.len()), Vec::len);
                        left[i] = left[i]
                            .checked_sub(n)
                            .ok_or_else(|| exhausted(&format!("aggregate {key} limit")))?;
                    }
                    visit(value, left)?;
                }
            }
            serde_json::Value::Array(values) => {
                for v in values {
                    visit(v, left)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(
        value,
        &mut [
            limits.max_atoms_total,
            limits.max_fields_total,
            limits.max_bindings,
            limits.max_witnesses,
            limits.max_conflicts,
        ],
    )
}
#[derive(Clone, Debug)]
pub struct TraceOptions {
    pub checkpoint_interval: u64,
    pub conflict_states: Vec<u64>,
}
impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            checkpoint_interval: 32,
            conflict_states: Vec::new(),
        }
    }
}
struct Checkpoint {
    state: usize,
    execution: Execution,
    runner: Option<Runner<Scheduler>>,
}
pub struct VerifiedTrace {
    document: TraceDocument,
    checkpoints: Vec<Checkpoint>,
    limits: TraceLimits,
}

impl TraceDocument {
    pub fn from_recording(
        recording: &Recording,
        observer: &Observer,
        source_sha256: Option<String>,
        options: TraceOptions,
        limits: TraceLimits,
    ) -> Result<Self> {
        Ok(VerifiedTrace::verify(
            Self::recording_document(recording, observer, source_sha256, options, limits)?,
            limits,
        )?
        .document)
    }
    fn recording_document(
        recording: &Recording,
        observer: &Observer,
        source_sha256: Option<String>,
        options: TraceOptions,
        limits: TraceLimits,
    ) -> Result<Self> {
        let initial = GraphData {
            next_identity: Number(recording.initial_allocator()),
            atoms: recording.initial().clone(),
        };
        let execution = recording.execution_limits();
        let doc = Self {
            schema: SCHEMA.into(),
            profile: PROFILE.into(),
            implementation: IMPLEMENTATION.into(),
            source_sha256,
            initial_sha256: hash_json(&initial)?,
            initial,
            relation: RELATION.into(),
            limitation: LIMITATION.into(),
            settings: Settings {
                execution_profile: None,
                graph: execution.graph.into(),
                search: execution.search.into(),
                recording: recording.limits().into(),
                analysis: limits.analysis.into(),
                observer: ObserverData::from_observer(observer),
                plan: None,
            },
            status: "recorded_prefix".into(),
            report: None,
            events: recording.events().to_vec(),
            dependencies: None,
            conflicts: options
                .conflict_states
                .into_iter()
                .map(|n| ConflictAt {
                    state: Number(n),
                    analysis: None,
                })
                .collect(),
            checkpoint_interval: Number(options.checkpoint_interval),
            checkpoints: Vec::new(),
        };
        Ok(doc)
    }
    pub fn from_runner(
        runner: &Runner<Scheduler>,
        source_sha256: Option<String>,
        options: TraceOptions,
        limits: TraceLimits,
    ) -> Result<Self> {
        let recording = runner
            .recording()
            .ok_or_else(|| bad("recording is not enabled"))?;
        let plan = runner.run_plan().ok_or_else(|| bad("no logical run"))?;
        if plan.start_step != 0 {
            return Err(bad(
                "export requires one logical run starting at state zero",
            ));
        }
        let mut doc =
            Self::recording_document(recording, runner.observer(), source_sha256, options, limits)?;
        doc.settings.plan = Some(PlanData {
            max_steps: Number(plan.limits.max_steps),
            detect_cycles: plan.limits.detect_cycles,
            observation: plan.limits.observation.into(),
            requested: plan.requested.map(Into::into),
            seed: runner.scheduler().seed().map(Number),
        });
        doc.settings.execution_profile = Some(runner.profile());
        doc.status = if runner.is_active() {
            "yielded"
        } else {
            "finished"
        }
        .into();
        doc.report = runner.last_report().map(ReportData::from);
        doc.checkpoints.clear();
        Ok(VerifiedTrace::verify(doc, limits)?.document)
    }
    /// Bounded deterministic UTF-8 output. Publication is the caller's concern.
    pub fn to_json(&self, max_bytes: usize) -> Result<Vec<u8>> {
        struct Output {
            bytes: Vec<u8>,
            max: usize,
        }
        impl Write for Output {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if b.len() > self.max.saturating_sub(self.bytes.len()) {
                    return Err(std::io::Error::other("trace byte limit"));
                }
                self.bytes.extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut out = Output {
            bytes: Vec::new(),
            max: max_bytes,
        };
        serde_json::to_writer_pretty(&mut out, self).map_err(|e| exhausted(&e.to_string()))?;
        out.write_all(b"\n")
            .map_err(|e| exhausted(&e.to_string()))?;
        Ok(out.bytes)
    }
}
impl VerifiedTrace {
    fn parse_document(bytes: &[u8], limits: TraceLimits) -> Result<TraceDocument> {
        if bytes.len() > limits.max_bytes {
            return Err(exhausted("trace byte limit"));
        }
        strict::validate(bytes, limits)?;
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let doc: TraceDocument =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|e| bad(&e.to_string()))?;
        if doc.events.len() > limits.max_events || doc.checkpoints.len() > limits.max_checkpoints {
            return Err(exhausted("events/checkpoints"));
        }
        let canonical = serde_json::to_vec(&doc).map_err(|e| bad(&e.to_string()))?;
        if !canonical::equal(bytes, &canonical).map_err(|e| bad(&e.to_string()))? {
            return Err(bad("noncanonical or duplicate set members"));
        }
        Ok(doc)
    }
    pub fn import(bytes: &[u8], limits: TraceLimits) -> Result<Self> {
        Self::verify(Self::parse_document(bytes, limits)?, limits)
    }
    pub fn replace_from_json(&mut self, bytes: &[u8]) -> Result<()> {
        let next = Self::import(bytes, self.limits)?;
        *self = next;
        Ok(())
    }
    /// Explicit migration of one legacy scheduler history, not a declaration
    /// that an arbitrary historical implementation is compatible. Every event,
    /// selection, terminal report and checkpoint must reproduce under reference-v1.
    /// Original input bytes and build provenance are never rewritten in place.
    pub fn migrate_reference(bytes: &[u8], limits: TraceLimits) -> Result<Self> {
        let mut doc = Self::parse_document(bytes, limits)?;
        if doc.settings.execution_profile.is_some() || doc.settings.plan.is_none() {
            return Err(bad(
                "migration requires a legacy scheduler trace without an explicit execution profile",
            ));
        }
        doc.settings.execution_profile = Some(crate::runner::ExecutionProfile::ReferenceV1);
        Self::verify(doc, limits)
    }
    pub fn document(&self) -> &TraceDocument {
        &self.document
    }
    pub fn verify(mut doc: TraceDocument, limits: TraceLimits) -> Result<Self> {
        // The same structural budget applies to callers constructing DTOs directly.
        strict::validate(&doc.to_json(limits.max_bytes)?, limits)?;
        if doc.schema != SCHEMA
            || doc.profile != PROFILE
            || doc.relation != RELATION
            || doc.limitation != LIMITATION
        {
            return Err(bad("unsupported schema, profile or causal relation"));
        }
        if doc.events.len() > limits.max_events || doc.checkpoints.len() > limits.max_checkpoints {
            return Err(exhausted("events/checkpoints"));
        }
        if doc.checkpoint_interval.0 == 0 {
            return Err(bad("checkpoint interval must be positive"));
        }
        doc.settings.graph.capped(limits.execution.graph)?;
        doc.settings.search.capped(limits.execution.search)?;
        doc.settings.recording.capped(limits.recording)?;
        doc.settings.analysis.capped(limits.analysis)?;
        for digest in std::iter::once(&doc.initial_sha256).chain(doc.source_sha256.iter()) {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(bad("invalid SHA-256"));
            }
        }
        if hash_json(&doc.initial)? != doc.initial_sha256 {
            return Err(bad("initial_sha256 mismatch"));
        }
        let graph = doc.initial.restore(&doc.settings.graph.native()?)?;
        let observer = doc.settings.observer.restore(&graph, limits.max_bytes)?;
        let mut runner = if let Some(plan) = &doc.settings.plan {
            if doc.settings.execution_profile.is_none() && doc.implementation != IMPLEMENTATION {
                return Err(bad(
                    "scheduler reproduction requires compatible implementation",
                ));
            }
            plan.observation.capped(limits.observation)?;
            if plan.max_steps.0 > limits.max_events as u64 {
                return Err(exhausted("logical step limit exceeds import policy"));
            }
            let requested = plan
                .requested
                .map(|id| {
                    graph
                        .key(id.0)
                        .ok_or_else(|| bad("requested rule is absent"))
                })
                .transpose()?;
            let mut r = Runner::with_profile(
                graph.clone(),
                doc.settings.execution()?,
                plan.seed
                    .map_or(Scheduler::Deterministic, |n| Scheduler::seeded(n.0)),
                observer,
                doc.settings.execution_profile.unwrap_or_default(),
            )?;
            r.enable_recording(doc.settings.recording.native()?)?;
            r.begin_run(plan.limits()?, requested)?;
            Some(r)
        } else {
            None
        };
        match (doc.status.as_str(), runner.is_some(), doc.report.is_some()) {
            ("recorded_prefix", false, false)
            | ("yielded", true, false)
            | ("finished", true, true) => {}
            _ => return Err(bad("status, plan and report are inconsistent")),
        }
        let framed = doc
            .settings
            .execution_profile
            .unwrap_or_default()
            .uses_frames();
        let mut execution = if framed {
            Execution::with_frame_validation(graph, doc.settings.execution()?)?
        } else {
            Execution::new(graph, doc.settings.execution()?)?
        };
        let mut checked_recording = Recording::new(&execution, doc.settings.recording.native()?)?;
        let mut checkpoints = Vec::new();
        let mut units = limits.max_checkpoint_units;
        let mut work = limits.max_replay_work;
        let supplied = std::mem::take(&mut doc.checkpoints);
        if supplied.windows(2).any(|w| w[0].state >= w[1].state)
            || supplied.iter().any(|c| c.state.0 > doc.events.len() as u64)
        {
            return Err(bad("checkpoints must have unique ordered in-range states"));
        }
        if doc.conflicts.windows(2).any(|w| w[0].state >= w[1].state)
            || doc
                .conflicts
                .iter()
                .any(|c| c.state.0 > doc.events.len() as u64)
        {
            return Err(bad("conflicts must have unique ordered in-range states"));
        }
        for state in 0..=doc.events.len() {
            let cost = crate::recording::graph_units(execution.graph())?.saturating_add(1);
            let claim = supplied.iter().find(|c| c.state.0 == state as u64);
            let retain =
                state == doc.events.len() || (state as u64) % doc.checkpoint_interval.0 == 0;
            let charge = if framed {
                let event = doc.events.get(state).map_or(1, |e| {
                    1 + e.redex.binding.len()
                        + e.footprint.read.len()
                        + e.footprint.consumed.len()
                        + e.footprint.produced.len()
                });
                event.saturating_add(if retain || claim.is_some() {
                    cost.saturating_add(runner.as_ref().map_or(0, Runner::checkpoint_units))
                } else {
                    0
                })
            } else {
                cost.saturating_add(checked_recording.units())
                    .saturating_add(runner.as_ref().map_or(0, Runner::checkpoint_units))
            };
            work = work
                .checked_sub(charge)
                .ok_or_else(|| exhausted("aggregate replay work"))?;
            let current = (retain || claim.is_some()).then(|| CheckpointData {
                state: Number(state as u64),
                graph: GraphData::from_graph(execution.graph()),
                scheduler: runner.as_ref().map(|r| r.scheduler().description()),
                continuation_sha256: runner.as_ref().map(Runner::continuation_identity),
            });
            if let Some(claim) = claim {
                if Some(claim) != current.as_ref() {
                    return Err(bad(&format!(
                        "checkpoints[{state}] does not match checked prefix"
                    )));
                }
            }
            if retain {
                if checkpoints.len() >= limits.max_checkpoints {
                    return Err(exhausted("checkpoint count"));
                }
                units = units
                    .checked_sub(
                        cost.saturating_mul(2)
                            .saturating_add(runner.as_ref().map_or(0, Runner::checkpoint_units)),
                    )
                    .ok_or_else(|| exhausted("checkpoint retention"))?;
                doc.checkpoints.push(current.unwrap());
                checkpoints.push(Checkpoint {
                    state,
                    execution: execution.fork(),
                    // Only a yielded endpoint is a continuation authority.
                    // Historical/finished checkpoints need an execution graph,
                    // not another complete recorder and recurrence history.
                    // Keep the existing conservative budget charges above.
                    runner: if doc.status == "yielded" && state == doc.events.len() {
                        runner.as_ref().map(Runner::fork)
                    } else {
                        None
                    },
                });
            }
            if let Some(claim) = doc.conflicts.iter_mut().find(|c| c.state.0 == state as u64) {
                let actual = ConflictData::from_snapshot(crate::conflict::LocalSnapshot::analyse(
                    &execution,
                    doc.settings.analysis.native()?,
                )?)?;
                if claim.analysis.as_ref().is_some_and(|c| c != &actual) {
                    return Err(bad("conflict snapshot mismatch"));
                }
                claim.analysis = Some(actual);
            }
            if state == doc.events.len() {
                break;
            }
            checked_recorded_step(
                &mut execution,
                &doc.events[state],
                Some(&mut checked_recording),
            )
            .map_err(|e| Diagnostic::new(e.kind(), format!("events[{state}]: {e}")))?;
            if let Some(r) = runner.as_mut() {
                if !matches!(r.advance(1, || false)?, RunProgress::Yielded)
                    || r.recording().and_then(|h| h.events().last()) != Some(&doc.events[state])
                {
                    return Err(bad(&format!(
                        "events[{state}]: scheduler reproduction mismatch"
                    )));
                }
            }
        }
        if doc.status == "finished" {
            let report = doc.report.as_ref().expect("validated report");
            let r = runner.as_mut().expect("validated plan");
            match r.advance(1, || report.outcome == "Cancelled")? {
                RunProgress::Finished(actual) if ReportData::from(&actual) == *report => {}
                _ => return Err(bad("terminal report is not reproduced")),
            }
        }
        // Their claims have all been checked. Release transient replay owners
        // before deriving and validating the final portable document.
        drop(checked_recording);
        drop(runner);
        drop(execution);
        let edges = DependencyOrder::extract(&doc.events, doc.settings.analysis.native()?)?
            .edges()
            .iter()
            .map(|(&(before, after), w)| EdgeData {
                before,
                after,
                witnesses: w.clone(),
            })
            .collect::<Vec<_>>();
        if doc.dependencies.as_ref().is_some_and(|e| e != &edges) {
            return Err(bad("dependencies disagree with recomputed witnesses"));
        }
        doc.dependencies = Some(edges);
        // Derived data can be larger than the input that requested it.
        strict::validate(&doc.to_json(limits.max_bytes)?, limits)?;
        Ok(Self {
            document: doc,
            checkpoints,
            limits,
        })
    }
    pub fn state_at(&self, index: usize) -> Result<Graph> {
        if index > self.document.events.len() {
            return Err(bad("state index outside trace"));
        }
        let checkpoint = self
            .checkpoints
            .iter()
            .rev()
            .find(|c| c.state <= index)
            .expect("state-zero checkpoint");
        let mut execution = checkpoint.execution.fork();
        for event in &self.document.events[checkpoint.state..index] {
            checked_step(&mut execution, event)?;
        }
        Ok(execution.graph().clone())
    }
    pub fn state_from_initial(&self, index: usize) -> Result<Graph> {
        if index > self.document.events.len() {
            return Err(bad("state index outside trace"));
        }
        let graph = self
            .document
            .initial
            .restore(&self.document.settings.graph.native()?)?;
        let mut execution = if self
            .document
            .settings
            .execution_profile
            .unwrap_or_default()
            .uses_frames()
        {
            Execution::with_frame_validation(graph, self.document.settings.execution()?)?
        } else {
            Execution::new(graph, self.document.settings.execution()?)?
        };
        for event in &self.document.events[..index] {
            checked_step(&mut execution, event)?;
        }
        Ok(execution.graph().clone())
    }
    /// Historical inspection is not a branch. Only an explicitly yielded endpoint resumes.
    pub fn resume_at(&self, index: usize) -> Result<Runner<Scheduler>> {
        if self.document.status != "yielded" || index != self.document.events.len() {
            return Err(bad(
                "only the yielded endpoint can resume; historical branching requires a new run",
            ));
        }
        Ok(self
            .checkpoints
            .last()
            .and_then(|c| c.runner.as_ref())
            .ok_or_else(|| bad("no continuation"))?
            .fork())
    }
}
fn checked_step(execution: &mut Execution, event: &RecordedEvent) -> Result<()> {
    checked_recorded_step(execution, event, None)
}
fn checked_recorded_step(
    execution: &mut Execution,
    event: &RecordedEvent,
    recording: Option<&mut Recording>,
) -> Result<()> {
    let g = execution.graph();
    let resolve = |id: crate::recording::AtomId| {
        g.key(id.0)
            .ok_or_else(|| bad("binding references missing atom"))
    };
    if event.redex.binding.windows(2).any(|w| w[0].0 >= w[1].0) {
        return Err(bad("duplicate or unordered binding"));
    }
    let binding = OperationalBinding::new(
        resolve(event.redex.source)?,
        event
            .redex
            .binding
            .iter()
            .map(|&(a, b)| Ok((resolve(a)?, resolve(b)?)))
            .collect::<Result<_>>()?,
    );
    let prepared = execution.prepare(binding)?;
    if RecordedEvent::preview(execution.committed_steps(), &prepared)? != *event {
        return Err(bad("recorded event disagrees with checked transaction"));
    }
    if let Some(log) = recording {
        log.commit(execution, prepared)?;
    } else {
        execution.commit(prepared)?;
    }
    Ok(())
}
