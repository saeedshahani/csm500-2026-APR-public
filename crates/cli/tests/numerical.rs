use serde_json::{json, Value};
mod support;
use std::process::{Command, Stdio};
fn invoke(command: &str, input: &str, options: &[&str]) -> std::process::Output {
    let child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args([command, "-"])
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    support::finish_with_input(child, input.as_bytes())
}
#[test]
fn workflow_runs_and_export_checks_without_python() {
    let out = invoke(
        "numerical-run",
        include_str!("../../../examples/numerical/workflow.json"),
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let p = &report["publications"][0];
    assert_eq!(p["packet"]["status"], "certified");
    let check = invoke("numerical-check", &p["packet"].to_string(), &[]);
    assert!(check.status.success());
    let checked: Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(checked["authenticated"], false);
    let trace = p["trace"].to_string();
    assert!(invoke("trace-check", &trace, &[]).status.success());
    assert!(invoke("replay", &trace, &[]).status.success());
}
#[test]
fn invalid_schema_and_resource_requests_publish_nothing() {
    let original: Value =
        serde_json::from_str(include_str!("../../../examples/numerical/workflow.json")).unwrap();
    for (field, value) in [
        ("bits", json!(65)),
        ("steps", json!(4097)),
        ("mode", json!("unknown")),
        ("boundary", json!(0.5)),
        ("unknown", json!(1)),
    ] {
        let mut r = original.clone();
        r[field] = value;
        let out = invoke("numerical-run", &r.to_string(), &[]);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    let out = invoke("numerical-check", "{}", &[]);
    assert_eq!(out.status.code(), Some(2));
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let before = std::fs::read(&path).unwrap();
    let out = invoke(
        "numerical-run",
        &original.to_string(),
        &["--output", path.to_str().unwrap()],
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(before, std::fs::read(path).unwrap());
}
#[test]
fn help_describes_scope() {
    let out = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["numerical-run", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("Exact Rust arithmetic"));
}
