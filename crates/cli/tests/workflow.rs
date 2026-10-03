use std::{
    io::Write,
    process::{Command, Stdio},
};
fn invoke(command: &str, request: &serde_json::Value) -> std::process::Output {
    let mut p = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args([command, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    p.stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    p.wait_with_output().unwrap()
}
#[test]
fn workflow_cli_certifies_runs_and_exports_replayable_history() {
    for family in ["routing", "branching"] {
        let example = Command::new(env!("CARGO_BIN_EXE_cli"))
            .args(["workflow-example", family, "2"])
            .output()
            .unwrap();
        assert!(example.status.success(), "{example:?}");
        let mut request: serde_json::Value = serde_json::from_slice(&example.stdout).unwrap();
        let checked = invoke("workflow-check", &request);
        assert!(checked.status.success());
        request["max_steps"] = 0.into();
        let zero = invoke("workflow-run", &request);
        assert_eq!(zero.status.code(), Some(3));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&zero.stdout).unwrap()["outcome"],
            "StepLimit"
        );
        request["max_steps"] = 100.into();
        let run = invoke("workflow-run", &request);
        assert!(run.status.success(), "{run:?}");
        let value: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
        assert_eq!(value["outcome"], "Completed");
        let trace = invoke("trace-check", &value["trace"]);
        assert!(trace.status.success(), "{trace:?}");
        request["contract"]["weights"]["received"] = "0".into();
        let bad = invoke("workflow-check", &request);
        assert_eq!(bad.status.code(), Some(2));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bad.stdout).unwrap()["refuted"],
            false
        );
    }
}
