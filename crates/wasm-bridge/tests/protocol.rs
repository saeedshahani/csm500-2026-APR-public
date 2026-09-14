use serde_json::{json, Value};
use wasm_bridge::Workbench;
fn request(w: &mut Workbench, v: Value) -> Value {
    serde_json::to_value(w.dispatch(&v.to_string())).unwrap()
}
fn load(w: &mut Workbench) -> Value {
    request(
        w,
        json!({"command":"load","source":include_str!("../../../examples/behaviour/oscillating.graph"),"config":{"max_steps":"6","seed":"42","detect_cycles":true}}),
    )
}
#[test]
fn historical_inspection_and_analysis_preserve_the_run() {
    let mut w = Workbench::default();
    let initial = request(
        &mut w,
        json!({"command":"load","source":include_str!("../../../examples/independent/shared-read.graph")}),
    );
    assert_eq!(initial["state"]["names"]["0"], "settle");
    assert!(!initial["state"]["rules"][0]["lhs"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(initial["state"]["previous"].is_null());
    let result = request(&mut w, json!({"command":"analyse"}));
    assert_eq!(result["ok"], true);
    assert_eq!(
        result["analysis"]["local"]["alternatives"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(result["analysis"]["local"]["pairs"][0]["commutes"], true);
    assert_eq!(request(&mut w, json!({"command":"state"})), initial);
    let exhausted = request(&mut w, json!({"command":"analyse","max_work":"0"}));
    assert_eq!(exhausted["error"]["kind"], "ResourceExhaustion");
    assert_eq!(request(&mut w, json!({"command":"state"})), initial);
    let step = request(&mut w, json!({"command":"advance","batch":"1"}));
    assert_eq!(step["state"]["previous"], initial["state"]["graph"]);
    assert_eq!(step["state"]["names"], initial["state"]["names"]);
    let result = request(&mut w, json!({"command":"analyse"}));
    assert_eq!(result["analysis"]["local"]["alternatives"][0]["state"], "1");
    assert_eq!(request(&mut w, json!({"command":"state"})), step);
    let related = request(&mut w, json!({"command":"analyse","event":"0"}));
    assert_eq!(related["analysis"]["ancestors"], json!([]));
    assert_eq!(request(&mut w, json!({"command":"state"})), step);
    assert_eq!(
        request(
            &mut w,
            json!({"command":"analyse","event":"18446744073709551615"})
        )["ok"],
        false
    );
}
#[test]
fn causal_queries_return_witnessed_order_and_no_execution_changes() {
    let mut w = Workbench::default();
    load(&mut w);
    request(&mut w, json!({"command":"advance","batch":"2"}));
    let before = request(&mut w, json!({"command":"state"}));
    assert_eq!(
        request(&mut w, json!({"command":"analyse","event":"0"}))["analysis"]["descendants"],
        json!(["1"])
    );
    assert_eq!(
        request(&mut w, json!({"command":"analyse","event":"1"}))["analysis"]["ancestors"],
        json!(["0"])
    );
    request(&mut w, json!({"command":"jump","state":"0"}));
    assert_eq!(
        request(&mut w, json!({"command":"analyse"}))["analysis"]["local"]["alternatives"][0]
            ["state"],
        "0"
    );
    request(&mut w, json!({"command":"jump","state":"2"}));
    assert_eq!(request(&mut w, json!({"command":"state"})), before);
}
#[test]
fn handle_lifecycle_and_atomic_replacement() {
    let mut a = Workbench::default();
    let mut b = Workbench::default();
    assert_eq!(request(&mut a, json!({"command":"state"}))["ok"], false);
    assert_eq!(load(&mut a)["ok"], true);
    let old = request(&mut a, json!({"command":"state"}));
    for bad in [
        json!({"command":"load","source":"bad"}),
        json!({"command":"import","trace":"bad"}),
        json!({"command":"advance","batch":"18446744073709551615"}),
        json!({"command":"jump","state":"18446744073709551615"}),
    ] {
        assert_eq!(request(&mut a, bad)["ok"], false);
        assert_eq!(request(&mut a, json!({"command":"state"})), old);
    }
    assert_eq!(request(&mut b, json!({"command":"state"}))["ok"], false);
    for _ in 0..3 {
        assert_eq!(
            request(&mut a, json!({"command":"reset"}))["state"]["position"],
            "0"
        );
    }
    assert_eq!(request(&mut a, json!({"command":"dispose"}))["ok"], true);
    assert_eq!(load(&mut a)["ok"], false);
    assert_eq!(request(&mut a, json!({"command":"dispose"}))["ok"], false);
    assert_eq!(load(&mut b)["ok"], true);
}
#[test]
fn all_commands_preserve_replay_and_logical_run_semantics() {
    let mut w = Workbench::default();
    assert_eq!(
        request(&mut w, json!({"command":"validate","source":"program {}"}))["ok"],
        true
    );
    load(&mut w);
    request(&mut w, json!({"command":"advance","batch":"1"}));
    let saved = request(&mut w, json!({"command":"export"}));
    assert_eq!(
        request(&mut w, json!({"command":"stop"}))["state"]["status"],
        "Paused"
    );
    assert_eq!(
        request(&mut w, json!({"command":"jump","state":"0"}))["state"]["position"],
        "0"
    );
    assert_eq!(
        request(&mut w, json!({"command":"advance","batch":"1"}))["ok"],
        false
    );
    request(&mut w, json!({"command":"jump","state":"1"}));
    let mut imported = Workbench::default();
    assert_eq!(
        request(
            &mut imported,
            json!({"command":"import","trace":saved["trace"]})
        )["ok"],
        true
    );
    for _ in 0..2 {
        let mut authored = request(&mut w, json!({"command":"advance","batch":"1"}));
        let restored = request(&mut imported, json!({"command":"advance","batch":"1"}));
        // Trace-v1 retains executable identities, not optional source-name aliases.
        assert_eq!(restored["state"]["names"], json!({}));
        authored["state"]["names"] = json!({});
        assert_eq!(authored, restored);
    }
    let done = request(&mut w, json!({"command":"state"}));
    assert_eq!(done["state"]["status"], "CycleDetected");
    assert_eq!(done["state"]["report"]["recurrence"]["first"], "0");
    assert_eq!(done["state"]["report"]["recurrence"]["repeated"], "2");
}
#[test]
fn policy_limits_empty_graph_and_located_errors() {
    let mut w = Workbench::default();
    let bad = request(
        &mut w,
        json!({"command":"load","source":"program {\n bad }"}),
    );
    assert_eq!(bad["ok"], false);
    assert!(!bad["error"]["line"].is_null());
    let empty = request(
        &mut w,
        json!({"command":"load","source":"program {}","config":{"max_steps":"0"}}),
    );
    assert_eq!(empty["ok"], true);
    assert_eq!(
        request(&mut w, json!({"command":"advance","batch":"1"}))["state"]["status"],
        "NormalForm"
    );
    let bad = request(
        &mut w,
        json!({"command":"load","source":"program {}","config":{"max_steps":"4294967296"}}),
    );
    assert_eq!(bad["error"]["kind"], "ResourceExhaustion");
}
