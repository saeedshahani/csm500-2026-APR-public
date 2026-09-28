use engine::observation::Observer;
use engine::runner::{Outcome, RunLimits, Runner};
use engine::scheduling::Scheduler;
use engine::ExecutionLimits;
use graph_core::{Diagnostic, ErrorKind, Kind};
use parser::{load_with_limits, print_with_limits, source_position, ParseLimits};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const HELP: &str = "usage: cli [demo]\n       cli check INPUT [limits] [--output PATH]\n       cli run INPUT --max-steps N [run options] [--output PATH]\n       cli export INPUT --max-steps N [run options] [--checkpoint-interval N] [--conflict-state N] [--output PATH]\n       cli trace-check TRACE\n       cli replay TRACE [--at N] [--output PATH]\n\nINPUT/TRACE is a UTF-8 file, or - for standard input.\nrun/export use the configured scheduler and checked execution.\n--rule restricts steps to the named initial rule, not a global normal-form query.\n--max-steps is required; zero checks without committing.\nLimits: --max-candidates N, --max-work N (run/export); --max-atoms N,\n        --max-fields N, --max-label-bytes N, --max-input-bytes N,\n        --max-validation-work N, --max-package-depth N.\nAll limits are non-negative decimal integers. Defaults follow the native API.\nTrace import/export uses the default TraceLimits policy (see docs/reference/trace-format.md).\nrun/check/replay emit loadable graph text; export emits only trace JSON.\ntrace-check verifies bindings, derived analysis and any scheduler claims.\nreplay inspects history; it does not append or branch.\n--output publishes a new file only; existing files are never overwritten.\nExit codes: 0 valid/normal form; 1 engine/output failure; 2 arguments/input failure;\n            3 resource/step limit; 4 requested rule unavailable; 5 cancelled; 6 recurrence.\nA finished trace may describe a bounded prefix: inspect its report, not status alone.\ncheck establishes validity, not normal form. Limits do not prove divergence.\n";

pub(crate) struct Failure {
    pub code: u8,
    pub message: String,
}
impl Failure {
    fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
type Result<T> = std::result::Result<T, Failure>;
// Operator-selected local route; never inferred from untrusted trace settings.
fn trace_limits() -> engine::trace::TraceLimits {
    let mut limits = engine::trace::TraceLimits::default();
    if std::env::var("GRAPH_EXECUTION_ENVELOPE").as_deref() == Ok("large-local-v1") {
        limits.max_bytes = 128 * 1024 * 1024;
        limits.max_nodes = 8_000_000;
        limits.max_events = 20_000;
        limits.max_atoms_total = 500_000;
        limits.max_fields_total = 1_000_000;
        limits.max_bindings = 200_000;
        limits.execution.graph.max_atoms = 50_000;
        limits.execution.graph.max_fields = 100_000;
        limits.execution.graph.max_input_bytes = 16 * 1024 * 1024;
        limits.execution.graph.max_label_bytes = 8 * 1024 * 1024;
        limits.execution.search.max_candidates = 20_000;
        limits.execution.search.max_work = 10_000_000;
        limits.recording.max_events = 20_000;
        limits.recording.max_units = 50_000_000;
        limits.recording.max_work = 50_000_000;
        limits.observation.max_history_units = 50_000_000;
    }
    limits
}
fn lambda_failure(error: parser::lambda::Error) -> Failure {
    let code = match &error {
        parser::lambda::Error::ResourceLimit => 3,
        parser::lambda::Error::Core(d) if d.kind() == ErrorKind::ResourceExhaustion => 3,
        _ => 2,
    };
    Failure::new(code, error.to_string())
}
struct Options {
    execution_profile: engine::runner::ExecutionProfile,
    run: bool,
    export: bool,
    batch_steps: Option<u64>,
    trace_options: engine::trace::TraceOptions,
    input: String,
    output: Option<PathBuf>,
    max_steps: u64,
    rule: Option<String>,
    parse: ParseLimits,
    execution: ExecutionLimits,
    scheduler: String,
    seed: Option<u64>,
    observer: Option<String>,
    run_limits: RunLimits,
}

pub(crate) fn main_command() -> Result<u8> {
    if std::env::var("GRAPH_EXECUTION_ENVELOPE").is_ok_and(|v| v != "large-local-v1") {
        return Err(Failure::new(2, "Unknown operator execution envelope"));
    }
    let args = std::env::args_os()
        .skip(1)
        .map(|s| {
            s.into_string()
                .map_err(|_| Failure::new(2, "arguments must be UTF-8"))
        })
        .collect::<Result<Vec<_>>>()?;
    if args.is_empty() || args == ["demo"] {
        super::demo().map_err(|e| Failure::new(1, e.to_string()))?;
        return Ok(0);
    }
    if args == ["--help"]
        || args == ["-h"]
        || args == ["help"]
        || args == ["run", "--help"]
        || args == ["check", "--help"]
        || args == ["export", "--help"]
        || args == ["resume", "--help"]
        || args == ["replay", "--help"]
        || args == ["trace-check", "--help"]
        || args == ["trace-migrate", "--help"]
        || args == ["trace-conditions", "--help"]
        || args == ["dataset-compile", "--help"]
        || args == ["lambda-compile", "--help"]
        || args == ["lambda-readback", "--help"]
    {
        io::stdout().lock().write_all(b"Execution profiles: --execution-profile reference-v1 (default) | indexed-v2 | framed-v3 | joined-v4\nOperator envelope: GRAPH_EXECUTION_ENVELOPE=large-local-v1 enables the documented larger local source/trace limits; unset retains defaults.\nExplicit legacy migration: cli trace-migrate TRACE [--output NEW_PATH]\nMigration rechecks the full history under reference-v1; it never overwrites its input.\n").map_err(|e| Failure::new(1, e.to_string()))?;
        io::stdout().lock().write_all(b"Continuation: cli export INPUT --max-steps N --batch-steps N [export options]\n              cli resume TRACE --batch-steps N [--output PATH]\nA yielded checkpoint exits 0 without claiming completion. Resume preserves the original run limits and only accepts a verified yielded endpoint.\n").map_err(|e| Failure::new(1, e.to_string()))?;
        io::stdout().lock().write_all(b"Dataset adapter: cli dataset-compile JSON (work-items-v1; file or -)\nCondition report: cli trace-conditions TRACE (monotone constructor unquotation only)\n").map_err(|e| Failure::new(1, e.to_string()))?;
        io::stdout().lock().write_all(b"Lambda adapters: cli lambda-compile SOURCE | cli lambda-readback GRAPH\nBoth accept a file or -; compilation emits ordinary graph source.\nUse cli replay TRACE --at N | cli lambda-readback - for recorded terms.\n\n").map_err(|e| Failure::new(1, e.to_string()))?;
        io::stdout()
            .lock()
            .write_all(format!("{HELP}\nScheduling: --strategy deterministic (default), or --strategy seeded --seed U64.\nOrdering: source identity, then (template identity, host identity) tuples.\nObservers: --observer FILE (observer-v1); --detect-cycles true|false (default false).\nBounds: --max-observation-work N, --max-comparison-work N, --max-history-units N.\nRecurrence is diagnostic, not proof of infinite behaviour; exit 6 CycleDetected.\nShared API cancellation stops at transaction boundaries (exit 5); CLI does not install a signal handler.\nNormal form takes precedence over recurrence and step limits; recurrence precedes step limit.\n").as_bytes())
            .map_err(|e| Failure::new(1, format!("output: {e}")))?;
        return Ok(0);
    }
    if args[0] == "resume" {
        return resume_command(&args);
    }
    if matches!(
        args[0].as_str(),
        "replay" | "trace-check" | "trace-conditions" | "trace-migrate"
    ) {
        return trace_command(&args);
    }
    if args[0] == "dataset-compile" {
        if args.len() != 2 {
            return Err(Failure::new(
                2,
                "usage: cli dataset-compile JSON (file or -)",
            ));
        }
        let text = read_source(&args[1], 1024 * 1024)?;
        let dataset: parser::dataset::Dataset =
            serde_json::from_str(&text).map_err(|e| Failure::new(2, e.to_string()))?;
        let source = parser::dataset::source(&dataset).map_err(|e| Failure::new(2, e))?;
        parser::load(&source)
            .map_err(|e| Failure::new(2, format!("dataset compilation: {e:?}")))?;
        io::stdout()
            .lock()
            .write_all(source.as_bytes())
            .map_err(|e| Failure::new(1, e.to_string()))?;
        return Ok(0);
    }
    if matches!(args[0].as_str(), "lambda-compile" | "lambda-readback") {
        if args.len() != 2 {
            return Err(Failure::new(
                2,
                "usage: cli lambda-compile SOURCE | lambda-readback GRAPH (file or -)",
            ));
        }
        let text = read_source(&args[1], ParseLimits::default().graph.max_input_bytes)?;
        let limits = parser::lambda::Limits::default();
        let output = if args[0] == "lambda-compile" {
            let term = parser::lambda::parse(&text, limits).map_err(lambda_failure)?;
            let encoded = parser::lambda::encode(&term, limits).map_err(lambda_failure)?;
            format!(
                "// Lambda source: {}\n{}",
                text.trim().replace(['\n', '\r'], " "),
                parser::print(&encoded.graph).map_err(engine_failure)?
            )
        } else {
            let loaded = parser::load(&text).map_err(|errors| {
                let code = if errors
                    .iter()
                    .any(|e| e.diagnostic.kind() == ErrorKind::ResourceExhaustion)
                {
                    3
                } else {
                    2
                };
                Failure::new(code, format!("{errors:?}"))
            })?;
            parser::lambda::interpret(loaded.graph(), limits)
                .map_err(lambda_failure)?
                .ok_or_else(|| Failure::new(2, "no lambda machine"))?
        };
        io::stdout()
            .lock()
            .write_all(format!("{output}\n").as_bytes())
            .map_err(|e| Failure::new(1, e.to_string()))?;
        return Ok(0);
    }
    let options = options(&args)?;
    let source = read_source(&options.input, options.parse.graph.max_input_bytes)?;
    let loaded = load_with_limits(&source, &options.parse).map_err(|errors| {
        let code = if errors
            .iter()
            .any(|e| e.diagnostic.kind() == ErrorKind::ResourceExhaustion)
        {
            3
        } else {
            2
        };
        let mut message = String::new();
        for error in errors {
            let location = position(&source, error.span.start);
            message += &format!(
                "{}:{}:{}: {:?}: {}\n",
                options.input,
                location.0,
                location.1,
                error.diagnostic.kind(),
                error.diagnostic
            );
            for related in error.related {
                let location = position(&source, related.span.start);
                message += &format!(
                    "  {}:{}:{}: {}\n",
                    options.input, location.0, location.1, related.message
                );
            }
        }
        Failure::new(code, message)
    })?;
    let requested = options
        .rule
        .as_ref()
        .map(|name| {
            let key = loaded
                .name(name)
                .ok_or_else(|| Failure::new(2, format!("unknown rule name: {name}")))?;
            if loaded.graph().get(key).unwrap().kind != Kind::RuleHead {
                return Err(Failure::new(2, format!("not a rule head: {name}")));
            }
            Ok(key)
        })
        .transpose()?;
    let observer = if let Some(path) = &options.observer {
        let text = read_source(path, options.parse.graph.max_input_bytes)?;
        Observer::parse(
            &text,
            |name| loaded.name(name),
            options.parse.graph.max_input_bytes,
        )
        .map_err(|e| Failure::new(2, e.to_string()))?
    } else {
        Observer::all()
    };
    let scheduler = match options.seed {
        Some(seed) => Scheduler::seeded(seed),
        None => Scheduler::Deterministic,
    };
    let mut runner = Runner::with_profile(
        loaded.into_graph(),
        options.execution,
        scheduler,
        observer,
        options.execution_profile,
    )
    .map_err(engine_failure)?;
    if options.export {
        runner
            .enable_recording(trace_limits().recording)
            .map_err(engine_failure)?;
    }
    let mut outcome = "Valid";
    let mut code = 0;
    let mut rejected = 0;
    let mut last_diagnostic = None;
    let mut details = String::new();
    if let Some(batch) = options.batch_steps {
        runner
            .begin_run(options.run_limits, requested)
            .map_err(engine_failure)?;
        if let engine::runner::RunProgress::Finished(report) =
            runner.advance(batch, || false).map_err(engine_failure)?
        {
            code = outcome_code(report.outcome);
            last_diagnostic = report.diagnostic;
        }
    } else if options.run {
        let report = runner
            .run(options.run_limits, requested, || false)
            .map_err(engine_failure)?;
        (outcome, code) = match report.outcome {
            Outcome::NormalForm => ("NormalForm", 0),
            Outcome::RequestedRuleUnavailable => ("RequestedRuleUnavailable", 4),
            Outcome::ResourceLimit => ("ResourceLimit", 3),
            Outcome::StepLimit => ("StepLimit", 3),
            Outcome::Cancelled => ("Cancelled", 5),
            Outcome::CycleDetected => ("CycleDetected", 6),
        };
        rejected = report.rejected;
        last_diagnostic = report.diagnostic;
        details+=&format!("// scheduler: {}\n// observer: {}\n// value-payload: {:?}; nontriviality-threshold: {}\n",report.scheduler,report.observer,report.payload,report.nontriviality_threshold);
        details+=&format!("// observation-limits: projection-work={} comparison-work={} history-units={}; detect-cycles={}\n",options.run_limits.observation.max_work,options.run_limits.observation.max_comparison_work,options.run_limits.observation.max_history_units,options.run_limits.detect_cycles);
        if let Some(cycle) = report.recurrence {
            details+=&format!("// recurrence: {} -> {}; exact-attributed-carrier-isomorphism; complete-state-equal={}\n// first-scheduler: {}\n// repeated-scheduler: {}\n// recurrence is not a proof of full execution-state recurrence, convergence, oscillation or divergence\n",cycle.first,cycle.repeated,cycle.complete_state_equal,cycle.first_scheduler,cycle.repeated_scheduler);
        }
    }
    if options.export {
        let doc = engine::trace::TraceDocument::from_runner(
            &runner,
            Some(engine::trace::sha256(source.as_bytes())),
            options.trace_options,
            trace_limits(),
        )
        .map_err(engine_failure)?;
        let bytes = doc
            .to_json(trace_limits().max_bytes)
            .map_err(engine_failure)?;
        if let Some(path) = &options.output {
            publish_new(path, &bytes)?;
        } else {
            io::stdout()
                .lock()
                .write_all(&bytes)
                .map_err(|e| Failure::new(1, e.to_string()))?;
        }
        if let Some(error) = last_diagnostic {
            eprintln!("{:?}: {}", error.kind(), error);
        }
        return Ok(code);
    }
    let execution = runner.execution();
    let graph = print_with_limits(execution.graph(), &options.parse).map_err(engine_failure)?;
    let policy = if !options.run {
        "none"
    } else if options.seed.is_some() {
        "seeded-source-template-host-v1"
    } else if requested.is_some() {
        "requested-source-template-host-v1"
    } else {
        "deterministic-source-template-host-v1"
    };
    let mut output=format!("// outcome: {outcome}\n// committed-steps: {}\n// selection-policy: {policy}\n// rejected-in-last-complete-query: {rejected}\n",execution.committed_steps());
    output+=&format!("// max-steps: {}\n// limits: candidates={} work={} atoms={} fields={} label-bytes={} input-bytes={} validation-work={} package-depth={}\n",options.max_steps,options.execution.search.max_candidates,options.execution.search.max_work,options.parse.graph.max_atoms,options.parse.graph.max_fields,options.parse.graph.max_label_bytes,options.parse.graph.max_input_bytes,options.parse.graph.max_validation_work,options.parse.graph.max_package_depth);
    if let Some(rule) = &options.rule {
        output += &format!("// requested-initial-rule: {rule}\n");
    }
    output += &graph;
    output += &details;
    if output.len() > options.parse.graph.max_input_bytes {
        return Err(Failure::new(
            3,
            "ResourceExhaustion: final report byte limit exceeded",
        ));
    }
    if let Some(path) = &options.output {
        publish_new(path, output.as_bytes())?;
    } else {
        io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(|e| Failure::new(1, format!("output: {e}")))?;
    }
    if let Some(error) = last_diagnostic {
        eprintln!("{:?}: {}", error.kind(), error);
    }
    if code == 4 {
        eprintln!(
            "requested rule has no enabled candidate; this is not a global normal-form result"
        );
    }
    Ok(code)
}

fn position(source: &str, offset: usize) -> (usize, usize) {
    source_position(source, offset).map_or((1, 1), |p| (p.line, p.column))
}

fn outcome_code(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::NormalForm => 0,
        Outcome::ResourceLimit | Outcome::StepLimit => 3,
        Outcome::RequestedRuleUnavailable => 4,
        Outcome::Cancelled => 5,
        Outcome::CycleDetected => 6,
    }
}

fn resume_command(args: &[String]) -> Result<u8> {
    if args.len() < 4 {
        return Err(Failure::new(
            2,
            "usage: cli resume TRACE --batch-steps N [--output PATH]",
        ));
    }
    let mut batch = None;
    let mut output = None;
    let mut seen = BTreeSet::new();
    for pair in args[2..].chunks(2) {
        if pair.len() != 2 || !seen.insert(&pair[0]) {
            return Err(Failure::new(2, "missing or duplicate resume option"));
        }
        match pair[0].as_str() {
            "--batch-steps" => batch = Some(number::<u64>(&pair[1], "--batch-steps")?),
            "--output" => output = Some(PathBuf::from(&pair[1])),
            _ => {
                return Err(Failure::new(
                    2,
                    "unsupported resume option; run settings cannot be changed",
                ))
            }
        }
    }
    let batch = batch
        .filter(|n| *n > 0)
        .ok_or_else(|| Failure::new(2, "positive --batch-steps required"))?;
    let limits = trace_limits();
    let text = read_source(&args[1], limits.max_bytes)?;
    let verified =
        engine::trace::VerifiedTrace::import(text.as_bytes(), limits).map_err(engine_failure)?;
    let document = verified.document();
    let mut runner = verified
        .resume_at(document.events.len())
        .map_err(engine_failure)?;
    let code = match runner.advance(batch, || false).map_err(engine_failure)? {
        engine::runner::RunProgress::Yielded => 0,
        engine::runner::RunProgress::Finished(report) => outcome_code(report.outcome),
    };
    let options = engine::trace::TraceOptions {
        checkpoint_interval: document.checkpoint_interval.0,
        conflict_states: document.conflicts.iter().map(|c| c.state.0).collect(),
    };
    let bytes = engine::trace::TraceDocument::from_runner(
        &runner,
        document.source_sha256.clone(),
        options,
        limits,
    )
    .map_err(engine_failure)?
    .to_json(limits.max_bytes)
    .map_err(engine_failure)?;
    if let Some(path) = output {
        publish_new(&path, &bytes)?;
    } else {
        io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|e| Failure::new(1, e.to_string()))?;
    }
    Ok(code)
}

fn trace_command(args: &[String]) -> Result<u8> {
    if args.len() < 2 {
        return Err(Failure::new(
            2,
            "usage: cli trace-check TRACE | cli replay TRACE [--at N] [--output PATH]",
        ));
    }
    let mut index = None;
    let mut output = None;
    let mut seen = BTreeSet::new();
    for pair in args[2..].chunks(2) {
        if pair.len() != 2 || !seen.insert(&pair[0]) {
            return Err(Failure::new(2, "missing or duplicate replay option"));
        }
        match pair[0].as_str() {
            "--at" if args[0] == "replay" => index = Some(number::<usize>(&pair[1], "--at")?),
            "--output" if args[0] == "replay" || args[0] == "trace-migrate" => {
                output = Some(PathBuf::from(&pair[1]))
            }
            _ => return Err(Failure::new(2, "unsupported replay option")),
        }
    }
    let limits = trace_limits();
    let text = read_source(&args[1], limits.max_bytes)?;
    let trace = (if args[0] == "trace-migrate" {
        engine::trace::VerifiedTrace::migrate_reference(text.as_bytes(), limits)
    } else {
        engine::trace::VerifiedTrace::import(text.as_bytes(), limits)
    })
    .map_err(|e| {
        Failure::new(
            if e.kind() == ErrorKind::ResourceExhaustion {
                3
            } else {
                2
            },
            e.to_string(),
        )
    })?;
    let result = if args[0] == "trace-migrate" {
        String::from_utf8(
            trace
                .document()
                .to_json(limits.max_bytes)
                .map_err(engine_failure)?,
        )
        .map_err(|e| Failure::new(1, e.to_string()))?
    } else if args[0] == "trace-conditions" {
        let report = engine::conditions::extract(&trace, engine::causal::AnalysisLimits::default())
            .map_err(engine_failure)?;
        let value = serde_json::json!({"source_trace_sha256": engine::trace::sha256(text.as_bytes()), "analysis": report});
        format!(
            "{}\n",
            serde_json::to_string_pretty(&value).map_err(|e| Failure::new(1, e.to_string()))?
        )
    } else if args[0] == "trace-check" {
        format!(
            "Verified {} events; status={}; relation={}\n",
            trace.document().events.len(),
            trace.document().status,
            trace.document().relation
        )
    } else {
        let state = index.unwrap_or(trace.document().events.len());
        let graph = trace.state_at(state).map_err(engine_failure)?;
        format!(
            "// verified trace state: {state}; historical inspection only\n{}",
            print_with_limits(&graph, &ParseLimits::default()).map_err(engine_failure)?
        )
    };
    if let Some(path) = output {
        publish_new(&path, result.as_bytes())?;
    } else {
        io::stdout()
            .lock()
            .write_all(result.as_bytes())
            .map_err(|e| Failure::new(1, e.to_string()))?;
    }
    Ok(0)
}

fn options(args: &[String]) -> Result<Options> {
    let run = match args[0].as_str() {
        "run" | "export" => true,
        "check" => false,
        _ => return Err(Failure::new(2, HELP)),
    };
    let mut result = Options {
        execution_profile: Default::default(),
        run,
        export: args[0] == "export",
        batch_steps: None,
        trace_options: Default::default(),
        input: String::new(),
        output: None,
        max_steps: 0,
        rule: None,
        parse: ParseLimits {
            graph: trace_limits().execution.graph,
            max_tokens: if trace_limits().max_events > 10_000 {
                1_000_000
            } else {
                ParseLimits::default().max_tokens
            },
            ..Default::default()
        },
        execution: trace_limits().execution,
        scheduler: "deterministic".into(),
        seed: None,
        observer: None,
        run_limits: RunLimits {
            observation: trace_limits().observation,
            ..Default::default()
        },
    };
    let mut seen = BTreeSet::new();
    let mut i = 1;
    let mut positional = false;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        if !positional && arg == "--" {
            positional = true;
            continue;
        }
        if !positional && arg.starts_with("--") {
            if !seen.insert(arg.clone()) {
                return Err(Failure::new(2, format!("duplicate option: {arg}")));
            }
            let value = args
                .get(i)
                .ok_or_else(|| Failure::new(2, format!("missing value for {arg}")))?;
            i += 1;
            match arg.as_str() {
                "--batch-steps" if result.export => result.batch_steps = Some(number(value, arg)?),
                "--checkpoint-interval" if result.export => {
                    result.trace_options.checkpoint_interval = number(value, arg)?
                }
                "--conflict-state" if result.export => result
                    .trace_options
                    .conflict_states
                    .push(number(value, arg)?),
                "--output" => result.output = Some(PathBuf::from(value)),
                "--rule" if run => result.rule = Some(value.clone()),
                "--strategy" if run => result.scheduler = value.clone(),
                "--seed" if run => result.seed = Some(number(value, arg)?),
                "--execution-profile" if run => {
                    result.execution_profile =
                        match value.as_str() {
                            "reference-v1" => engine::runner::ExecutionProfile::ReferenceV1,
                            "indexed-v2" => engine::runner::ExecutionProfile::IndexedV2,
                            "framed-v3" => engine::runner::ExecutionProfile::FramedV3,
                            "joined-v4" => engine::runner::ExecutionProfile::JoinedV4,
                            _ => return Err(Failure::new(
                                2,
                                "--execution-profile expects reference-v1, indexed-v2, framed-v3 or joined-v4",
                            )),
                        }
                }
                "--observer" => result.observer = Some(value.clone()),
                "--detect-cycles" if run => {
                    result.run_limits.detect_cycles = match value.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => return Err(Failure::new(2, "--detect-cycles expects true or false")),
                    }
                }
                "--max-observation-work" if run => {
                    result.run_limits.observation.max_work = number(value, arg)?
                }
                "--max-comparison-work" if run => {
                    result.run_limits.observation.max_comparison_work = number(value, arg)?
                }
                "--max-history-units" if run => {
                    result.run_limits.observation.max_history_units = number(value, arg)?
                }
                "--max-steps" if run => result.max_steps = number(value, arg)?,
                "--max-candidates" if run => {
                    result.execution.search.max_candidates = number(value, arg)?
                }
                "--max-work" if run => result.execution.search.max_work = number(value, arg)?,
                "--max-atoms" => result.parse.graph.max_atoms = number(value, arg)?,
                "--max-fields" => result.parse.graph.max_fields = number(value, arg)?,
                "--max-label-bytes" => result.parse.graph.max_label_bytes = number(value, arg)?,
                "--max-input-bytes" => result.parse.graph.max_input_bytes = number(value, arg)?,
                "--max-validation-work" => {
                    result.parse.graph.max_validation_work = number(value, arg)?
                }
                "--max-package-depth" => result.parse.graph.max_package_depth = number(value, arg)?,
                _ => {
                    return Err(Failure::new(
                        2,
                        format!("unsupported option for {}: {arg}\n{HELP}", args[0]),
                    ))
                }
            }
        } else if result.input.is_empty() {
            result.input = arg.clone();
        } else {
            return Err(Failure::new(
                2,
                format!("unexpected argument: {arg}\n{HELP}"),
            ));
        }
    }
    if result.input.is_empty() || (run && !seen.contains("--max-steps")) {
        return Err(Failure::new(2, HELP));
    }
    result.execution.graph = result.parse.graph;
    if result.batch_steps == Some(0) {
        return Err(Failure::new(2, "--batch-steps must be positive"));
    }
    result.run_limits.max_steps = result.max_steps;
    match (result.scheduler.as_str(), result.seed) {
        ("deterministic", None) | ("seeded", Some(_)) => {}
        _ => {
            return Err(Failure::new(
                2,
                "use --strategy deterministic without a seed, or --strategy seeded --seed U64",
            ))
        }
    }
    Ok(result)
}
fn number<T: std::str::FromStr>(value: &str, option: &str) -> Result<T> {
    if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(Failure::new(
            2,
            format!("{option} requires a non-negative decimal integer"),
        ));
    }
    value
        .parse()
        .map_err(|_| Failure::new(2, format!("{option} is out of range")))
}
fn engine_failure(error: Diagnostic) -> Failure {
    Failure::new(
        if error.kind() == ErrorKind::ResourceExhaustion {
            3
        } else {
            1
        },
        format!("{:?}: {}", error.kind(), error),
    )
}
fn read_source(path: &str, maximum: usize) -> Result<String> {
    let bound = u64::try_from(maximum)
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| Failure::new(2, "input byte bound is out of range"))?;
    let reader: Box<dyn Read> = if path == "-" {
        Box::new(io::stdin())
    } else {
        Box::new(File::open(path).map_err(|e| Failure::new(2, format!("input {path}: {e}")))?)
    };
    let mut bytes = Vec::new();
    reader
        .take(bound)
        .read_to_end(&mut bytes)
        .map_err(|e| Failure::new(2, format!("input {path}: {e}")))?;
    if bytes.len() > maximum {
        return Err(Failure::new(
            3,
            "ResourceExhaustion: input byte limit exceeded",
        ));
    }
    String::from_utf8(bytes).map_err(|_| Failure::new(2, "input is not valid UTF-8"))
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn publish_new(destination: &Path, bytes: &[u8]) -> Result<()> {
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for attempt in 0..100 {
        let path = parent.join(format!(
            ".graph-output-{}-{attempt}.tmp",
            std::process::id()
        ));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(Failure::new(
                    1,
                    format!("output {}: {e}", destination.display()),
                ))
            }
        };
        let temporary = Temporary(path);
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| Failure::new(1, format!("output {}: {e}", destination.display())))?;
        drop(file);
        // A same-directory hard link publishes the complete file atomically and
        // fails if any destination (including a symlink) already exists.
        fs::hard_link(&temporary.0, destination)
            .map_err(|e| Failure::new(1, format!("output {}: {e}", destination.display())))?;
        return Ok(());
    }
    Err(Failure::new(1, "cannot allocate temporary output file"))
}
