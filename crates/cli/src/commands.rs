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

const HELP: &str = "usage: cli [demo]\n       cli check INPUT [limits] [--output PATH]\n       cli run INPUT --max-steps N [run options] [--output PATH]\n       cli export INPUT --max-steps N [run options] [--checkpoint-interval N] [--conflict-state N] [--output PATH]\n       cli trace-check TRACE\n       cli replay TRACE [--at N] [--output PATH]\n\nINPUT/TRACE is a UTF-8 file, or - for standard input.\nrun/export use the configured scheduler and checked execution.\n--rule restricts steps to the named initial rule, not a global normal-form query.\n--max-steps is required; zero checks without committing.\nLimits: --max-candidates N, --max-work N (run/export); --max-atoms N,\n        --max-fields N, --max-label-bytes N, --max-input-bytes N,\n        --max-validation-work N, --max-package-depth N.\nAll limits are non-negative decimal integers. Defaults follow the native API.\nTrace import/export uses the default TraceLimits policy (see docs/trace-format.md).\nrun/check/replay emit loadable graph text; export emits only trace JSON.\ntrace-check verifies bindings, derived analysis and any scheduler claims.\nreplay inspects history; it does not append or branch.\n--output publishes a new file only; existing files are never overwritten.\nExit codes: 0 valid/normal form; 1 engine/output failure; 2 arguments/input failure;\n            3 resource/step limit; 4 requested rule unavailable; 5 cancelled; 6 recurrence.\nA finished trace may describe a bounded prefix: inspect its report, not status alone.\ncheck establishes validity, not normal form. Limits do not prove divergence.\n";

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
fn lambda_failure(error: parser::lambda::Error) -> Failure {
    let code = match &error {
        parser::lambda::Error::ResourceLimit => 3,
        parser::lambda::Error::Core(d) if d.kind() == ErrorKind::ResourceExhaustion => 3,
        _ => 2,
    };
    Failure::new(code, error.to_string())
}
struct Options {
    run: bool,
    export: bool,
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
        || args == ["replay", "--help"]
        || args == ["trace-check", "--help"]
        || args == ["lambda-compile", "--help"]
        || args == ["lambda-readback", "--help"]
    {
        io::stdout().lock().write_all(b"Lambda adapters: cli lambda-compile SOURCE | cli lambda-readback GRAPH\nBoth accept a file or -; compilation emits ordinary graph source.\nUse cli replay TRACE --at N | cli lambda-readback - for recorded terms.\n\n").map_err(|e| Failure::new(1, e.to_string()))?;
        io::stdout()
            .lock()
            .write_all(format!("{HELP}\nScheduling: --strategy deterministic (default), or --strategy seeded --seed U64.\nOrdering: source identity, then (template identity, host identity) tuples.\nObservers: --observer FILE (observer-v1); --detect-cycles true|false (default false).\nBounds: --max-observation-work N, --max-comparison-work N, --max-history-units N.\nRecurrence is diagnostic, not proof of infinite behaviour; exit 6 CycleDetected.\nShared API cancellation stops at transaction boundaries (exit 5); CLI does not install a signal handler.\nNormal form takes precedence over recurrence and step limits; recurrence precedes step limit.\n").as_bytes())
            .map_err(|e| Failure::new(1, format!("output: {e}")))?;
        return Ok(0);
    }
    if matches!(args[0].as_str(), "replay" | "trace-check") {
        return trace_command(&args);
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
    let mut runner = Runner::new(loaded.into_graph(), options.execution, scheduler, observer)
        .map_err(engine_failure)?;
    if options.export {
        runner
            .enable_recording(Default::default())
            .map_err(engine_failure)?;
    }
    let mut outcome = "Valid";
    let mut code = 0;
    let mut rejected = 0;
    let mut last_diagnostic = None;
    let mut details = String::new();
    if options.run {
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
            Default::default(),
        )
        .map_err(engine_failure)?;
        let bytes = doc
            .to_json(engine::trace::TraceLimits::default().max_bytes)
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
            "--output" if args[0] == "replay" => output = Some(PathBuf::from(&pair[1])),
            _ => return Err(Failure::new(2, "unsupported replay option")),
        }
    }
    let limits = engine::trace::TraceLimits::default();
    let text = read_source(&args[1], limits.max_bytes)?;
    let trace = engine::trace::VerifiedTrace::import(text.as_bytes(), limits).map_err(|e| {
        Failure::new(
            if e.kind() == ErrorKind::ResourceExhaustion {
                3
            } else {
                2
            },
            e.to_string(),
        )
    })?;
    let result = if args[0] == "trace-check" {
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
        run,
        export: args[0] == "export",
        trace_options: Default::default(),
        input: String::new(),
        output: None,
        max_steps: 0,
        rule: None,
        parse: ParseLimits::default(),
        execution: ExecutionLimits::default(),
        scheduler: "deterministic".into(),
        seed: None,
        observer: None,
        run_limits: RunLimits::default(),
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
