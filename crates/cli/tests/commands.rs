use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cli"))
}
fn program(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/programs")
        .join(format!("{name}.graph"))
}
fn stdin(args: &[&str], text: &[u8]) -> Output {
    let mut child = command()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(text).unwrap();
    child.wait_with_output().unwrap()
}
fn outcome(output: &Output, code: i32, label: &str, steps: u64) -> parser::LoadedProgram {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::str::from_utf8(&output.stdout).unwrap();
    assert!(text.contains(&format!("// outcome: {label}\n")), "{text}");
    assert!(
        text.contains(&format!("// committed-steps: {steps}\n")),
        "{text}"
    );
    parser::load(text).unwrap()
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "s2-cli-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn help_documents_all_commands_and_exit_meanings() {
    for args in [
        vec!["--help"],
        vec!["run", "--help"],
        vec!["check", "--help"],
    ] {
        let out = command().args(args).output().unwrap();
        assert!(out.status.success());
        assert!(out.stderr.is_empty());
        let help = String::from_utf8(out.stdout).unwrap();
        for word in [
            "check",
            "--max-steps",
            "normal form",
            "never overwritten",
            "export",
        ] {
            assert!(help.contains(word));
        }
    }
}

#[test]
fn check_validates_active_and_inactive_rules_without_running() {
    let text = std::fs::read_to_string(program("arithmetic")).unwrap();
    for text in [
        text.clone(),
        text.lines()
            .filter(|l| !l.contains("atom az_active ="))
            .collect::<Vec<_>>()
            .join("\n"),
    ] {
        let out = stdin(&["check", "-"], text.as_bytes());
        let loaded = outcome(&out, 0, "Valid", 0);
        assert_eq!(
            loaded.graph().len(),
            parser::load(&text).unwrap().graph().len()
        );
        assert!(out.stderr.is_empty());
    }
}

#[test]
fn arithmetic_run_reports_exact_normal_form_at_the_bound() {
    let out = command()
        .arg("run")
        .arg(program("arithmetic"))
        .args(["--max-steps", "1"])
        .output()
        .unwrap();
    let loaded = outcome(&out, 0, "NormalForm", 1);
    assert_eq!(
        (loaded.graph().len(), loaded.graph().field_count()),
        (27, 35)
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn zero_steps_distinguishes_enabled_input_from_normal_form() {
    let out = command()
        .arg("run")
        .arg(program("arithmetic"))
        .args(["--max-steps", "0"])
        .output()
        .unwrap();
    assert_eq!(outcome(&out, 3, "StepLimit", 0).graph().len(), 31);
    let out = stdin(&["run", "-", "--max-steps", "0"], b"program {}");
    assert!(outcome(&out, 0, "NormalForm", 0).graph().is_empty());
}

#[test]
fn accepted_reflection_runs_bounded_and_is_reproducible() {
    let run = || {
        command()
            .arg("run")
            .arg(program("self-modify"))
            .args(["--max-steps", "3"])
            .output()
            .unwrap()
    };
    let out = run();
    outcome(&out, 3, "StepLimit", 3);
    assert_eq!(out.stdout, run().stdout);
}

#[test]
fn rejected_rule_is_not_confused_with_global_or_partial_normality() {
    let mut base = command();
    base.arg("run")
        .arg(program("mutual-regress"))
        .args(["--max-steps", "1"]);
    let global = base.output().unwrap();
    assert_eq!(outcome(&global, 0, "NormalForm", 0).graph().len(), 81);
    assert!(String::from_utf8_lossy(&global.stderr).contains("Guardedness"));
    let restricted = base.args(["--rule", "p"]).output().unwrap();
    outcome(&restricted, 4, "RequestedRuleUnavailable", 0);
    assert!(String::from_utf8_lossy(&restricted.stderr).contains("not a global normal-form"));
}

#[test]
fn incomplete_search_reports_a_resource_limit_with_no_partial_commit() {
    for args in [["--max-work", "0"], ["--max-candidates", "0"]] {
        let out = command()
            .arg("run")
            .arg(program("arithmetic"))
            .args(["--max-steps", "1"])
            .args(args)
            .output()
            .unwrap();
        assert_eq!(outcome(&out, 3, "ResourceLimit", 0).graph().len(), 31);
        assert!(String::from_utf8_lossy(&out.stderr).contains("ResourceExhaustion"));
    }
}

#[test]
fn invalid_arguments_fail_before_reading_or_output() {
    for args in [
        vec!["run", "missing"],
        vec!["run", "missing", "--max-steps", "-1"],
        vec!["run", "missing", "--max-steps", "18446744073709551616"],
        vec!["run", "missing", "--max-steps", "1", "--max-steps", "2"],
        vec!["check", "missing", "--max-steps", "0"],
        vec!["check", "missing", "--rule", "x"],
        vec!["check", "missing", "--max-work", "1"],
        vec!["demo", "extra"],
        vec!["export", "missing"],
        vec!["check", "missing", "extra"],
        vec!["check", "missing", "--output"],
        vec!["run", "missing", "--max-steps", "+1"],
        vec!["run", "missing", "--max-steps", "1.0"],
    ] {
        let out = command().args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert!(!out.stderr.is_empty());
    }
}

#[test]
fn malformed_invalid_utf8_and_missing_inputs_never_produce_a_graph() {
    for text in [
        b"program { atom x = Nat(missing); }".as_slice(),
        b"program { atom x = RuleHead; }",
        &[255],
    ] {
        let out = stdin(&["check", "-"], text);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    let out = command()
        .args(["check", "/definitely-absent-s2-input.graph"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let out = stdin(&["check", "-"], b"program { atom x = Nat(missing); }");
    assert!(String::from_utf8_lossy(&out.stderr).contains("-:1:"));
}

#[test]
fn loading_limits_fail_closed() {
    for args in [
        ["--max-input-bytes", "2"],
        ["--max-atoms", "0"],
        ["--max-validation-work", "0"],
    ] {
        let out = command()
            .arg("check")
            .arg(program("arithmetic"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(3));
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn report_size_limit_publishes_neither_a_prefix_nor_a_success_outcome() {
    // Input and bare graph fit, but the complete report does not.
    let out = stdin(&["check", "-", "--max-input-bytes", "16"], b"program {}");
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("report byte limit"));
}

#[test]
fn unknown_non_head_and_quoted_rule_names_are_distinguished() {
    for name in ["absent", "input"] {
        let out = command()
            .arg("run")
            .arg(program("staging"))
            .args(["--max-steps", "1", "--rule", name])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    let out = command()
        .arg("run")
        .arg(program("staging"))
        .args(["--max-steps", "1", "--rule", "wh"])
        .output()
        .unwrap();
    outcome(&out, 4, "RequestedRuleUnavailable", 0);
}

#[test]
fn output_file_is_complete_loadable_and_never_overwrites_existing_data() {
    let dir = Directory::new();
    let dest = dir.0.join("final.graph");
    let mut cmd = command();
    cmd.arg("run")
        .arg(program("arithmetic"))
        .args(["--max-steps", "1", "--output"])
        .arg(&dest);
    let out = cmd.output().unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    let bytes = std::fs::read(&dest).unwrap();
    assert_eq!(
        parser::load(std::str::from_utf8(&bytes).unwrap())
            .unwrap()
            .graph()
            .len(),
        27
    );
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn failed_input_and_unwritable_destination_publish_nothing() {
    let dir = Directory::new();
    let dest = dir.0.join("new.graph");
    let out = stdin(
        &["check", "-", "--output", dest.to_str().unwrap()],
        b"invalid",
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(!dest.exists());
    assert!(out.stdout.is_empty());
    let out = command()
        .arg("check")
        .arg(program("arithmetic"))
        .arg("--output")
        .arg(dir.0.join("missing/new.graph"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn output_does_not_follow_or_replace_a_destination_symlink() {
    let dir = Directory::new();
    let dest = dir.0.join("link.graph");
    let target = dir.0.join("target.graph");
    std::fs::write(&target, b"preserve").unwrap();
    std::os::unix::fs::symlink(&target, &dest).unwrap();
    let out = command()
        .arg("check")
        .arg(program("arithmetic"))
        .arg("--output")
        .arg(&dest)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(std::fs::read(target).unwrap(), b"preserve");
    assert!(dest.is_symlink());
}
