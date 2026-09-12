#[allow(dead_code)]
mod support;
use engine::observation::Observer;
use engine::recording::{AtomId, Recording, RecordingLimits};
use engine::runner::{Outcome, RunLimits, RunProgress, Runner};
use engine::scheduling::{Scheduler, SchedulingStrategy};
use engine::trace::*;
use engine::{Execution, ExecutionLimits};
use graph_core::{Graph, GraphLimits, Kind, Role, Sort};
use support::*;

fn graph() -> Graph {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    let y = variable(&mut g, Sort::Constructor);
    let old = link(&mut g, "pending", x, y);
    let new = link(&mut g, "done", x, y);
    sides(&mut g, h, &[x, y, old], &[x, y, new]);
    let a = ctor(&mut g, "λ 🦀 \" \\ \n");
    let b = ctor(&mut g, "B");
    link(&mut g, "pending", a, b);
    link(&mut g, "pending", a, b);
    g
}
fn runner(g: Graph) -> Runner<Scheduler> {
    let mut r = Runner::new(
        g,
        ExecutionLimits::default(),
        Scheduler::seeded(42),
        Observer::all(),
    )
    .unwrap();
    r.enable_recording(RecordingLimits::default()).unwrap();
    r
}
fn doc() -> TraceDocument {
    let mut r = runner(graph());
    r.run(
        RunLimits {
            max_steps: 2,
            ..Default::default()
        },
        None,
        || false,
    )
    .unwrap();
    TraceDocument::from_runner(
        &r,
        None,
        TraceOptions {
            checkpoint_interval: 1,
            conflict_states: vec![0],
        },
        TraceLimits::default(),
    )
    .unwrap()
}
fn check(d: TraceDocument) -> VerifiedTrace {
    VerifiedTrace::import(
        &d.to_json(TraceLimits::default().max_bytes).unwrap(),
        TraceLimits::default(),
    )
    .unwrap()
}

#[test]
fn deterministic_json_unicode_checked_replay_every_index_and_derived_conflicts() {
    let d = doc();
    let bytes = d.to_json(1_000_000).unwrap();
    assert_eq!(bytes, doc().to_json(1_000_000).unwrap());
    assert!(std::str::from_utf8(&bytes).unwrap().contains("λ 🦀"));
    let t = check(d.clone());
    assert_eq!(t.document(), &d);
    for i in 0..=2 {
        assert_eq!(
            GraphData::from_graph(&t.state_at(i).unwrap()),
            GraphData::from_graph(&t.state_from_initial(i).unwrap())
        );
    }
    assert!(t.state_at(3).is_err());
    assert!(t.resume_at(1).is_err());
    assert!(t.resume_at(2).is_err());
    assert!(d.dependencies.unwrap().is_empty());
    let c = d.conflicts[0].analysis.as_ref().unwrap();
    assert_eq!(c.pairs[0].commutes, Some(true));
}
#[test]
fn paused_seeded_continuation_preserves_history_and_allocator() {
    let mut r = runner(graph());
    let limits = RunLimits {
        max_steps: 2,
        detect_cycles: true,
        ..Default::default()
    };
    r.begin_run(limits, None).unwrap();
    assert!(r.begin_run(limits, None).is_err());
    assert!(r.advance(0, || false).is_err());
    assert!(matches!(
        r.advance(1, || false).unwrap(),
        RunProgress::Yielded
    ));
    let d = TraceDocument::from_runner(&r, None, Default::default(), Default::default()).unwrap();
    let t = check(d);
    assert!(t.resume_at(0).is_err());
    let mut resumed = t.resume_at(1).unwrap();
    let finish = |r: &mut Runner<Scheduler>| loop {
        if let RunProgress::Finished(report) = r.advance(1, || false).unwrap() {
            break report;
        }
    };
    let a = finish(&mut r);
    let b = finish(&mut resumed);
    assert_eq!(ReportData::from(&a), ReportData::from(&b));
    assert_eq!(a.outcome, Outcome::NormalForm);
    assert_eq!(
        r.recording().unwrap().events(),
        resumed.recording().unwrap().events()
    );
    assert_eq!(
        GraphData::from_graph(r.execution().graph()),
        GraphData::from_graph(resumed.execution().graph())
    );
    let mut whole = runner(graph());
    let c = whole.run(limits, None, || false).unwrap();
    assert_eq!(ReportData::from(&a), ReportData::from(&c));
    assert_eq!(
        whole.scheduler().description(),
        resumed.scheduler().description()
    );
}
#[test]
fn recurrence_spans_yields_and_intentional_new_run_resets_history() {
    let mut g = Graph::new();
    rule(&mut g, 0, false, true);
    let mut r = runner(g.clone());
    let limits = RunLimits {
        max_steps: 10,
        detect_cycles: true,
        ..Default::default()
    };
    r.begin_run(limits, None).unwrap();
    assert!(matches!(
        r.advance(1, || false).unwrap(),
        RunProgress::Yielded
    ));
    let trace = check(
        TraceDocument::from_runner(&r, None, Default::default(), Default::default()).unwrap(),
    );
    let mut r = trace.resume_at(1).unwrap();
    let RunProgress::Finished(report) = r.advance(1, || false).unwrap() else {
        panic!("cycle expected")
    };
    assert_eq!(report.outcome, Outcome::CycleDetected);
    let c = report.recurrence.unwrap();
    assert_eq!((c.first, c.repeated), (0, 1));
    r.begin_run(limits, None).unwrap();
    assert_eq!(r.retained_states(), 0);
    assert!(matches!(
        r.advance(1, || false).unwrap(),
        RunProgress::Yielded
    ));
    r.replace_graph(g, Observer::all()).unwrap();
    assert!(!r.is_active());
    assert_eq!(r.execution().committed_steps(), 0);
    assert_eq!(r.retained_states(), 0);
}
#[test]
fn forged_events_edges_reports_and_checkpoint_metadata_rejected_atomically() {
    let d = doc();
    let mut session = check(d.clone());
    let original = session.document().clone();
    let mut cases = Vec::new();
    let mut c = d.clone();
    c.events[0].redex.revision = AtomId(u64::MAX);
    cases.push(c);
    let mut c = d.clone();
    let binding = c.events[0].redex.binding[0];
    c.events[0].redex.binding.push(binding);
    cases.push(c);
    let mut c = d.clone();
    c.events[0].footprint.consumed.clear();
    cases.push(c);
    let mut c = d.clone();
    c.events[0].fresh.values_mut().next().unwrap().fields[0] = AtomId(u64::MAX);
    cases.push(c);
    let mut c = d.clone();
    c.events[0].post_state = 7;
    cases.push(c);
    let mut c = d.clone();
    c.events[0].id.0 = 1;
    cases.push(c);
    let mut c = d.clone();
    c.checkpoints[1].graph.next_identity.0 += 1;
    cases.push(c);
    let mut c = d.clone();
    c.checkpoints[1].graph = c.initial.clone();
    cases.push(c);
    let mut c = d.clone();
    c.checkpoints[1].scheduler = Some("forged RNG".into());
    cases.push(c);
    let mut c = d.clone();
    c.checkpoints[1].continuation_sha256 = Some("forged observer/history".into());
    cases.push(c);
    let mut c = d.clone();
    c.checkpoints[1].graph.next_identity.0 = 0;
    cases.push(c);
    let mut c = d.clone();
    c.conflicts[0].analysis.as_mut().unwrap().pairs[0].commutes = Some(false);
    cases.push(c);
    let mut c = d.clone();
    c.report.as_mut().unwrap().outcome = "StepLimit".into();
    cases.push(c);
    let mut c = d.clone();
    c.dependencies = Some(vec![EdgeData {
        before: engine::recording::EventId(1),
        after: engine::recording::EventId(0),
        witnesses: Default::default(),
    }]);
    cases.push(c);
    let mut c = d.clone();
    c.settings.observer.nontriviality_threshold = Number(0);
    cases.push(c);
    let mut c = d.clone();
    c.settings
        .observer
        .inputs
        .insert("unused".into(), AtomId(0));
    cases.push(c);
    for c in cases {
        assert!(session
            .replace_from_json(&c.to_json(1_000_000).unwrap())
            .is_err());
        assert_eq!(session.document(), &original);
    }
}
#[test]
fn strict_json_rejects_duplicates_unknown_missing_and_malformed_integers() {
    let d = doc();
    let json = String::from_utf8(d.to_json(1_000_000).unwrap()).unwrap();
    let bads = [
        json.replacen("{", "{\"schema\":\"x\",", 1),
        json.replacen("{", "{\"unknown\":true,", 1),
        json.replace("\"reflective-trace-v1\"", "\"v999\""),
        json.replacen("\"next_identity\": \"", "\"next_identity\": \"0", 1),
        json.replacen("\"state\": \"0\"", "\"state\": 0", 1),
        json.replacen(
            "\"next_identity\": \"",
            "\"next_identity\": \"18446744073709551616",
            1,
        ),
        json[..json.len() - 2].into(),
        format!("{json} null"),
    ];
    for text in bads {
        assert!(
            VerifiedTrace::import(text.as_bytes(), Default::default()).is_err(),
            "accepted malformed input"
        );
    }
    let mut value = serde_json::to_value(d).unwrap();
    value.as_object_mut().unwrap().remove("report");
    assert!(
        VerifiedTrace::import(&serde_json::to_vec(&value).unwrap(), Default::default()).is_err()
    );
    let mut value = serde_json::to_value(doc()).unwrap();
    let set = value["events"][0]["footprint"]["read"]
        .as_array_mut()
        .unwrap();
    set.push(set[0].clone());
    assert!(
        VerifiedTrace::import(&serde_json::to_vec(&value).unwrap(), Default::default()).is_err()
    );
}
#[test]
fn each_import_budget_is_enforced() {
    let bytes = doc().to_json(1_000_000).unwrap();
    let caps = [
        TraceLimits {
            max_bytes: 1,
            ..Default::default()
        },
        TraceLimits {
            max_nodes: 1,
            ..Default::default()
        },
        TraceLimits {
            max_depth: 1,
            ..Default::default()
        },
        TraceLimits {
            max_events: 1,
            ..Default::default()
        },
        TraceLimits {
            max_checkpoints: 1,
            ..Default::default()
        },
        TraceLimits {
            max_checkpoint_units: 1,
            ..Default::default()
        },
        TraceLimits {
            max_replay_work: 1,
            ..Default::default()
        },
        TraceLimits {
            max_atoms_total: 1,
            ..Default::default()
        },
        TraceLimits {
            max_fields_total: 1,
            ..Default::default()
        },
        TraceLimits {
            max_bindings: 0,
            ..Default::default()
        },
        TraceLimits {
            max_conflicts: 0,
            ..Default::default()
        },
    ];
    for cap in caps {
        assert!(VerifiedTrace::import(&bytes, cap).is_err());
    }
    assert!(doc().to_json(1).is_err());
    let mut large = Graph::new();
    ctor(&mut large, &"x".repeat(2000));
    let e = Execution::new(large, Default::default()).unwrap();
    let log = Recording::new(&e, Default::default()).unwrap();
    let d = TraceDocument::from_recording(
        &log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    for cap in [
        TraceLimits {
            max_checkpoint_units: 1000,
            ..Default::default()
        },
        TraceLimits {
            max_replay_work: 1000,
            ..Default::default()
        },
    ] {
        assert!(
            VerifiedTrace::verify(d.clone(), cap).is_err(),
            "label bytes must be charged"
        );
    }
    let deep = format!("{}null{}", "[".repeat(1000), "]".repeat(1000));
    assert!(VerifiedTrace::import(deep.as_bytes(), Default::default()).is_err());
    let mut g = graph();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    let y = variable(&mut g, Sort::Constructor);
    let done = link(&mut g, "done", x, y);
    let pending = link(&mut g, "pending", x, y);
    sides(&mut g, h, &[x, y, done], &[x, y, pending]);
    let mut e = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&e, Default::default()).unwrap();
    let p = e.enabled().unwrap().into_steps().remove(0);
    log.commit(&mut e, p).unwrap();
    let p = e.enabled_for(h).unwrap().into_steps().remove(0);
    log.commit(&mut e, p).unwrap();
    let d = TraceDocument::from_recording(
        &log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(!d.dependencies.as_ref().unwrap().is_empty());
    assert!(VerifiedTrace::import(
        &d.to_json(1_000_000).unwrap(),
        TraceLimits {
            max_witnesses: 0,
            ..Default::default()
        }
    )
    .is_err());
    let mut forged = d;
    forged.dependencies.as_mut().unwrap().clear();
    assert!(VerifiedTrace::verify(forged, Default::default()).is_err());
}
#[test]
fn large_identities_gaps_cycles_forward_fields_and_exhausted_allocator() {
    let limits = GraphLimits::default();
    let base = (1u64 << 53) + 7;
    let atoms = vec![
        (base, Kind::Constructor("高".into()), vec![]),
        (
            base + 3,
            Kind::Record {
                role: Role::ObjectLink,
                label: Some("cycle".into()),
            },
            vec![base, base + 5],
        ),
        (base + 5, Kind::Constructor("B".into()), vec![]),
        (
            base + 6,
            Kind::Record {
                role: Role::ObjectLink,
                label: Some("cycle".into()),
            },
            vec![base + 5, base],
        ),
    ];
    for next in [base + 9, u64::MAX] {
        let graph = Graph::restore_identity_snapshot(&atoms, next, &limits).unwrap();
        let execution = Execution::new(graph.clone(), Default::default()).unwrap();
        let log = Recording::new(&execution, Default::default()).unwrap();
        let t = check(
            TraceDocument::from_recording(
                &log,
                &Observer::all(),
                None,
                Default::default(),
                Default::default(),
            )
            .unwrap(),
        );
        assert_eq!(
            GraphData::from_graph(&t.state_at(0).unwrap()),
            GraphData::from_graph(&graph)
        );
        let mut restored = t.state_at(0).unwrap();
        if next == u64::MAX {
            assert!(restored.insert(Kind::Nat, &[]).is_err());
        } else {
            assert_eq!(restored.insert(Kind::Nat, &[]).unwrap().number(), next);
        }
    }
    assert!(Graph::restore_identity_snapshot(&atoms, base + 5, &limits).is_err());
    let mut duplicate = atoms.clone();
    duplicate.push(atoms[0].clone());
    assert!(Graph::restore_identity_snapshot(&duplicate, u64::MAX, &limits).is_err());
    let mut broken = atoms.clone();
    broken[1].2[0] = 0;
    assert!(Graph::restore_identity_snapshot(&broken, u64::MAX, &limits).is_err());
    broken = atoms;
    broken[0].2.push(base);
    assert!(Graph::restore_identity_snapshot(&broken, u64::MAX, &limits).is_err());
}
#[test]
fn translated_runtime_identities_replay_without_printer_normalisation() {
    let original = graph();
    let offset = (1u64 << 53) + 99;
    let atoms = original
        .iter()
        .map(|(k, a)| {
            (
                k.number() + offset,
                a.kind.clone(),
                a.fields.iter().map(|k| k.number() + offset).collect(),
            )
        })
        .collect::<Vec<_>>();
    let shifted = Graph::restore_identity_snapshot(
        &atoms,
        original.next_identity() + offset,
        &GraphLimits::default(),
    )
    .unwrap();
    let mut r = runner(shifted);
    r.run(
        RunLimits {
            max_steps: 2,
            ..Default::default()
        },
        None,
        || false,
    )
    .unwrap();
    let t = check(
        TraceDocument::from_runner(&r, None, Default::default(), Default::default()).unwrap(),
    );
    assert!(t.document().events[0].redex.source.0 >= offset);
    assert_eq!(
        GraphData::from_graph(&t.state_at(2).unwrap()),
        GraphData::from_graph(r.execution().graph())
    );
}
#[test]
fn recorded_binding_mode_and_finished_empty_run_have_no_false_continuation() {
    let mut e = Execution::new(graph(), Default::default()).unwrap();
    let mut log = Recording::new(&e, Default::default()).unwrap();
    let candidate = e.enabled().unwrap().into_steps().pop().unwrap();
    log.commit(&mut e, candidate).unwrap();
    let d = TraceDocument::from_recording(
        &log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(d.status, "recorded_prefix");
    assert!(check(d).resume_at(1).is_err());
    let mut r = runner(Graph::new());
    let report = r
        .run(
            RunLimits {
                max_steps: 0,
                ..Default::default()
            },
            None,
            || false,
        )
        .unwrap();
    assert_eq!(report.outcome, Outcome::NormalForm);
    let t = check(
        TraceDocument::from_runner(&r, None, Default::default(), Default::default()).unwrap(),
    );
    assert!(t.state_at(0).unwrap().is_empty());
    assert!(t.state_at(1).is_err());
}

#[test]
fn maximum_atom_identity_and_declared_recording_limits_are_checked() {
    let g = Graph::restore_identity_snapshot(
        &[(u64::MAX - 1, Kind::Constructor("last".into()), vec![])],
        u64::MAX,
        &GraphLimits::default(),
    )
    .unwrap();
    let e = Execution::new(g, Default::default()).unwrap();
    let log = Recording::new(&e, Default::default()).unwrap();
    let d = TraceDocument::from_recording(
        &log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(check(d).state_at(0).unwrap().key(u64::MAX - 1).is_some());
    let mut d = doc();
    d.status = "recorded_prefix".into();
    d.settings.plan = None;
    d.report = None;
    d.checkpoints.clear();
    d.settings.recording.max_events = Number(0);
    assert!(VerifiedTrace::verify(d, Default::default()).is_err());
}
