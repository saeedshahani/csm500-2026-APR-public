mod support;
use std::process::{Command, Stdio};
fn invoke(args: &[&str], input: &[u8]) -> std::process::Output {
    let c = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    support::finish_with_input(c, input)
}
fn trace() -> Vec<u8> {
    let out = invoke(
        &["export", "-", "--max-steps", "3"],
        include_bytes!("../../../examples/behaviour/oscillating.graph"),
    );
    assert!(
        !out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
#[test]
fn native_command_supports_occurrence_class_and_empty_selection() {
    let input = trace();
    for (flags, labels) in [
        (vec![], 3),
        (vec!["--select", "0,2"], 2),
        (vec!["--select", ""], 0),
        (vec!["--classes", "0=phase,2=phase"], 1),
        (vec!["--classes", "0=phase,0=update,2=phase"], 2),
    ] {
        let mut args = vec!["trace-provenance", "-"];
        args.extend(flags);
        let out = invoke(&args, &input);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["source_trace_sha256"], engine::trace::sha256(&input));
        assert_eq!(v["analysis"]["labels"].as_array().unwrap().len(), labels);
        assert_eq!(v["analysis"]["observations"].as_array().unwrap().len(), 3);
        assert_eq!(v["analysis"]["global_necessity"], "unknown");
        assert_eq!(v["analysis"]["relation"], "selected_birth_read_ancestry_v1");
    }
    let help = invoke(&["trace-provenance", "--help"], b"");
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)
        .unwrap()
        .contains("--classes"));
}
#[test]
fn input_validation_and_resource_errors_produce_no_report() {
    let input = trace();
    for (flags, code) in [
        (vec!["--select", "99"], 1),
        (vec!["--select", "0,0"], 2),
        (vec!["--select", "0", "--classes", "0=x"], 2),
        (vec!["--classes", "0=x,0=x"], 2),
        (vec!["--classes", "false=x"], 2),
        (vec!["--classes", "0="], 2),
        (vec!["--select", "-1"], 2),
        (vec!["--max-units", "no"], 2),
    ] {
        let mut args = vec!["trace-provenance", "-"];
        args.extend(flags);
        let out = invoke(&args, &input);
        assert_eq!(out.status.code(), Some(code), "{out:?}");
        assert!(out.stdout.is_empty());
    }
    for option in ["--max-work", "--max-units"] {
        let out = invoke(&["trace-provenance", "-", option, "0"], &input);
        assert_eq!(out.status.code(), Some(3));
        assert!(out.stdout.is_empty());
    }
    let original = String::from_utf8(input).unwrap();
    for input in [
        "{}".into(),
        original.replacen('{', "{\"schema\":\"duplicate\",", 1),
        original.replacen("\"pending\"", "\"forged\"", 1),
    ] {
        let out = invoke(&["trace-provenance", "-"], input.as_bytes());
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
}
#[test]
fn report_publication_never_overwrites_and_report_is_not_a_trace() {
    let dir = std::env::temp_dir().join(format!(
        "selected-provenance-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("report.json");
    let args = ["trace-provenance", "-", "--output", path.to_str().unwrap()];
    let input = trace();
    assert!(invoke(&args, &input).status.success());
    let before = std::fs::read(&path).unwrap();
    assert!(!invoke(&args, &input).status.success());
    assert_eq!(before, std::fs::read(&path).unwrap());
    assert!(!invoke(&["trace-provenance", "-"], &before).status.success());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
