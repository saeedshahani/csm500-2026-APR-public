use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};
fn run(args: &[&str], source: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Err(error) = child.stdin.take().unwrap().write_all(source.as_bytes()) {
        assert_eq!(error.kind(), ErrorKind::BrokenPipe);
    }
    child.wait_with_output().unwrap()
}
const LOOP:&str="program { rule a { atom az=Nat(); rank ar=az; mode am=ModeObject; revision av; active aa; } rule b { atom bz=Nat(); rank br=bz; mode bm=ModeObject; revision bv; active ba; } }";
#[test]
fn deterministic_and_seeded_results_repeat_across_processes() {
    for extra in [
        vec![],
        vec!["--strategy", "seeded", "--seed", "18446744073709551615"],
    ] {
        let mut args = vec!["run", "-", "--max-steps", "4"];
        args.extend(extra);
        let a = run(&args, LOOP);
        let b = run(&args, LOOP);
        assert_eq!(
            a.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&a.stderr)
        );
        assert_eq!(a.stdout, b.stdout);
        assert_eq!(a.stderr, b.stderr);
        assert!(
            String::from_utf8_lossy(&a.stdout).contains("source-template-host-lexicographic-v1")
        );
    }
}
#[test]
fn cycle_and_budget_reports_remain_loadable() {
    for (extra, code, outcome) in [
        (vec!["--detect-cycles", "true"], 6, "CycleDetected"),
        (
            vec!["--detect-cycles", "true", "--max-comparison-work", "0"],
            3,
            "ResourceLimit",
        ),
        (
            vec!["--detect-cycles", "true", "--max-history-units", "0"],
            3,
            "ResourceLimit",
        ),
    ] {
        let mut args = vec!["run", "-", "--max-steps", "1"];
        args.extend(extra);
        let result = run(&args, LOOP);
        assert_eq!(
            result.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let source = String::from_utf8(result.stdout).unwrap();
        assert!(source.contains(outcome));
        parser::load(&source).unwrap();
        if code == 6 {
            assert!(source.contains("not a proof"));
            assert!(source.contains("0 -> 1"));
        }
    }
}
#[test]
fn invalid_scheduler_and_seed_options_are_input_errors() {
    for extra in [
        vec!["--strategy", "seeded"],
        vec!["--seed", "2"],
        vec!["--strategy", "random"],
        vec!["--strategy", "seeded", "--seed", "18446744073709551616"],
        vec!["--strategy", "seeded", "--seed", "-1"],
        vec!["--detect-cycles", "yes"],
    ] {
        let mut args = vec!["run", "-", "--max-steps", "0"];
        args.extend(extra);
        let result = run(&args, LOOP);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    assert_eq!(
        run(&["check", "-", "--strategy", "seeded", "--seed", "1"], LOOP)
            .status
            .code(),
        Some(2)
    );
}
