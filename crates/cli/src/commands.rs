use engine::{Execution, ExecutionLimits};
use graph_core::{Diagnostic, ErrorKind, Kind};
use parser::{load_with_limits, print_with_limits, source_position, ParseLimits};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const HELP: &str = "usage: cli [demo]\n       cli check INPUT [limits] [--output PATH]\n       cli run INPUT --max-steps N [--rule NAME] [limits] [--output PATH]\n\nINPUT is a UTF-8 program file, or - for standard input.\nrun selects the first fully enabled candidate in identity-based order.\n--rule restricts each step to the named initial rule (not a global normal-form query).\n--max-steps is required for every run; zero checks without committing.\nLimits: --max-candidates N, --max-work N (run only); --max-atoms N,\n        --max-fields N, --max-label-bytes N, --max-input-bytes N,\n        --max-validation-work N, --max-package-depth N.\nAll limits are non-negative decimal integers. Defaults follow the native API.\nOutput: loadable graph text with outcome/step/limit comments; diagnostics on stderr.\n--output publishes a new file only; existing files are never overwritten.\nExit codes: 0 valid/normal form; 1 engine/output failure; 2 arguments/input failure;\n            3 resource/step limit; 4 requested rule unavailable.\ncheck establishes validity, not normal form. Limits do not prove divergence.\nJSON trace export is not implemented; it belongs to the trace-export work.\n";

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
struct Options {
    run: bool,
    input: String,
    output: Option<PathBuf>,
    max_steps: u64,
    rule: Option<String>,
    parse: ParseLimits,
    execution: ExecutionLimits,
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
    {
        io::stdout()
            .lock()
            .write_all(HELP.as_bytes())
            .map_err(|e| Failure::new(1, format!("output: {e}")))?;
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
    let mut execution =
        Execution::new(loaded.into_graph(), options.execution).map_err(engine_failure)?;
    let mut outcome = "Valid";
    let mut code = 0;
    let mut rejected = 0;
    let mut last_diagnostic = None;
    if options.run {
        loop {
            if let Some(head) = requested {
                let view = execution.view().map_err(engine_failure)?;
                if view.rule(head).is_none() {
                    outcome = "RequestedRuleUnavailable";
                    code = 4;
                    break;
                }
            }
            let query = match requested {
                Some(head) => execution.enabled_for(head),
                None => execution.enabled(),
            };
            let enabled = match query {
                Ok(enabled) => enabled,
                Err(error) if error.kind() == ErrorKind::ResourceExhaustion => {
                    outcome = "ResourceLimit";
                    code = 3;
                    last_diagnostic = Some(error);
                    break;
                }
                Err(error) => return Err(engine_failure(error)),
            };
            rejected = enabled.rejected().len();
            if enabled.steps().is_empty() {
                if requested.is_some() {
                    outcome = "RequestedRuleUnavailable";
                    code = 4;
                } else {
                    outcome = "NormalForm";
                }
                last_diagnostic = enabled.rejected().first().map(|r| r.diagnostic.clone());
                break;
            }
            if execution.committed_steps() >= options.max_steps {
                outcome = "StepLimit";
                code = 3;
                break;
            }
            let first = enabled.into_steps().into_iter().next().unwrap();
            execution.commit(first).map_err(engine_failure)?;
        }
    }
    let graph = print_with_limits(execution.graph(), &options.parse).map_err(engine_failure)?;
    let policy = if !options.run {
        "none"
    } else if requested.is_some() {
        "requested-identity-first-v1"
    } else {
        "identity-first-v1"
    };
    let mut output=format!("// outcome: {outcome}\n// committed-steps: {}\n// selection-policy: {policy}\n// rejected-in-last-complete-query: {rejected}\n",execution.committed_steps());
    output+=&format!("// max-steps: {}\n// limits: candidates={} work={} atoms={} fields={} label-bytes={} input-bytes={} validation-work={} package-depth={}\n",options.max_steps,options.execution.search.max_candidates,options.execution.search.max_work,options.parse.graph.max_atoms,options.parse.graph.max_fields,options.parse.graph.max_label_bytes,options.parse.graph.max_input_bytes,options.parse.graph.max_validation_work,options.parse.graph.max_package_depth);
    if let Some(rule) = &options.rule {
        output += &format!("// requested-initial-rule: {rule}\n");
    }
    output += &graph;
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

fn options(args: &[String]) -> Result<Options> {
    let run = match args[0].as_str() {
        "run" => true,
        "check" => false,
        _ => return Err(Failure::new(2, HELP)),
    };
    let mut result = Options {
        run,
        input: String::new(),
        output: None,
        max_steps: 0,
        rule: None,
        parse: ParseLimits::default(),
        execution: ExecutionLimits::default(),
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
                "--output" => result.output = Some(PathBuf::from(value)),
                "--rule" if run => result.rule = Some(value.clone()),
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
