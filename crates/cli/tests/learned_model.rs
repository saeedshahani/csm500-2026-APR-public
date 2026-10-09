use serde_json::{json, Value};
mod support;
use std::process::{Command, Stdio};
fn invoke(command: &str, input: &str) -> std::process::Output {
    let child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args([command, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    support::finish_with_input(child, input.as_bytes())
}
fn request() -> Value {
    json!({"states":["a","b"],"start":"a","transitions":[
    {"from":"a","action":0,"target":"b"},{"from":"b","action":1,"target":"a"}],"word":[0,1]})
}
#[test]
fn acquired_model_execution_exports_an_ordinary_verified_trace() {
    let output = invoke("model-run", &request().to_string());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["projected"], json!(["a", "b", "a"]));
    assert_eq!(report["admitted_steps"], 2);
    assert!(invoke("trace-check", &report["trace"].to_string())
        .status
        .success());
    let mut damaged = report["trace"].clone();
    damaged["initial_sha256"] = json!("0".repeat(64));
    assert!(!invoke("trace-check", &damaged.to_string()).status.success());
}
#[test]
fn malformed_and_resource_limited_models_have_distinct_exit_codes() {
    for input in ["null".to_string(), "{".to_string(), "{}".to_string()] {
        assert_eq!(invoke("model-run", &input).status.code(), Some(2));
    }
    let mut r = request();
    r["word"] = json!([1]);
    assert_eq!(invoke("model-run", &r.to_string()).status.code(), Some(2));
    r = request();
    r["native_max_work"] = json!(0);
    assert_eq!(invoke("model-run", &r.to_string()).status.code(), Some(3));
    for value in [
        json!(-1),
        json!(1.5),
        json!("0"),
        json!(u64::MAX),
        Value::Null,
    ] {
        r["native_max_work"] = value;
        assert_eq!(invoke("model-run", &r.to_string()).status.code(), Some(2));
    }
}
#[test]
fn empty_words_and_maximum_supported_words_are_checked() {
    let mut r = json!({"states":["x"],"start":"x","transitions":[{"from":"x","action":0,"target":"x"}],"word":[]});
    for length in [0, 16] {
        r["word"] = json!(vec![0; length]);
        let out = invoke("model-run", &r.to_string());
        assert!(out.status.success());
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["admitted_steps"], length);
    }
    r["word"] = json!(vec![0; 17]);
    assert_eq!(invoke("model-run", &r.to_string()).status.code(), Some(2));
}

#[test]
fn labels_are_opaque_and_table_order_does_not_choose_the_path() {
    let names = ["at:0:7", "λ: one", "\"two\"\n"];
    let word = [0usize, 1, 1, 0, 1, 0, 0, 1];
    let mut transitions = Vec::new();
    for (state, name) in names.iter().enumerate() {
        for action in 0..2 {
            transitions.push(json!({"from":name,"action":action,
                "target":names[(state + action + 1) % names.len()]}));
        }
    }
    let mut state = 0;
    let mut expected = vec![names[state]];
    for action in word {
        state = (state + action + 1) % names.len();
        expected.push(names[state]);
    }
    for reverse in [false, true] {
        if reverse {
            transitions.reverse();
        }
        let input = json!({"states":names,"start":names[0],
            "transitions":transitions,"word":word});
        let output = invoke("model-run", &input.to_string());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["projected"], json!(expected));
    }
}

#[test]
fn valid_input_bounds_do_not_bypass_native_resource_limits() {
    let states: Vec<_> = (0..32).map(|s| s.to_string()).collect();
    let transitions: Vec<_> = (0..32)
        .flat_map(|s| {
            (0..2).map(
                move |a| json!({"from":s.to_string(),"action":a,"target":((s+a+1)%32).to_string()}),
            )
        })
        .collect();
    let input = json!({"states":states,"start":"0","transitions":transitions,"word":vec![0;16]});
    let output = invoke("model-run", &input.to_string());
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ResourceExhaustion"));
}
