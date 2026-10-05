use engine::organization::{execute, Config, Request, Stage, System};
use serde_json::{json, Value};

fn request(value: Value) -> Request {
    serde_json::from_value(value).unwrap()
}

#[test]
fn canonical_rust_flow_renews_inherits_and_executes_without_python() {
    let request: Request =
        serde_json::from_str(include_str!("../../../examples/organization/renewal.json")).unwrap();
    let result = execute(&request).unwrap();
    assert_eq!(result["parent"]["summary"]["functional"], true);
    assert_eq!(result["child"]["summary"]["functional"], true);
    assert_eq!(result["child"]["summary"]["heldout"]["passed"], 512);
    assert_eq!(result["native"]["environmental_agreement"], true);
    assert_eq!(result["native"]["execution"]["admitted_steps"], 8);
    assert_eq!(
        result["native"]["execution"]["portable_trace_verified"],
        true
    );
    let parent = &result["parent"]["graph"]["root"];
    let remaining = &result["allocation"]["parent"]["root"];
    let child = &result["allocation"]["child"]["root"];
    for pool in ["template", "sequence"] {
        for bit in 0..2 {
            assert_eq!(
                parent["initial_symbols"][pool][bit].as_u64().unwrap(),
                remaining["initial_symbols"][pool][bit].as_u64().unwrap()
                    + child["initial_symbols"][pool][bit].as_u64().unwrap()
            );
        }
    }
    assert_eq!(
        parent["initial_energy"].as_u64().unwrap(),
        remaining["initial_energy"].as_u64().unwrap() + child["initial_energy"].as_u64().unwrap()
    );
    for (key, record) in child["model"].as_object().unwrap() {
        assert_eq!(
            record["last_checked"].as_i64().unwrap(),
            parent["model"][key]["last_checked"].as_i64().unwrap() - 1600
        );
        assert_eq!(
            record["witness_clock"],
            parent["model"][key]["witness_clock"]
        );
    }
    let trace = serde_json::to_vec(&result["native"]["execution"]["trace"]).unwrap();
    engine::trace::VerifiedTrace::import(&trace, Default::default()).unwrap();
}

#[test]
fn continuation_partition_preserves_the_full_state_and_limits_are_atomic() {
    let mut direct = System::new(Config::default(), vec![]).unwrap();
    let mut partitioned = direct.clone();
    let stages = request(json!({"stages":[{"epochs":100}]})).stages;
    direct.run_stages(&stages).unwrap();
    let half = request(json!({"stages":[{"epochs":50}]})).stages;
    partitioned.run_stages(&half).unwrap();
    partitioned.run_stages(&half).unwrap();
    assert_eq!(direct.snapshot(), partitioned.snapshot());
    let before = partitioned.snapshot();
    let too_many = request(json!({"stages":[{"epochs":8193}]})).stages;
    assert!(partitioned.run_stages(&too_many).is_err());
    assert_eq!(before, partitioned.snapshot());
}

#[test]
fn configuration_domains_and_unknown_fields_are_rejected() {
    for value in [
        json!({"config":{"seed":0}}),
        json!({"config":{"n":0}}),
        json!({"config":{"n":1025}}),
        json!({"config":{"rooms":33}}),
        json!({"config":{"length":11}}),
        json!({"config":{"symbol_factor":-1}}),
        json!({"config":{"columns":[0,0,1,2]}}),
        json!({"config":{"energy":1000000000001u64}}),
        json!({"inheritance":{"seed":0}}),
        json!({"inheritance":{"bottleneck":65}}),
        json!({"word":[2]}),
        json!({"word":vec![0;17]}),
        json!({"events":[{"at":2,"operation":"environment_swap"},{"at":1,"operation":"erase_sequences"}]}),
        json!({"stages":[{"epochs":u64::MAX},{"epochs":1}]}),
    ] {
        assert!(execute(&request(value.clone())).is_err(), "{value}");
    }
    for value in [
        json!({"injected":true}),
        json!({"events":[{"at":0,"operation":"environment_swap","unexpected":1}]}),
        json!({"events":[{"at":0,"operation":"recognition_scramble","unexpected":1}]}),
        json!({"events":[{"at":0,"operation":"erase_sequences","unexpected":1}]}),
        json!({"config":{"n":true}}),
        json!({"config":{"unknown":1}}),
        json!({"events":[{"at":0,"operation":"set","field":"rng","value":0}]}),
        json!({"stages":[{"epochs":1,"controls":{"unknown":true}}]}),
    ] {
        assert!(
            serde_json::from_value::<Request>(value.clone()).is_err(),
            "{value}"
        );
    }
}

#[test]
fn exhausted_resources_stop_paid_activity_but_do_not_freeze_removal() {
    let value = execute(&request(
        json!({"config":{"energy":0,"source":0},"stages":[{"epochs":8192}]}),
    ))
    .unwrap();
    let root = &value["parent"]["graph"]["root"];
    assert_eq!(root["actions"], 0);
    assert_eq!(root["spent_energy"], 0);
    assert!(root["blocked_energy"].as_u64().unwrap() > 0);
    for pool in ["template", "sequence"] {
        assert!(root["pools"][pool]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_null));
    }
    assert_eq!(value["parent"]["summary"]["functional"], false);
}

#[test]
fn typed_application_never_imports_an_arbitrary_snapshot_as_execution_authority() {
    let mut system = System::new(Config::default(), vec![]).unwrap();
    let snapshot = system.snapshot();
    assert!(serde_json::from_value::<Request>(snapshot).is_err());
    let zero = Stage {
        epochs: 0,
        controls: None,
        drain_resources: false,
        complement_operators: false,
    };
    let before = system.snapshot();
    system.run_stages(&[zero]).unwrap();
    assert_eq!(before, system.snapshot());
}
