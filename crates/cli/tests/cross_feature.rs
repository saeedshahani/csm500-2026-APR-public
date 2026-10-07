//! Every application publishes the same checked-history contract.
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn invoke(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
fn success(args: &[&str], input: &[u8]) -> Vec<u8> {
    let out = invoke(args, input);
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
fn report(command: &str, input: &[u8]) -> Value {
    serde_json::from_slice(&success(&[command, "-"], input)).unwrap()
}

#[test]
fn adopted_applications_share_replay_storage_and_provenance() {
    let organization = report(
        "organization-run",
        include_bytes!("../../../examples/organization/renewal.json"),
    );
    let numerical = report(
        "numerical-run",
        include_bytes!("../../../examples/numerical/workflow.json"),
    );
    let actions = report(
        "action-learn",
        include_bytes!("../../../examples/action-learning/doors.json"),
    );
    let versioned = success(
        &["version-compile", "-"],
        include_bytes!("../../../examples/reflection/versioned.json"),
    );
    let versioned: Value =
        serde_json::from_slice(&success(&["export", "-", "--max-steps", "8"], &versioned)).unwrap();
    let traces = [
        &organization["native"]["execution"]["trace"],
        &numerical["publications"][0]["trace"],
        &actions["execution"]["trace"],
        &versioned,
    ];
    for trace in traces {
        let bytes = serde_json::to_vec(trace).unwrap();
        let events = trace["events"].as_array().unwrap().len();
        assert!(events > 0);
        success(&["trace-check", "-"], &bytes);
        success(&["replay", "-"], &bytes);
        let default = report("trace-cost", &bytes);
        let explicit = success(&["trace-cost", "-", "--at", &events.to_string()], &bytes);
        assert_eq!(default, serde_json::from_slice::<Value>(&explicit).unwrap());
        assert_eq!(
            default["source_trace_sha256"],
            engine::trace::sha256(&bytes)
        );
        let provenance = report("trace-provenance", &bytes);
        assert_eq!(
            provenance["analysis"]["observations"]
                .as_array()
                .unwrap()
                .len(),
            events
        );
        assert_eq!(provenance["analysis"]["global_necessity"], "unknown");
        for args in [
            vec!["trace-cost", "-", "--max-snapshot-bytes", "0"],
            vec!["trace-provenance", "-", "--max-work", "0"],
        ] {
            let out = invoke(&args, &bytes);
            assert_eq!(out.status.code(), Some(3));
            assert!(out.stdout.is_empty());
        }
        let mut forged = trace.clone();
        forged["initial_sha256"] = "0".repeat(64).into();
        for command in ["trace-check", "trace-cost", "trace-provenance"] {
            let out = invoke(&[command, "-"], &serde_json::to_vec(&forged).unwrap());
            assert_eq!(out.status.code(), Some(2));
            assert!(out.stdout.is_empty());
        }
    }
}
