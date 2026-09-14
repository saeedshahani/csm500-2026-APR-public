use engine::{
    observation::Observer,
    recording::{AtomId, RecordedEvent},
    runner::{RunLimits, RunProgress, Runner},
    scheduling::Scheduler,
    trace::*,
    ExecutionLimits,
};
use graph_core::{Diagnostic, ErrorKind, Graph, ValidatedGraph};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub max_steps: Number,
    pub seed: Option<Number>,
    pub detect_cycles: bool,
    pub observer: String,
    pub requested: Option<String>,
    pub max_work: Number,
    pub max_candidates: Number,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_steps: Number(100),
            seed: None,
            detect_cycles: false,
            observer: Observer::all().canonical().into(),
            requested: None,
            max_work: Number(1_000_000),
            max_candidates: Number(1000),
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Load {
        source: String,
        #[serde(default)]
        config: Config,
    },
    Validate {
        source: String,
    },
    Advance {
        batch: Number,
    },
    Stop,
    Reset,
    State,
    Export,
    Import {
        trace: String,
    },
    Jump {
        state: Number,
    },
    Analyse {
        event: Option<Number>,
        max_work: Option<Number>,
    },
    Dispose,
}
#[derive(Clone, Debug, Serialize)]
pub struct BridgeError {
    pub kind: String,
    pub message: String,
    pub line: Option<Number>,
    pub column: Option<Number>,
}
impl From<Diagnostic> for BridgeError {
    fn from(e: Diagnostic) -> Self {
        Self {
            kind: format!("{:?}", e.kind()),
            message: e.to_string(),
            line: None,
            column: None,
        }
    }
}
type Result<T> = std::result::Result<T, BridgeError>;
fn error(message: &str) -> BridgeError {
    Diagnostic::new(ErrorKind::InvalidConfiguration, message).into()
}
fn resource(message: &str) -> BridgeError {
    Diagnostic::new(ErrorKind::ResourceExhaustion, message).into()
}
#[derive(Clone, Debug, Serialize)]
pub struct RuleData {
    pub head: AtomId,
    pub revision: AtomId,
    pub rank: Number,
    pub active: bool,
    pub meta: bool,
    pub lhs: Vec<AtomId>,
    pub rhs: Vec<AtomId>,
    pub preserved: Vec<AtomId>,
    pub representation: Vec<AtomId>,
}
#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub graph: GraphData,
    pub rules: Vec<RuleData>,
    pub quoted: BTreeMap<AtomId, AtomId>,
    pub events: Vec<RecordedEvent>,
    pub dependencies: Vec<EdgeData>,
    pub relation: String,
    pub limitation: String,
    pub position: Number,
    pub endpoint: Number,
    pub status: String,
    pub can_advance: bool,
    pub report: Option<ReportData>,
    pub previous: Option<GraphData>,
    pub names: BTreeMap<AtomId, String>,
    pub observer: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    pub state: Number,
    pub local: Option<ConflictData>,
    pub ancestors: Vec<engine::recording::EventId>,
    pub descendants: Vec<engine::recording::EventId>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Reply {
    pub ok: bool,
    pub error: Option<BridgeError>,
    pub state: Option<State>,
    pub trace: Option<String>,
    pub analysis: Option<Analysis>,
}
impl Reply {
    fn empty() -> Self {
        Self {
            ok: true,
            error: None,
            state: None,
            trace: None,
            analysis: None,
        }
    }
}
struct Session {
    runner: Option<Runner<Scheduler>>,
    verified: VerifiedTrace,
    position: usize,
    status: String,
    names: BTreeMap<AtomId, String>,
}
impl Session {
    fn from_runner(
        runner: Runner<Scheduler>,
        source_hash: Option<String>,
        status: String,
    ) -> Result<Self> {
        let doc = TraceDocument::from_runner(
            &runner,
            source_hash,
            Default::default(),
            Default::default(),
        )?;
        let verified = VerifiedTrace::verify(doc, Default::default())?;
        let position = verified.document().events.len();
        Ok(Self {
            runner: Some(runner),
            verified,
            position,
            status,
            names: BTreeMap::new(),
        })
    }
    fn from_trace(text: &str) -> Result<Self> {
        let verified = VerifiedTrace::import(text.as_bytes(), Default::default())?;
        let position = verified.document().events.len();
        let runner = if verified.document().status == "yielded" {
            Some(verified.resume_at(position)?)
        } else {
            None
        };
        let status = verified.document().report.as_ref().map_or_else(
            || {
                if runner.is_some() {
                    "Paused".into()
                } else {
                    "RecordedPrefix".into()
                }
            },
            |r| r.outcome.clone(),
        );
        Ok(Self {
            runner,
            verified,
            position,
            status,
            names: BTreeMap::new(),
        })
    }
    fn reply(&self) -> Result<Reply> {
        let doc = self.verified.document();
        let graph = self.verified.state_at(self.position)?;
        let view = ValidatedGraph::new(&graph, &doc.settings.graph.native()?)?;
        let rules = view
            .configuration()
            .rules
            .values()
            .map(|r| RuleData {
                head: r.head.into(),
                revision: r.revision.into(),
                rank: Number(r.rank as u64),
                active: r.active,
                meta: r.meta,
                lhs: r.lhs.iter().copied().map(Into::into).collect(),
                rhs: r.rhs.iter().copied().map(Into::into).collect(),
                preserved: r
                    .lhs
                    .intersection(&r.rhs)
                    .copied()
                    .map(Into::into)
                    .collect(),
                representation: r.representation.iter().copied().map(Into::into).collect(),
            })
            .collect();
        let state = State {
            graph: GraphData::from_graph(&graph),
            rules,
            quoted: view
                .configuration()
                .quoted
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
            events: doc.events.clone(),
            dependencies: doc.dependencies.clone().unwrap_or_default(),
            relation: doc.relation.clone(),
            limitation: doc.limitation.clone(),
            position: Number(self.position as u64),
            endpoint: Number(doc.events.len() as u64),
            status: self.status.clone(),
            can_advance: self.position == doc.events.len()
                && self.runner.as_ref().is_some_and(Runner::is_active),
            report: doc.report.clone(),
            previous: if self.position > 0 {
                Some(GraphData::from_graph(
                    &self.verified.state_at(self.position - 1)?,
                ))
            } else {
                None
            },
            names: self.names.clone(),
            observer: doc.settings.observer.declaration.clone(),
        };
        let trace = String::from_utf8(doc.to_json(TraceLimits::default().max_bytes)?)
            .map_err(|_| error("internal UTF-8 failure"))?;
        let reply = Reply {
            ok: true,
            error: None,
            state: Some(state),
            trace: Some(trace),
            analysis: None,
        };
        let size = serde_json::to_vec(&reply)
            .map_err(|e| error(&e.to_string()))?
            .len();
        if size > 32 * 1024 * 1024 {
            return Err(resource("bridge reply byte limit"));
        }
        Ok(reply)
    }
}
#[derive(Default)]
pub struct Workbench {
    session: Option<Session>,
    disposed: bool,
}
impl Workbench {
    pub fn dispatch(&mut self, request: &str) -> Reply {
        let result = (|| {
            if self.disposed {
                return Err(error("handle is disposed"));
            }
            if request.len() > 20 * 1024 * 1024 {
                return Err(resource("bridge request byte limit"));
            }
            let command: Command = serde_json::from_str(request).map_err(|e| BridgeError {
                kind: "MalformedInput".into(),
                message: e.to_string(),
                line: Some(Number(e.line() as u64)),
                column: Some(Number(e.column() as u64)),
            })?;
            self.execute(command)
        })();
        result.unwrap_or_else(|error| Reply {
            ok: false,
            error: Some(error),
            state: None,
            trace: None,
            analysis: None,
        })
    }
    fn publish(&mut self, next: Session) -> Result<Reply> {
        let reply = next.reply()?;
        self.session = Some(next);
        Ok(reply)
    }
    fn execute(&mut self, command: Command) -> Result<Reply> {
        match command {
            Command::Dispose => {
                self.session = None;
                self.disposed = true;
                Ok(Reply::empty())
            }
            Command::Validate { source } => {
                load(&source)?;
                Ok(Reply::empty())
            }
            Command::Load { source, config } => {
                if config.max_steps.0 > 10_000
                    || config.max_work.0 > 1_000_000
                    || config.max_candidates.0 > 1000
                {
                    return Err(resource("configuration exceeds portable workbench policy"));
                }
                let input = load(&source)?;
                let names = input
                    .names()
                    .iter()
                    .map(|(name, key)| ((*key).into(), name.clone()))
                    .collect();
                let observer =
                    Observer::parse(&config.observer, |n| input.name(n), 4 * 1024 * 1024)?;
                let requested = config
                    .requested
                    .as_ref()
                    .map(|n| input.name(n).ok_or_else(|| error("unknown requested rule")))
                    .transpose()?;
                if requested.is_some_and(|k| {
                    input.graph().get(k).unwrap().kind != graph_core::Kind::RuleHead
                }) {
                    return Err(error("requested name is not a rule"));
                }
                let mut limits = ExecutionLimits::default();
                limits.search.max_work = config.max_work.size()?;
                limits.search.max_candidates = config.max_candidates.size()?;
                let mut runner = Runner::new(
                    input.into_graph(),
                    limits,
                    config
                        .seed
                        .map_or(Scheduler::Deterministic, |n| Scheduler::seeded(n.0)),
                    observer,
                )?;
                runner.enable_recording(Default::default())?;
                runner.begin_run(
                    RunLimits {
                        max_steps: config.max_steps.0,
                        detect_cycles: config.detect_cycles,
                        ..Default::default()
                    },
                    requested,
                )?;
                let mut next =
                    Session::from_runner(runner, Some(sha256(source.as_bytes())), "Ready".into())?;
                next.names = names;
                self.publish(next)
            }
            Command::Import { trace } => self.publish(Session::from_trace(&trace)?),
            other => {
                let current = self
                    .session
                    .as_mut()
                    .ok_or_else(|| error("no loaded session"))?;
                match other {
                    Command::State | Command::Export => current.reply(),
                    Command::Analyse { event, max_work } => {
                        let work = max_work.unwrap_or(Number(2_000_000));
                        if work.0 > 2_000_000 {
                            return Err(resource("analysis exceeds portable workbench policy"));
                        }
                        let limits = engine::causal::AnalysisLimits {
                            max_work: work.size()?,
                            max_units: 2_000_000,
                        };
                        let mut analysis = Analysis {
                            state: Number(current.position as u64),
                            local: None,
                            ancestors: vec![],
                            descendants: vec![],
                        };
                        if let Some(event) = event {
                            let order = engine::causal::DependencyOrder::extract(
                                &current.verified.document().events,
                                limits,
                            )?;
                            analysis.ancestors = order
                                .ancestors(engine::recording::EventId(event.0), limits)?
                                .into_iter()
                                .collect();
                            analysis.descendants = order
                                .descendants(engine::recording::EventId(event.0), limits)?
                                .into_iter()
                                .collect();
                        } else {
                            let execution = engine::Execution::new(
                                current.verified.state_at(current.position)?,
                                current.verified.document().settings.execution()?,
                            )?;
                            let mut snapshot =
                                engine::conflict::LocalSnapshot::analyse(&execution, limits)?;
                            // The isolated query starts its own counter at zero; public
                            // redex identities refer to the selected verified history state.
                            snapshot.state = current.position as u64;
                            for redex in &mut snapshot.alternatives {
                                redex.state = current.position as u64;
                            }
                            analysis.local = Some(ConflictData::from_snapshot(snapshot)?);
                        }
                        let reply = Reply {
                            analysis: Some(analysis),
                            ..Reply::empty()
                        };
                        if serde_json::to_vec(&reply)
                            .map_err(|e| error(&e.to_string()))?
                            .len()
                            > 32 * 1024 * 1024
                        {
                            return Err(resource("bridge analysis reply byte limit"));
                        }
                        Ok(reply)
                    }
                    Command::Stop => {
                        let old = current.status.clone();
                        if current.runner.as_ref().is_some_and(Runner::is_active) {
                            current.status = "Paused".into();
                        }
                        let reply = current.reply();
                        if reply.is_err() {
                            current.status = old;
                        }
                        reply
                    }
                    Command::Jump { state } => {
                        if state.0 > current.verified.document().events.len() as u64 {
                            return Err(error("state index outside trace"));
                        }
                        let state = state.size()?;
                        current.verified.state_at(state)?;
                        let old = current.position;
                        current.position = state;
                        let reply = current.reply();
                        if reply.is_err() {
                            current.position = old;
                        }
                        reply
                    }
                    Command::Advance { batch } => {
                        if batch.0 == 0 || batch.0 > 100 {
                            return Err(error("batch must be between 1 and 100"));
                        }
                        if current.position != current.verified.document().events.len() {
                            return Err(error("historical inspection cannot append; return to the endpoint or reset"));
                        }
                        let mut runner = current
                            .runner
                            .as_ref()
                            .ok_or_else(|| error("trace endpoint is not resumable"))?
                            .fork();
                        let status = match runner.advance(batch.0, || false)? {
                            RunProgress::Yielded => "Yielded".into(),
                            RunProgress::Finished(report) => format!("{:?}", report.outcome),
                        };
                        let mut next = Session::from_runner(
                            runner,
                            current.verified.document().source_sha256.clone(),
                            status,
                        )?;
                        next.names = current.names.clone();
                        self.publish(next)
                    }
                    Command::Reset => {
                        let doc = current.verified.document();
                        let graph: Graph = doc.initial.restore(&doc.settings.graph.native()?)?;
                        let observer = doc.settings.observer.restore(&graph, 4 * 1024 * 1024)?;
                        let plan = doc.settings.plan.clone().unwrap_or(PlanData {
                            max_steps: Number(100),
                            detect_cycles: false,
                            observation: engine::observation::ObservationLimits::default().into(),
                            requested: None,
                            seed: None,
                        });
                        let requested = plan
                            .requested
                            .map(|id| {
                                graph
                                    .key(id.0)
                                    .ok_or_else(|| error("missing initial source"))
                            })
                            .transpose()?;
                        let mut runner = Runner::new(
                            graph,
                            doc.settings.execution()?,
                            plan.seed
                                .map_or(Scheduler::Deterministic, |n| Scheduler::seeded(n.0)),
                            observer,
                        )?;
                        runner.enable_recording(doc.settings.recording.native()?)?;
                        runner.begin_run(plan.limits()?, requested)?;
                        let mut next = Session::from_runner(
                            runner,
                            doc.source_sha256.clone(),
                            "Ready".into(),
                        )?;
                        next.names = current.names.clone();
                        self.publish(next)
                    }
                    _ => unreachable!("handled session-independent command"),
                }
            }
        }
    }
}
fn load(source: &str) -> Result<parser::LoadedProgram> {
    parser::load(source).map_err(|errors| {
        let first = &errors[0];
        let position = parser::source_position(source, first.span.start);
        BridgeError {
            kind: format!("{:?}", first.diagnostic.kind()),
            message: errors
                .iter()
                .map(|e| e.diagnostic.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            line: position.map(|p| Number(p.line as u64)),
            column: position.map(|p| Number(p.column as u64)),
        }
    })
}
