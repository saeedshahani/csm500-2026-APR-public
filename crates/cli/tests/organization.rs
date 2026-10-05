use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(input: &str, options: &[&str]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["organization-run", "-"])
        .args(options)
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
fn rust_only_command_publishes_a_checked_portable_history() {
    let result = invoke(
        include_str!("../../../examples/organization/renewal.json"),
        &[],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["schema"], "organization-run-v1");
    assert_eq!(report["native"]["environmental_agreement"], true);
    let trace = serde_json::to_vec(&report["native"]["execution"]["trace"]).unwrap();
    engine::trace::VerifiedTrace::import(&trace, Default::default()).unwrap();
}

#[test]
fn invalid_requests_publish_nothing_and_do_not_overwrite_existing_files() {
    for input in [
        "null".to_string(),
        "{bad".into(),
        json!({"config":{"seed":0}}).to_string(),
        json!({"word":vec![0;17]}).to_string(),
        json!({"stages":[{"epochs":8193}]}).to_string(),
    ] {
        let result = invoke(&input, &[]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    // Existing source is a stable target; publication must leave it byte-identical.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let before = std::fs::read(&path).unwrap();
    let result = invoke("{\"stages\":[]}", &["--output", path.to_str().unwrap()]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn help_describes_canonical_rust_entry_point_and_scope() {
    let result = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["organization-run", "--help"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("8192 epochs/64 stages"));
}
