use std::{path::Path, process::Command};

#[test]
fn agreement_and_explicit_retry_reconstruction_are_exposed_without_fabricating_completion() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_cli"))
        .current_dir(&root)
        .args([
            "stability-check",
            "examples/practical/repair-policy.graph",
            "examples/practical/repair-policy.contract.json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["agreement"], "agreed");
    for (label, code) in [("pending", 0), ("done", 2)] {
        let out = Command::new(env!("CARGO_BIN_EXE_cli"))
            .current_dir(&root)
            .args([
                "repair-statuses",
                "examples/practical/repair-policy.graph",
                "examples/practical/repair-policy.contract.json",
                label,
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(code), "{out:?}");
        if code == 0 {
            let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(r["history_continuation"], false);
            assert_eq!(r["receipt"]["reset_label"], "pending");
            assert_eq!(r["receipt"]["created"], serde_json::json!([]));
        }
    }
}
fn run(extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cli"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args([
            "stability-check",
            "examples/practical/revise-guard.graph",
            "examples/practical/revise-guard.contract.json",
        ])
        .args(extra)
        .output()
        .unwrap()
}
#[test]
fn complete_analysis_and_inconclusive_have_distinct_machine_readable_outcomes() {
    for (args, code, verdict) in [
        (vec![], 0, "certified"),
        (vec!["--max-states", "1"], 3, "inconclusive"),
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(code), "{:?}", out);
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["verdict"], verdict);
        if code == 0 {
            assert_eq!(value["recovery_bound"], 3);
        }
    }
}
#[test]
fn malformed_bounds_are_not_treated_as_research_results() {
    for args in [
        vec!["--max-states"],
        vec!["--unknown", "1"],
        vec!["--max-states", "-1"],
        vec!["--max-states", "1", "--max-states", "2"],
    ] {
        assert_eq!(run(&args).status.code(), Some(2));
    }
}

#[test]
fn a_refutation_is_not_a_parse_error_or_successful_certificate() {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args([
            "stability-check",
            "examples/practical/revise-output.graph",
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let contract = include_str!("../../../examples/practical/revise-output.contract.json").replace(
        "\"legitimate_labels\":[\"done\",\"audited\"]",
        "\"legitimate_labels\":[\"done\"]",
    );
    assert_ne!(
        contract,
        include_str!("../../../examples/practical/revise-output.contract.json")
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(contract.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(7));
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["verdict"], "refuted");
}
