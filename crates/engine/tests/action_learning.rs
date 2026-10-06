use engine::action_learning::*;
use serde_json::{json, Value};

fn fixture() -> Request {
    serde_json::from_str(include_str!("../../../examples/action-learning/doors.json")).unwrap()
}
fn snapshot(l: &Learner) -> Value {
    serde_json::to_value(l.snapshot()).unwrap()
}
fn tiny_schema() -> Schema {
    serde_json::from_value(json!({"id":"tiny", "types":["item"],
        "predicates":{"p":[],"q":[]},"actions":{"act":[]}}))
    .unwrap()
}
fn world(bits: u8) -> World {
    World {
        objects: Default::default(),
        facts: ["p", "q"]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Fact {
                atom: GroundAtom {
                    predicate: p.into(),
                    arguments: vec![],
                },
                value: bits & (1 << i) != 0,
            })
            .collect(),
    }
}
fn action() -> Action {
    Action {
        name: "act".into(),
        arguments: vec![],
    }
}
fn observation(l: &Learner, x: u8, y: Option<u8>) -> Observation {
    let state = l.snapshot();
    Observation {
        schema: "tiny".into(),
        environment: "test".into(),
        expected_version: state.version,
        epoch: state.models["act"].epoch,
        sequence: state.version + 1,
        action: action(),
        before: world(x),
        after: y.map(world),
    }
}
fn ternary(mut i: usize) -> [Option<bool>; 2] {
    std::array::from_fn(|_| {
        let v = [None, Some(false), Some(true)][i % 3];
        i /= 3;
        v
    })
}
fn applies(pre: &[Option<bool>; 2], x: u8) -> bool {
    pre.iter()
        .enumerate()
        .all(|(i, v)| v.is_none_or(|v| v == (x & (1 << i) != 0)))
}
fn successor(eff: &[Option<bool>; 2], x: u8) -> u8 {
    eff.iter().enumerate().fold(x, |out, (i, v)| match v {
        None => out,
        Some(true) => out | (1 << i),
        Some(false) => out & !(1 << i),
    })
}
#[test]
fn exhaustive_stationary_models_orders_and_negative_inference() {
    let mut prefixes = 0;
    let mut queries = 0;
    let mut safe = 0;
    for p in 0..9 {
        for e in 0..9 {
            let pre = ternary(p);
            let eff = ternary(e);
            for a in 0..4 {
                for b in 0..4 {
                    for c in 0..4 {
                        for d in 0..4 {
                            let order = [a, b, c, d];
                            if order
                                .iter()
                                .collect::<std::collections::BTreeSet<_>>()
                                .len()
                                != 4
                            {
                                continue;
                            }
                            let mut l =
                                Learner::new(tiny_schema(), "test".into(), Repair::Local).unwrap();
                            let mut positives = Vec::new();
                            for x in order {
                                let y = applies(&pre, x).then(|| successor(&eff, x));
                                let o = observation(&l, x, y);
                                let r = l.observe(&o).unwrap();
                                assert!(!r.alarm);
                                prefixes += 1;
                                if let Some(y) = y {
                                    positives.push((x, y));
                                }
                                // Independent batch set-intersection/changed-coordinate oracle.
                                if !positives.is_empty() {
                                    let m = &l.snapshot().models["act"];
                                    for i in 0..2 {
                                        let values: std::collections::BTreeSet<_> = positives
                                            .iter()
                                            .map(|(x, _)| x & (1 << i) != 0)
                                            .collect();
                                        assert_eq!(
                                            m.preconditions[i],
                                            (values.len() == 1).then(|| *values.first().unwrap())
                                        );
                                        let effects: std::collections::BTreeSet<_> = positives
                                            .iter()
                                            .filter_map(|(x, y)| {
                                                ((x ^ y) & (1 << i) != 0)
                                                    .then_some(y & (1 << i) != 0)
                                            })
                                            .collect();
                                        assert_eq!(m.effects[i], effects.first().copied());
                                    }
                                }
                                for z in 0..4 {
                                    queries += 1;
                                    match l.classify(&action(), &world(z)).unwrap() {
                                        Classification::Safe => {
                                            safe += 1;
                                            assert!(applies(&pre, z));
                                            let m = &l.snapshot().models["act"];
                                            let learned = [m.effects[0], m.effects[1]];
                                            assert_eq!(successor(&learned, z), successor(&eff, z));
                                        }
                                        Classification::Rejected => assert!(!applies(&pre, z)),
                                        Classification::Unknown => (),
                                        Classification::Quarantined => {
                                            panic!("stationary evidence quarantined")
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!((prefixes, queries, safe), (7776, 31104, 9000));
    println!(
        "ACTION_EVIDENCE prefixes={prefixes} classifications={queries} safe_predictions={safe}"
    );
}
#[test]
fn native_all_complete_two_feature_models_and_inputs() {
    let mut executions = 0;
    let mut steps = 0;
    for p in 0..9 {
        for e in 0..9 {
            let pre = ternary(p);
            let eff = ternary(e);
            let mut request = Request {
                schema: tiny_schema(),
                environment: "test".into(),
                repair: Repair::Local,
                observations: vec![],
                run: None,
            };
            let mut l = Learner::new(tiny_schema(), "test".into(), Repair::Local).unwrap();
            for x in 0..4 {
                let o = observation(&l, x, applies(&pre, x).then(|| successor(&eff, x)));
                l.observe(&o).unwrap();
                request.observations.push(o);
            }
            for x in 0..4 {
                request.run = Some(Run {
                    expected_version: 4,
                    initial: world(x),
                    actions: vec![action()],
                    native_max_work: None,
                });
                let r = execute(&request).unwrap();
                executions += 1;
                let expected = applies(&pre, x);
                assert_eq!(r["execution"]["completed"], expected);
                assert_eq!(r["execution"]["admitted_steps"], usize::from(expected));
                assert_eq!(r["execution"]["portable_trace_verified"], true);
                let actual = r["execution"]["states"].as_array().unwrap().last().unwrap();
                assert_eq!(
                    *actual,
                    serde_json::to_value(
                        world(if expected { successor(&eff, x) } else { x }).facts
                    )
                    .unwrap()
                );
                steps += usize::from(expected);
            }
        }
    }
    assert_eq!((executions, steps), (324, 144));
    println!("ACTION_NATIVE executions={executions} admitted_steps={steps}");
}
#[test]
fn transfer_frame_nullary_conditions_and_replay() {
    let mut r = fixture();
    let report = execute(&r).unwrap();
    assert_eq!(report["execution"]["admitted_steps"], 2);
    assert_eq!(report["execution"]["states"][1][1]["value"], false); // rear untouched
    assert_eq!(report["execution"]["states"][2][1]["value"], true);
    r.run.as_mut().unwrap().initial.facts[2].value = false;
    let report = execute(&r).unwrap();
    assert_eq!(report["execution"]["admitted_steps"], 0);
    assert_eq!(report["execution"]["completed"], false);
}
#[test]
fn malformed_and_stale_observations_are_atomic() {
    let r = fixture();
    let (base, _) = fit(&r).unwrap();
    for case in 0..10 {
        let mut l = base.clone();
        let before = snapshot(&l);
        let mut o = r.observations[0].clone();
        o.expected_version = 2;
        o.sequence = 3;
        match case {
            0 => o.schema = "foreign".into(),
            1 => o.environment = "foreign".into(),
            2 => o.expected_version = 1,
            3 => o.sequence = 4,
            4 => o.epoch = 1,
            5 => {
                o.before.facts.pop();
            }
            6 => o.before.facts.push(o.before.facts[0].clone()),
            7 => o.action.arguments.push("training".into()),
            8 => {
                o.after
                    .as_mut()
                    .unwrap()
                    .objects
                    .insert("extra".into(), "door".into());
            }
            9 => o
                .before
                .objects
                .insert("training".into(), "unknown".into())
                .map(|_| ())
                .unwrap(),
            _ => unreachable!(),
        }
        assert!(l.observe(&o).is_err(), "case {case}");
        assert_eq!(snapshot(&l), before);
    }
    let mut l =
        Learner::with_observation_limit(r.schema.clone(), r.environment.clone(), Repair::Local, 1)
            .unwrap();
    l.observe(&r.observations[0]).unwrap();
    let before = snapshot(&l);
    assert!(l.observe(&r.observations[1]).is_err());
    assert_eq!(snapshot(&l), before);
}
#[test]
fn repair_quarantine_and_environmental_change_boundary() {
    for repair in [Repair::Local, Repair::Quarantine] {
        let mut l = Learner::new(tiny_schema(), "test".into(), repair).unwrap();
        let o = observation(&l, 0, Some(1));
        l.observe(&o).unwrap();
        assert_eq!(
            l.classify(&action(), &world(0)).unwrap(),
            Classification::Safe
        );
        let old = snapshot(&l);
        let o = observation(&l, 0, Some(2));
        let receipt = l.observe(&o).unwrap();
        assert!(receipt.alarm);
        assert_ne!(snapshot(&l), old);
        match repair {
            Repair::Local => {
                assert_eq!(receipt.epoch, 1);
                assert_eq!(l.snapshot().models["act"].effects, vec![None, Some(true)]);
            }
            Repair::Quarantine => assert_eq!(
                l.classify(&action(), &world(0)).unwrap(),
                Classification::Quarantined
            ),
        }
        let mut stale = observation(&l, 0, Some(2));
        stale.expected_version = 1;
        let before = snapshot(&l);
        assert!(l.observe(&stale).is_err());
        assert_eq!(snapshot(&l), before);
    }
}
#[test]
fn input_frame_injectivity_schema_bounds_and_search_failures() {
    let r = fixture();
    let (mut l, _) = fit(&r).unwrap();
    let mut o = r.observations[0].clone();
    o.expected_version = 2;
    o.sequence = 3;
    o.before = r.run.as_ref().unwrap().initial.clone();
    o.action.arguments = vec!["front".into()];
    o.after = Some(o.before.clone());
    o.after.as_mut().unwrap().facts[1].value = true;
    let before = snapshot(&l);
    assert!(l.observe(&o).is_err());
    assert_eq!(snapshot(&l), before);
    let mut r = fixture();
    r.schema
        .actions
        .insert("pair".into(), vec!["door".into(), "door".into()]);
    let (l, _) = fit(&r).unwrap();
    assert!(l
        .classify(
            &Action {
                name: "pair".into(),
                arguments: vec!["front".into(), "front".into()]
            },
            &r.run.as_ref().unwrap().initial
        )
        .is_err());
    r.run.as_mut().unwrap().native_max_work = Some(0);
    assert!(matches!(execute(&r), Err(Error::Native(_))));
    let mut schema = tiny_schema();
    schema
        .predicates
        .insert("huge".into(), vec!["item".into(); 4]);
    schema.actions.insert("four".into(), vec!["item".into(); 4]);
    assert!(Learner::new(schema, "test".into(), Repair::Local).is_err());
}
#[test]
fn acquired_versions_and_compilation_are_immutable() {
    let r = fixture();
    let (mut l, _) = fit(&r).unwrap();
    let old = l.snapshot();
    let graph = compile(&l, r.run.as_ref().unwrap()).unwrap().graph;
    let printed = parser::print(&graph).unwrap();
    parser::load(&printed).unwrap();
    let mut o = r.observations[0].clone();
    o.expected_version = 2;
    o.sequence = 3;
    l.observe(&o).unwrap();
    assert_eq!(old.version, 2);
    assert_eq!(l.snapshot().version, 3);
    assert!(compile(&l, r.run.as_ref().unwrap()).is_err());
    assert_eq!(parser::print(&graph).unwrap(), printed);
}

#[test]
fn hidden_conflict_in_negative_evidence_repairs_atomically() {
    // New positive 11 is Unknown under pre=00; dropping both literals would
    // incorrectly certify the earlier failed 01 unless the conflict is detected.
    let mut l = Learner::new(tiny_schema(), "test".into(), Repair::Local).unwrap();
    for (x, y) in [(0, Some(0)), (1, None)] {
        let o = observation(&l, x, y);
        l.observe(&o).unwrap();
    }
    let o = observation(&l, 3, Some(3));
    let receipt = l.observe(&o).unwrap();
    assert!(receipt.alarm);
    assert_eq!(receipt.epoch, 1);
    assert_eq!(
        l.snapshot().models["act"].preconditions,
        vec![Some(true), Some(true)]
    );
}

#[test]
fn native_rule_removal_changes_execution() {
    let r = fixture();
    let (l, _) = fit(&r).unwrap();
    let mut g = compile(&l, r.run.as_ref().unwrap()).unwrap().graph;
    let active = g
        .iter()
        .filter_map(|(k, a)| {
            matches!(
                a.kind,
                graph_core::Kind::Record {
                    role: graph_core::Role::Active,
                    ..
                }
            )
            .then_some(k)
        })
        .collect();
    g.remove_set(&active).unwrap();
    let run = engine::Execution::new(g, Default::default()).unwrap();
    assert!(run.enabled().unwrap().into_steps().is_empty());
}

#[test]
fn no_positive_abstention_and_per_schema_repair() {
    let mut schema = tiny_schema();
    schema.actions.insert("other".into(), vec![]);
    let mut l = Learner::new(schema, "test".into(), Repair::Local).unwrap();
    for x in 0..3 {
        let o = observation(&l, x, None);
        l.observe(&o).unwrap();
    }
    assert_eq!(
        l.classify(&action(), &world(3)).unwrap(),
        Classification::Unknown
    );
    let o = observation(&l, 3, Some(3));
    l.observe(&o).unwrap();
    let mut o = observation(&l, 0, Some(1));
    o.action.name = "other".into();
    l.observe(&o).unwrap();
    let other = serde_json::to_value(&l.snapshot().models["other"]).unwrap();
    let o = observation(&l, 3, None);
    assert!(l.observe(&o).unwrap().alarm);
    assert_eq!(l.snapshot().models["act"].epoch, 1);
    assert_eq!(
        serde_json::to_value(&l.snapshot().models["other"]).unwrap(),
        other
    );
    let before = snapshot(&l);
    let mut o = observation(&l, 3, Some(3));
    o.epoch = 0;
    assert!(l.observe(&o).is_err());
    assert_eq!(snapshot(&l), before);
}

#[test]
fn binary_predicates_repeated_positions_and_renamed_frame() {
    let schema: Schema = serde_json::from_value(json!({"id":"relations","types":["node"],
        "predicates":{"connected":["node","node"]},"actions":{"connect":["node","node"]}}))
    .unwrap();
    let make_world = |names: &[&str], bits: u16| {
        let mut facts = Vec::new();
        for (i, a) in names.iter().enumerate() {
            for (j, b) in names.iter().enumerate() {
                facts.push(Fact {
                    atom: GroundAtom {
                        predicate: "connected".into(),
                        arguments: vec![(*a).into(), (*b).into()],
                    },
                    value: bits & (1 << (i * names.len() + j)) != 0,
                });
            }
        }
        World {
            objects: names.iter().map(|n| ((*n).into(), "node".into())).collect(),
            facts,
        }
    };
    let mut request = Request {
        schema: schema.clone(),
        environment: "relations-test".into(),
        repair: Repair::Local,
        observations: vec![],
        run: None,
    };
    let mut l = Learner::new(schema, "relations-test".into(), Repair::Local).unwrap();
    for bits in 0..16 {
        let before = make_world(&["a", "b"], bits);
        let mut after = before.clone();
        after.facts[1].value = true;
        let o = Observation {
            schema: "relations".into(),
            environment: "relations-test".into(),
            expected_version: bits.into(),
            epoch: 0,
            sequence: u64::from(bits) + 1,
            action: Action {
                name: "connect".into(),
                arguments: vec!["a".into(), "b".into()],
            },
            before,
            after: Some(after),
        };
        assert!(!l.observe(&o).unwrap().alarm);
        request.observations.push(o);
    }
    assert_eq!(l.snapshot().features["connect"].len(), 4);
    assert_eq!(l.snapshot().models["connect"].preconditions, vec![None; 4]);
    for bits in [0, 85, 511] {
        let initial = make_world(&["x", "y", "z"], bits);
        let mut expected = initial.facts.clone();
        expected[6].value = true;
        request.run = Some(Run {
            expected_version: 16,
            initial,
            actions: vec![Action {
                name: "connect".into(),
                arguments: vec!["z".into(), "x".into()],
            }],
            native_max_work: None,
        });
        let report = execute(&request).unwrap();
        assert_eq!(
            report["execution"]["states"][1],
            serde_json::to_value(expected).unwrap()
        );
    }
}
