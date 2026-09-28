use engine::{
    observation::Observer,
    runner::{ExecutionProfile, Outcome, RunLimits, RunProgress, Runner},
    scheduling::Scheduler,
    trace::{TraceDocument, TraceLimits, TraceOptions, VerifiedTrace},
    Execution, ExecutionLimits,
};
use graph_core::{Graph, Kind, NewAtom, Role, Target};
const ITEMS: &str = include_str!("../../../examples/practical/work-items.graph");
#[test]
fn framed_replay_rejects_forged_events_profiles_and_continuation_claims() {
    let mut r = runner(ITEMS, ExecutionProfile::FramedV3, 42);
    r.run(
        RunLimits {
            max_steps: 10,
            ..Default::default()
        },
        None,
        || false,
    )
    .unwrap();
    let document =
        TraceDocument::from_runner(&r, None, TraceOptions::default(), TraceLimits::default())
            .unwrap();
    let mut event = document.clone();
    event.events[0].footprint.consumed.clear();
    assert!(VerifiedTrace::verify(event, TraceLimits::default()).is_err());
    let mut continuation = document.clone();
    continuation
        .checkpoints
        .last_mut()
        .unwrap()
        .continuation_sha256 = Some("0".repeat(64));
    assert!(VerifiedTrace::verify(continuation, TraceLimits::default()).is_err());
    let mut profile = serde_json::to_value(&document).unwrap();
    profile["settings"]["execution_profile"] = serde_json::json!("invented-v99");
    assert!(VerifiedTrace::import(
        &serde_json::to_vec(&profile).unwrap(),
        TraceLimits::default()
    )
    .is_err());
    let limits = TraceLimits {
        max_replay_work: 1,
        ..Default::default()
    };
    assert!(VerifiedTrace::verify(document, limits).is_err());
}
fn runner(source: &str, profile: ExecutionProfile, seed: u64) -> Runner<Scheduler> {
    let mut r = Runner::with_profile(
        parser::load(source).unwrap().into_graph(),
        ExecutionLimits::default(),
        Scheduler::seeded(seed),
        Observer::all(),
        profile,
    )
    .unwrap();
    r.enable_recording(Default::default()).unwrap();
    r
}
#[test]
fn frame_matches_full_validation_including_budget_boundaries_and_fallbacks() {
    for source in [
        ITEMS,
        include_str!("../../../examples/programs/arithmetic.graph"),
        "program { rule grow { atom z = Nat(); rank rk = z; mode md = ModeObject; revision rev; active act; atom c = Constructor(\"new\"); lhs {} rhs { rc: c; } } }",
        include_str!("../../../examples/programs/self-modify.graph"),
        include_str!("../../../examples/programs/restoration.graph"),
        include_str!("../../../examples/practical/policy-enable.graph"),
        include_str!("../../../examples/programs/staging.graph"),
    ] {
        let g = parser::load(source).unwrap().into_graph();
        let exact = graph_core::validate(&g).unwrap().validation_work;
        for allowance in [exact, exact + 1, exact + 100, 10_000_000] {
            let mut limits = ExecutionLimits::default();
            limits.graph.max_validation_work = allowance;
            let mut a = Execution::new(g.clone(), limits).unwrap();
            let mut b = Execution::with_frame_validation(g.clone(), limits).unwrap();
            for _ in 0..6 {
                let mut chosen = None;
                for head in a
                    .view()
                    .unwrap()
                    .live_rules()
                    .map(|r| r.head)
                    .collect::<Vec<_>>()
                {
                    for binding in a.bindings(head).unwrap() {
                        match (a.prepare(binding.clone()), b.prepare(binding)) {
                            (Ok(x), Ok(y)) => {
                                assert_eq!(x.event(), y.event());
                                if chosen.is_none() {
                                    chosen = Some((x, y));
                                }
                            }
                            (Err(x), Err(y)) => {
                                assert_eq!(x.kind(), y.kind());
                                assert_eq!(x.to_string(), y.to_string());
                            }
                            other => panic!("frame/reference mismatch: {other:?}"),
                        }
                    }
                }
                let Some((x, y)) = chosen else { break };
                a.commit(x).unwrap();
                b.commit(y).unwrap();
                assert_eq!(a.graph().snapshot(), b.graph().snapshot());
                assert_eq!(a.graph().next_identity(), b.graph().next_identity());
            }
        }
    }
}
#[test]
fn framed_schedules_events_and_continuations_match_reference() {
    for source in [
        ITEMS,
        include_str!("../../../examples/practical/policy-enable.graph"),
        include_str!("../../../examples/programs/self-modify.graph"),
        include_str!("../../../examples/programs/restoration.graph"),
        include_str!("../../../examples/behaviour/oscillating.graph"),
    ] {
        for seed in [0, 42, 999] {
            let mut a = runner(source, ExecutionProfile::ReferenceV1, seed);
            let mut b = runner(source, ExecutionProfile::FramedV3, seed);
            let limits = RunLimits {
                max_steps: 12,
                detect_cycles: true,
                ..Default::default()
            };
            let expected = a.run(limits, None, || false).unwrap();
            b.begin_run(limits, None).unwrap();
            assert!(matches!(
                b.advance(1, || false).unwrap(),
                RunProgress::Yielded
            ));
            let document = TraceDocument::from_runner(
                &b,
                None,
                TraceOptions::default(),
                TraceLimits::default(),
            )
            .unwrap();
            let verified = VerifiedTrace::verify(document, TraceLimits::default()).unwrap();
            let mut resumed = verified.resume_at(1).unwrap();
            let actual = loop {
                if let RunProgress::Finished(report) = resumed.advance(2, || false).unwrap() {
                    break report;
                }
            };
            assert_eq!(actual.outcome, expected.outcome);
            assert_eq!(actual.recurrence, expected.recurrence);
            assert_eq!(
                a.recording().unwrap().events(),
                resumed.recording().unwrap().events()
            );
            assert_eq!(
                a.execution().graph().snapshot(),
                resumed.execution().graph().snapshot()
            );
        }
    }
}
#[test]
fn recurrence_gate_history_replacement_and_projected_observers_are_atomic() {
    let source = include_str!("../../../examples/behaviour/oscillating.graph");
    for declaration in [
        "observer-v1\nall\n",
        "observer-v1\nselect carrier\nlink 76616c7565\n",
    ] {
        let g = parser::load(source).unwrap().into_graph();
        let observer = Observer::parse(declaration, |_| None, 1000).unwrap();
        let mut a = Runner::with_profile(
            g.clone(),
            ExecutionLimits::default(),
            Scheduler::seeded(42),
            observer.clone(),
            ExecutionProfile::ReferenceV1,
        )
        .unwrap();
        let mut b = Runner::with_profile(
            g.clone(),
            ExecutionLimits::default(),
            Scheduler::seeded(42),
            observer.clone(),
            ExecutionProfile::FramedV3,
        )
        .unwrap();
        let limits = RunLimits {
            max_steps: 20,
            detect_cycles: true,
            ..Default::default()
        };
        let expected = a.run(limits, None, || false).unwrap();
        b.begin_run(limits, None).unwrap();
        assert!(matches!(
            b.advance_guarded(1, || false, |_| false).unwrap(),
            RunProgress::Yielded
        ));
        assert_eq!(b.retained_states(), 0);
        let actual = loop {
            if let RunProgress::Finished(r) = b.advance(1, || false).unwrap() {
                break r;
            }
        };
        assert_eq!(actual.recurrence, expected.recurrence);
        b.replace_graph(g, observer).unwrap();
        assert_eq!(b.retained_states(), 0);
        let mut zero = limits;
        zero.observation.max_history_units = 0;
        assert_eq!(
            b.run(zero, None, || false).unwrap().outcome,
            Outcome::ResourceLimit
        );
        assert_eq!(b.execution().committed_steps(), 0);
    }
}
#[test]
fn maintained_graph_metadata_matches_independent_scans_and_shared_snapshots() {
    let mut g = Graph::new();
    let mut snapshots = Vec::new();
    for i in 0..100 {
        let ids = g
            .insert_batch(&[
                NewAtom {
                    kind: Kind::Constructor(format!("n{i}")),
                    fields: vec![],
                },
                NewAtom {
                    kind: Kind::Constructor(format!("m{i}")),
                    fields: vec![],
                },
                NewAtom {
                    kind: Kind::Record {
                        role: Role::ObjectLink,
                        label: Some("value".into()),
                    },
                    fields: vec![Target::Fresh(0), Target::Fresh(1)],
                },
            ])
            .unwrap();
        snapshots.push(g.shared_carrier());
        if i % 3 == 0 {
            g.remove_set(&ids.into_iter().collect()).unwrap();
        }
        assert_eq!(
            g.field_count(),
            g.iter().map(|(_, a)| a.fields.len()).sum::<usize>()
        );
        for (id, _) in g.iter() {
            let scan = g
                .iter()
                .filter(|(_, a)| a.fields.contains(&id))
                .map(|(k, _)| k)
                .collect();
            assert_eq!(g.references_to(id), scan);
        }
        let o = engine::observation::all(&g, Default::default(), Default::default()).unwrap();
        assert_eq!(g.retention_units(), o.retention_units());
        assert_eq!(g.value_payload(), o.value_payload());
        let restored = g.shared_carrier().restore(&Default::default()).unwrap();
        assert_eq!(g.shape_signature(), restored.shape_signature());
        assert_eq!(g.snapshot(), restored.snapshot());
    }
    for s in snapshots {
        assert_eq!(
            s.signature(),
            s.restore(&Default::default()).unwrap().shape_signature()
        );
    }
}

#[test]
fn certificate_is_label_parametric_and_neither_projection_nor_replacement_changes_choices() {
    for source in [ITEMS.to_string(),ITEMS.replace("pending","unreviewed").replace("processed","accepted"),ITEMS.replace("atom item = PatternVariable[Constructor] Free;\n atom pending = Record[ObjectLink](\"pending\", item, item);","atom pending = Record[ObjectLink](\"pending\", item, item);\n atom item = PatternVariable[Constructor] Free;")] {
        let mut graph=parser::load(&source).unwrap().into_graph();
        graph.insert(Kind::Nat,&[]).unwrap(); // Unrelated valid atoms are allowed.
        for seed in 0..8 {
            let mut reference=Runner::new(graph.clone(),ExecutionLimits::default(),Scheduler::seeded(seed),Observer::all()).unwrap();
            let mut framed=Runner::with_profile(graph.clone(),ExecutionLimits::default(),Scheduler::seeded(seed),Observer::all(),ExecutionProfile::FramedV3).unwrap();
            reference.enable_recording(Default::default()).unwrap();framed.enable_recording(Default::default()).unwrap();
            let limits=RunLimits {max_steps:12,detect_cycles:true,..Default::default()};
            reference.run(limits,None,||false).unwrap();framed.begin_run(limits,None).unwrap();
            assert!(matches!(framed.advance_guarded(1,||false,|_|false).unwrap(),RunProgress::Yielded));
            assert_eq!(framed.retained_states(),0);assert_eq!(framed.execution().committed_steps(),0);
            loop {if matches!(framed.advance(2,||false).unwrap(),RunProgress::Finished(_)){break}}
            assert_eq!(reference.recording().unwrap().events(),framed.recording().unwrap().events());
            framed.replace_graph(graph.clone(),Observer::all()).unwrap();framed.run(limits,None,||false).unwrap();
            assert_eq!(reference.recording().unwrap().events(),framed.recording().unwrap().events());
        }
    }
}
