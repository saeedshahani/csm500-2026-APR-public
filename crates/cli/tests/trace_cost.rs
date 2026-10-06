use std::io::Write;
use std::process::{Command, Stdio};
fn invoke(args: &[&str], input: &[u8]) -> std::process::Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(input).unwrap();
    c.wait_with_output().unwrap()
}
fn trace() -> Vec<u8> {
    let o = invoke(
        &["export", "-", "--max-steps", "0"],
        b"program { atom a = Constructor(\"value\"); }",
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    o.stdout
}
#[test]
fn snapshot_report_uses_checked_history_and_default_endpoint() {
    let input = trace();
    let a = invoke(&["trace-cost", "-"], &input);
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    let b = invoke(&["trace-cost", "-", "--at", "0"], &input);
    assert_eq!(a.stdout, b.stdout);
    let report: serde_json::Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(report["analysis"]["schema"], "snapshot-cost-v1");
    assert_eq!(report["analysis"]["atoms"], "1");
    assert_eq!(report["source_trace_sha256"], engine::trace::sha256(&input));
    let bytes = report["analysis"]["total_bytes"]
        .as_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert!(invoke(
        &[
            "trace-cost",
            "-",
            "--max-snapshot-bytes",
            &bytes.to_string()
        ],
        &input
    )
    .status
    .success());
    let low = invoke(
        &[
            "trace-cost",
            "-",
            "--max-snapshot-bytes",
            &(bytes - 1).to_string(),
        ],
        &input,
    );
    assert_eq!(low.status.code(), Some(3));
    assert!(low.stdout.is_empty());
    assert!(invoke(&["trace-cost", "--help"], b"").status.success());
}
#[test]
fn malformed_tampered_or_out_of_range_input_never_yields_a_report() {
    let input = trace();
    for args in [
        vec!["trace-cost", "-", "--at", "1"],
        vec!["trace-cost", "-", "--at", "false"],
        vec!["trace-cost", "-", "--at", "0", "--at", "0"],
        vec!["trace-cost", "-", "--max-snapshot-bytes", "-1"],
    ] {
        let out = invoke(&args, &input);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    let original = String::from_utf8(input).unwrap();
    for changed in [
        "{}".into(),
        original.replacen("\"value\"", "\"tampered\"", 1),
        original.replacen('{', "{\"schema\":\"duplicate\",", 1),
    ] {
        let out = invoke(&["trace-cost", "-"], changed.as_bytes());
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn publication_refuses_existing_files_and_leaves_them_unchanged() {
    let dir = std::env::temp_dir().join(format!(
        "snapshot-cost-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("report.json");
    let args = ["trace-cost", "-", "--output", path.to_str().unwrap()];
    let input = trace();
    let out = invoke(&args, &input);
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    let before = std::fs::read(&path).unwrap();
    let again = invoke(&args, &input);
    assert!(!again.status.success());
    assert!(again.stdout.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&dir).unwrap();
}
