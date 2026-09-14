use serde_json::{json, Map, Value};
use wasm_bridge::{
    dataset::{self, Policy, Status},
    Workbench,
};
fn ask(w: &mut Workbench, v: Value) -> Value {
    serde_json::to_value(w.dispatch(&v.to_string())).unwrap()
}
// Independent whole-carrier oracle: retain every non-domain atom and reference,
// rename only the deliberately replaced status/revision identities, then compare.
fn normal(state: &Value, head: &str) -> Map<String, Value> {
    let atoms = state["graph"]["atoms"].as_object().unwrap();
    let mut names = Map::new();
    for (id, a) in atoms {
        let role = a["kind"]["Record"]["role"].as_str();
        let fields = a["fields"].as_array().unwrap();
        if role == Some("ObjectLink")
            && fields.len() == 2
            && fields[0] == fields[1]
            && atoms[fields[0].as_str().unwrap()]["kind"]["Constructor"]
                .as_str()
                .is_some_and(|s| s.starts_with("WorkItem:"))
        {
            names.insert(
                id.clone(),
                json!(format!("status:{}", fields[0].as_str().unwrap())),
            );
        }
        if fields.first().and_then(Value::as_str) == Some(head)
            && matches!(role, Some("Revision" | "Active"))
        {
            names.insert(id.clone(), json!(format!("{}:{head}", role.unwrap())));
        }
    }
    atoms
        .iter()
        .map(|(id, a)| {
            let mut a = a.clone();
            for field in a["fields"].as_array_mut().unwrap() {
                if let Some(n) = names.get(field.as_str().unwrap()) {
                    *field = n.clone();
                }
            }
            (
                names
                    .get(id)
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_string(),
                a,
            )
        })
        .collect()
}
#[test]
fn both_policies_match_independent_whole_carrier_and_revision_oracles() {
    for policy in [Policy::Fixed, Policy::GuardedEnable] {
        for count in [0, 1, 10] {
            let mut data = dataset::generate(count, 42, policy.clone()).unwrap();
            if count > 1 {
                data.items[0].status = Status::Processed;
            }
            let pending = data
                .items
                .iter()
                .filter(|i| i.status == Status::Pending)
                .count();
            let source = dataset::source(&data).unwrap();
            let mut w = Workbench::default();
            let first = ask(
                &mut w,
                json!({"command":"load","source":source,"config":{"max_steps":"20"}}),
            );
            assert_eq!(first["ok"], true, "{first}");
            let head = first["state"]["names"]
                .as_object()
                .unwrap()
                .iter()
                .find(|(_, n)| n.as_str() == Some("process"))
                .unwrap()
                .0;
            let mut expected = normal(&first["state"], head);
            for (id, a) in expected.iter_mut() {
                if id.starts_with("status:") {
                    a["kind"]["Record"]["label"] = json!("processed");
                }
            }
            if policy == Policy::GuardedEnable {
                let permission = first["state"]["names"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .find(|(_, n)| n.as_str() == Some("permission"))
                    .unwrap()
                    .0;
                expected.remove(permission);
                expected.insert(
                    format!("Active:{head}"),
                    json!({"kind":{"Record":{"role":"Active","label":null}},"fields":[head]}),
                );
            }
            let result = ask(&mut w, json!({"command":"advance","batch":"20"}));
            assert_eq!(result["ok"], true, "{result}");
            assert_eq!(result["state"]["status"], "NormalForm");
            let events = result["state"]["events"].as_array().unwrap();
            let guard = usize::from(policy == Policy::GuardedEnable);
            assert_eq!(events.len(), pending + guard);
            assert_eq!(normal(&result["state"], head), expected);
            let next = first["state"]["graph"]["next_identity"]
                .as_str()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            assert_eq!(
                result["state"]["graph"]["next_identity"],
                json!((next + pending + 2 * guard).to_string())
            );
            if guard == 1 {
                let before = &first["state"]["rules"][0]["revision"];
                let after = &result["state"]["rules"][0]["revision"];
                assert_ne!(before, after);
                for e in &events[1..] {
                    assert_eq!(&e["redex"]["revision"], after);
                }
            }
        }
    }
}
#[test]
fn dataset_preview_and_bounded_inspection_are_atomic() {
    let mut w = Workbench::default();
    let old = ask(&mut w, json!({"command":"load","source":"program {}"}));
    for bad in [
        json!({"command":"generate_dataset","count":"1001","seed":"42","policy":"fixed"}),
        json!({"command":"dataset","data":"{\"schema\":\"work-items-v1\",\"policy\":\"fixed\",\"items\":[],\"unknown\":1}"}),
        json!({"command":"inspect","query":"","offset":"0","limit":"201"}),
    ] {
        assert_eq!(ask(&mut w, bad)["ok"], false);
        assert_eq!(ask(&mut w, json!({"command":"state"})), old);
    }
    let a = ask(
        &mut w,
        json!({"command":"generate_dataset","count":"1000","seed":"42","policy":"fixed"}),
    );
    assert_eq!(a["ok"], true);
    assert_eq!(ask(&mut w, json!({"command":"state"})), old);
    let b = ask(
        &mut w,
        json!({"command":"generate_dataset","count":"1000","seed":"42","policy":"fixed"}),
    );
    assert_eq!(a, b);
    let loaded = ask(
        &mut w,
        json!({"command":"load","source":a["dataset"]["source"]}),
    );
    assert_eq!(loaded["ok"], true);
    let page = ask(
        &mut w,
        json!({"command":"inspect","query":"WorkItem:work-0999","offset":"0","limit":"100"}),
    );
    assert_eq!(page["inspection"]["total"], "1");
    assert_eq!(page["inspection"]["atoms"].as_array().unwrap().len(), 1);
    assert_eq!(ask(&mut w, json!({"command":"state"})), loaded);
    for text in [
        r#"{"schema":"bad","policy":"fixed","items":[]}"#,
        r#"{"schema":"work-items-v1","policy":"fixed","items":[{"id":"x","status":"pending"},{"id":"x","status":"processed"}]}"#,
        r#"{"schema":"work-items-v1","policy":"fixed","items":[{"id":"x","status":"unknown"}]}"#,
    ] {
        assert_eq!(
            ask(&mut w, json!({"command":"dataset","data":text}))["ok"],
            false
        );
    }
}
#[test]
fn projects_verify_source_history_settings_and_position_before_replacement() {
    let mut w = Workbench::default();
    let source = dataset::source(&dataset::generate(3, 42, Policy::Fixed).unwrap()).unwrap();
    ask(&mut w, json!({"command":"load","source":source}));
    let old = ask(&mut w, json!({"command":"advance","batch":"1"}));
    let valid = json!({"command":"project","source":source,"trace":old["trace"],"execution":old["state"]["settings"],"position":"0"});
    for (key, value) in [
        ("source", json!("program {}")),
        ("position", json!("2")),
        ("execution", json!(null)),
        ("trace", json!("{}")),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert_eq!(ask(&mut w, bad)["ok"], false);
        assert_eq!(ask(&mut w, json!({"command":"state"})), old);
    }
    let mut forged = valid.clone();
    let mut forged_trace: Value = serde_json::from_str(old["trace"].as_str().unwrap()).unwrap();
    forged_trace["source_sha256"] = json!(engine::trace::sha256(b"program {}"));
    forged["source"] = json!("program {}");
    forged["trace"] = json!(forged_trace.to_string());
    assert_eq!(ask(&mut w, forged)["ok"], false);
    assert_eq!(ask(&mut w, json!({"command":"state"})), old);
    let restored = ask(&mut w, valid);
    assert_eq!(restored["ok"], true, "{restored}");
    assert_eq!(restored["state"]["position"], "0");
    assert_eq!(restored["state"]["names"], old["state"]["names"]);
    assert_eq!(restored["trace"], old["trace"]);
    assert_eq!(
        ask(
            &mut w,
            json!({"command":"project","source":"program {}","position":"0"})
        )["ok"],
        true
    );
}
