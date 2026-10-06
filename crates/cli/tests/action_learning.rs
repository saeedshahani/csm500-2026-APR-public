use std::io::Write;
use std::process::{Command, Stdio};
fn invoke(args: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn learning_compilation_execution_and_portable_replay() {
    let input = include_str!("../../../examples/action-learning/doors.json");
    let out = invoke(&["action-learn", "-"], input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["execution"]["admitted_steps"], 2);
    let trace = serde_json::to_string(&report["execution"]["trace"]).unwrap();
    assert!(invoke(&["trace-check", "-"], &trace).status.success());
    let compiled = invoke(&["action-compile", "-"], input);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let source = String::from_utf8(compiled.stdout).unwrap();
    let exported = invoke(&["export", "-", "--max-steps", "8"], &source);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let checked = invoke(
        &["trace-check", "-"],
        &String::from_utf8(exported.stdout).unwrap(),
    );
    assert!(checked.status.success());
    assert!(source.contains("action.fact"));
}
#[test]
fn strict_requests_and_resource_errors_never_publish_success() {
    let input = include_str!("../../../examples/action-learning/doors.json");
    for changed in [
        input.replacen("\"types\": [\"door\"]", "\"types\": [\"door\",\"door\"]", 1),
        input.replacen("\"actions\": {", "\"actions\": {\"open\": [\"door\"],", 1),
        input.replacen("\"objects\": {", "\"objects\": {\"training\": \"door\",", 1),
        input.replacen("\"repair\":", "\"extra\": 0, \"repair\":", 1),
        input.replacen("\"value\": false", "\"value\": 0", 1),
        input.replacen("\"value\": false", "\"value\": null", 1),
    ] {
        let out = invoke(&["action-learn", "-"], &changed);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    let mut r: serde_json::Value = serde_json::from_str(input).unwrap();
    r["observations"][0]
        .as_object_mut()
        .unwrap()
        .remove("after");
    let out = invoke(&["action-learn", "-"], &r.to_string());
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let mut r: serde_json::Value = serde_json::from_str(input).unwrap();
    r["run"]["native_max_work"] = 0.into();
    let out = invoke(&["action-learn", "-"], &r.to_string());
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
    let help = invoke(&["action-learn", "--help"], "");
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)
        .unwrap()
        .contains("Action learning:"));
}
