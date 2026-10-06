use engine::versioned::{self, Implementation as I, Request, View};
use engine::{Execution, ExecutionLimits};
use graph_core::{ErrorKind, Graph, Kind, Role};
use std::collections::BTreeSet;

fn request(previous: I, replacement: I, budget: u8, payload: &str) -> Request {
    Request {
        previous,
        replacement,
        budget,
        payload: payload.into(),
    }
}
fn projected(v: &View) -> Vec<(String, String, String)> {
    let mut xs: Vec<_> = v
        .calls
        .iter()
        .map(|c| {
            (
                c.status.clone(),
                c.version.identity.clone(),
                c.payload_identity.clone(),
            )
        })
        .collect();
    xs.sort();
    xs
}
fn explore(g: Graph, depth: usize, paths: &mut usize, transitions: &mut usize) {
    assert!(depth <= 6);
    let before = versioned::inspect(&g).unwrap();
    let e = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
    let enabled = e.enabled().unwrap().into_steps();
    if enabled.is_empty() {
        assert_eq!(before.calls.len(), 2);
        assert!(before.calls.iter().all(|c| c.status == "done"));
        assert!(!before.spawn_pending);
        if before.publication_pending {
            let target = before
                .versions
                .iter()
                .find(|v| v.identity != before.active.identity)
                .unwrap();
            assert!(target.evaluation_steps > before.budget);
        }
        *paths += 1;
        return;
    }
    for step in enabled {
        let mut child = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
        let prepared = child.prepare(step.binding().clone()).unwrap();
        let stale = child.prepare(step.binding().clone()).unwrap();
        let mut reference = g.clone();
        let reference_event = engine::apply_with_binding(
            &mut reference,
            step.binding().source(),
            step.binding().assignments(),
        )
        .unwrap();
        let event = child.commit(prepared).unwrap();
        assert_eq!(event, reference_event);
        assert_eq!(child.graph().snapshot(), reference.snapshot());
        let after_snapshot = child.graph().snapshot();
        let next = child.graph().next_identity();
        assert_eq!(
            child.commit(stale).unwrap_err().kind(),
            ErrorKind::StaleCandidate
        );
        assert_eq!(child.graph().snapshot(), after_snapshot);
        assert_eq!(child.graph().next_identity(), next);
        let after = versioned::inspect(child.graph()).unwrap();
        assert_eq!(after.versions, before.versions);
        assert_eq!(after.budget, before.budget);
        for (k, atom) in g.iter().filter(|(k, _)| !event.consumed.contains(k)) {
            assert_eq!(child.graph().atom(k).unwrap(), atom);
        }
        let consumed: BTreeSet<_> = event
            .consumed
            .iter()
            .map(|k| k.number().to_string())
            .collect();
        let mut expected = projected(&before);
        if before.publication_pending && !after.publication_pending {
            let candidate = before
                .versions
                .iter()
                .find(|v| v.identity != before.active.identity)
                .unwrap();
            assert_eq!(&after.active, candidate); // Includes the actual installed body and cost.
            assert!(after.active.evaluation_steps <= before.budget);
        } else {
            assert_eq!(after.active, before.active);
            if before.spawn_pending && !after.spawn_pending {
                expected.push((
                    "call".into(),
                    before.active.identity.clone(),
                    before.calls[0].payload_identity.clone(),
                ));
            } else {
                let old = before
                    .calls
                    .iter()
                    .find(|c| consumed.contains(&c.identity))
                    .unwrap();
                let new_status = match (old.status.as_str(), old.version.implementation) {
                    ("call", I::Return) | ("pending", I::DelayReturn) => "done",
                    ("call", I::DelayReturn) => "pending",
                    _ => panic!("invalid guest transition"),
                };
                let index = expected
                    .iter()
                    .position(|x| {
                        *x == (
                            old.status.clone(),
                            old.version.identity.clone(),
                            old.payload_identity.clone(),
                        )
                    })
                    .unwrap();
                expected.remove(index);
                expected.push((
                    new_status.into(),
                    old.version.identity.clone(),
                    old.payload_identity.clone(),
                ));
            }
        }
        expected.sort();
        assert_eq!(projected(&after), expected);
        *transitions += 1;
        explore(child.graph().clone(), depth + 1, paths, transitions);
    }
}

#[test]
fn all_schedules_preserve_readback_and_installed_contract() {
    let mut paths = 0;
    let mut transitions = 0;
    for previous in [I::Return, I::DelayReturn] {
        for replacement in [I::Return, I::DelayReturn] {
            for budget in [1, 2] {
                for payload in ["-7", "0", "11", "item λ"] {
                    let graph =
                        versioned::compile(&request(previous, replacement, budget, payload))
                            .unwrap();
                    explore(graph, 0, &mut paths, &mut transitions);
                }
            }
        }
    }
    println!("versioned closed fixtures: 32; terminal paths: {paths}; transition tree edges: {transitions}");
    assert_eq!(paths, 880);
    assert_eq!(transitions, 2684);
}

#[test]
fn invalid_requests_and_exhaustion_are_explicit() {
    for (budget, payload) in [(0, "x"), (3, "x"), (1, "")] {
        assert!(versioned::compile(&request(I::Return, I::Return, budget, payload)).is_err());
    }
    assert!(versioned::compile(&request(I::Return, I::Return, 1, &"x".repeat(257))).is_err());
    let g = versioned::compile(&request(I::Return, I::DelayReturn, 2, "x")).unwrap();
    let mut limits = ExecutionLimits::default();
    limits.search.max_work = 0;
    let e = Execution::new(g, limits).unwrap();
    let before = e.graph().snapshot();
    let next = e.graph().next_identity();
    assert_eq!(
        e.enabled().unwrap_err().kind(),
        ErrorKind::ResourceExhaustion
    );
    assert_eq!(before, e.graph().snapshot());
    assert_eq!(next, e.graph().next_identity());
}

#[test]
fn installed_body_readback_detects_fault_injection() {
    let mut g = versioned::compile(&request(I::DelayReturn, I::Return, 1, "x")).unwrap();
    let cfg = graph_core::validate(&g).unwrap();
    let quoted: BTreeSet<_> = cfg
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    let (record, fields) = g
        .iter()
        .find_map(|(k, a)| {
            if !quoted.contains(&k)
                && a.kind
                    == (Kind::Record {
                        role: Role::ObjectLink,
                        label: Some("body".into()),
                    })
            {
                Some((k, a.fields.clone()))
            } else {
                None
            }
        })
        .unwrap();
    // Replace an immutable description link through a construction boundary.
    // This is deliberately not an admitted execution event.
    g.remove_set(&BTreeSet::from([record])).unwrap();
    let wrong = g
        .insert(Kind::Constructor("versioned.Constant999".into()), &[])
        .unwrap();
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some("body".into()),
        },
        &[fields[0], wrong],
    )
    .unwrap();
    assert!(versioned::inspect(&g)
        .unwrap_err()
        .to_string()
        .contains("unsupported installed"));
}

#[test]
fn equivalent_over_budget_installed_body_is_not_the_checked_candidate() {
    let g = versioned::compile(&request(I::DelayReturn, I::Return, 1, "x")).unwrap();
    let mut e = Execution::new(g, ExecutionLimits::default()).unwrap();
    let first = e.enabled().unwrap().into_steps().remove(0);
    e.commit(first).unwrap();
    let accepted = versioned::inspect(e.graph()).unwrap();
    assert_eq!(accepted.active.implementation, I::Return);
    let mut g = e.graph().clone();
    let (record, owner) = g
        .iter()
        .find_map(|(k, a)| match &a.kind {
            Kind::Record {
                role: Role::ObjectLink,
                label: Some(s),
            } if s == "body" && a.fields[0].number().to_string() == accepted.active.identity => {
                Some((k, a.fields[0]))
            }
            _ => None,
        })
        .unwrap();
    g.remove_set(&BTreeSet::from([record])).unwrap();
    let wrong = g
        .insert(Kind::Constructor("versioned.DelayReturn".into()), &[])
        .unwrap();
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some("body".into()),
        },
        &[owner, wrong],
    )
    .unwrap();
    let observed = versioned::inspect(&g).unwrap();
    assert_eq!(observed.active.identity, accepted.active.identity);
    assert_ne!(observed.active, accepted.active);
    assert!(observed.active.evaluation_steps > observed.budget);
}

#[test]
fn portable_history_resumes_with_captured_versions() {
    use engine::observation::Observer;
    use engine::runner::{RunLimits, RunProgress, Runner};
    use engine::scheduling::Scheduler;
    use engine::trace::{TraceDocument, TraceLimits, TraceOptions, VerifiedTrace};
    let g = versioned::compile(&request(I::DelayReturn, I::Return, 1, "x")).unwrap();
    let mut runner = Runner::new(
        g,
        ExecutionLimits::default(),
        Scheduler::Deterministic,
        Observer::all(),
    )
    .unwrap();
    runner.enable_recording(Default::default()).unwrap();
    runner
        .begin_run(
            RunLimits {
                max_steps: 8,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    assert!(matches!(
        runner.advance(2, || false).unwrap(),
        RunProgress::Yielded
    ));
    let doc = TraceDocument::from_runner(
        &runner,
        None,
        TraceOptions::default(),
        TraceLimits::default(),
    )
    .unwrap();
    let verified = VerifiedTrace::verify(doc, TraceLimits::default()).unwrap();
    let mut resumed = verified.resume_at(2).unwrap();
    runner.advance(8, || false).unwrap();
    resumed.advance(8, || false).unwrap();
    assert_eq!(
        runner.execution().graph().snapshot(),
        resumed.execution().graph().snapshot()
    );
    assert_eq!(
        runner.recording().unwrap().events(),
        resumed.recording().unwrap().events()
    );
    let view = versioned::inspect(runner.execution().graph()).unwrap();
    assert_eq!(view.active.implementation, I::Return);
    assert!(view
        .calls
        .iter()
        .all(|c| c.status == "done" && c.payload == "x"));
    assert_ne!(
        view.calls[0].version.identity,
        view.calls[1].version.identity
    );
}
