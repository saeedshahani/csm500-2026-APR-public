mod support;
use std::process::{Command, Stdio};

fn invoke(args: &[&str], input: &str) -> std::process::Output {
    let child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    support::finish_with_input(child, input.as_bytes())
}
#[test]
fn compile_execute_readback_and_replay() {
    let request = include_str!("../../../examples/reflection/versioned.json");
    let compiled = invoke(&["version-compile", "-"], request);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let source = String::from_utf8(compiled.stdout).unwrap();
    assert_eq!(
        source.trim(),
        include_str!("../../../examples/reflection/versioned.graph").trim()
    );
    let exported = invoke(&["export", "-", "--max-steps", "8"], &source);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let trace = String::from_utf8(exported.stdout).unwrap();
    assert!(invoke(&["trace-check", "-"], &trace).status.success());
    let replay = invoke(&["replay", "-"], &trace);
    assert!(replay.status.success());
    let result = invoke(
        &["version-readback", "-"],
        &String::from_utf8(replay.stdout).unwrap(),
    );
    assert!(result.status.success());
    let view: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(view["active"]["implementation"], "return");
    assert_eq!(view["publication_pending"], false);
    assert_eq!(view["calls"].as_array().unwrap().len(), 2);
    for call in view["calls"].as_array().unwrap() {
        assert_eq!(call["status"], "done");
        assert_eq!(call["payload"], "sample-item");
    }
}
#[test]
fn malformed_request_publishes_nothing() {
    for input in [
        "{}",
        r#"{"previous":"return","replacement":"return","budget":0,"payload":"x"}"#,
        r#"{"previous":"return","replacement":"unknown","budget":1,"payload":"x"}"#,
        r#"{"previous":"return","replacement":"return","budget":1,"payload":"x","extra":true}"#,
    ] {
        let out = invoke(&["version-compile", "-"], input);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    let out = invoke(&["version-readback", "-"], "program {}");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}
