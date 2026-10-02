use std::{path::Path, process::Command};
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cli"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn contracts_have_distinct_exit_codes_and_invalid_inputs_are_not_refutations() {
    for (path, mode, tail, code) in [
        (
            "examples/programs/restoration.graph",
            "edit-burst",
            vec![],
            7,
        ),
        (
            "examples/programs/restoration.graph",
            "meta-invocations",
            vec![],
            7,
        ),
        (
            "examples/programs/restoration.graph",
            "meta-burst",
            vec![],
            7,
        ),
        (
            "examples/practical/credit-spawn.graph",
            "finite-edits",
            vec![],
            0,
        ),
        (
            "examples/programs/restoration.graph",
            "finite-edits",
            vec![],
            7,
        ),
        (
            "examples/practical/credit-spawn.graph",
            "finite-edits",
            vec!["--max-states", "1"],
            3,
        ),
        (
            "examples/practical/credit-spawn.graph",
            "productive",
            vec![],
            2,
        ),
        ("examples/practical/credit-spawn.graph", "wrong", vec![], 2),
        (
            "examples/practical/credit-spawn.graph",
            "finite-edits",
            vec!["--max-states"],
            2,
        ),
        (
            "examples/practical/credit-spawn.graph",
            "finite-edits",
            vec!["--max-states", "-1"],
            2,
        ),
        (
            "examples/practical/credit-spawn.graph",
            "finite-edits",
            vec!["--max-states", "1", "--max-states", "2"],
            2,
        ),
    ] {
        let mut args = vec!["meta-check", path, mode, "none"];
        args.extend(tail);
        let out = run(&args);
        assert_eq!(out.status.code(), Some(code), "{out:?}");
        if code != 2 {
            let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(
                r["verdict"],
                match code {
                    0 => "certified",
                    7 => "refuted",
                    _ => "inconclusive",
                }
            );
        }
    }
    assert!(run(&["meta-check", "--help"]).status.success());
}
#[test]
fn productive_mode_checks_a_declared_payload_contract() {
    let out = run(&[
        "meta-check",
        "examples/practical/revise-guard.graph",
        "productive",
        "examples/practical/revise-guard.contract.json",
    ]);
    assert!(out.status.success(), "{out:?}");
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["analysis"]["productive_modulation"], "certified");
}

#[test]
fn inert_analysis_is_explicit_versioned_and_fail_closed() {
    let out = run(&[
        "meta-abstract",
        "examples/practical/inert-log.graph",
        "finite-edits",
        "none",
        "Log",
    ]);
    assert!(out.status.success(), "{out:?}");
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["analysis"]["schema"], "meta-activity-analysis-v2");
    assert_eq!(r["analysis"]["total_edits"]["initial_bound"], 2);
    assert_eq!(r["analysis"]["abstraction"]["label"], "Log");
    for (args, code) in [
        (vec!["meta-abstract"], 2),
        (
            vec![
                "meta-abstract",
                "examples/practical/inert-log.graph",
                "finite-edits",
                "none",
            ],
            2,
        ),
        (
            vec![
                "meta-abstract",
                "examples/practical/inert-log.graph",
                "finite-edits",
                "none",
                "MetaCredit",
            ],
            2,
        ),
        (
            vec![
                "meta-abstract",
                "examples/practical/inert-log.graph",
                "finite-edits",
                "none",
                "Log",
                "--max-states",
                "0",
            ],
            3,
        ),
        (
            vec![
                "meta-check",
                "examples/practical/inert-log.graph",
                "finite-edits",
                "none",
                "--max-states",
                "4",
            ],
            3,
        ),
    ] {
        assert_eq!(run(&args).status.code(), Some(code), "{args:?}");
    }
    assert!(run(&["meta-abstract", "--help"]).status.success());
}
