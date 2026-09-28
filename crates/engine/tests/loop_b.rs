//! Independent checks for immutable-baseline reuse and exact observation search.
use engine::observation::Observer;
use engine::observation::{all, isomorphic, isomorphic_indexed};
use engine::runner::{ExecutionProfile, Outcome, RunLimits, RunProgress, Runner};
use engine::scheduling::{Scheduler, SchedulingStrategy};
use engine::trace::{TraceDocument, TraceLimits, TraceOptions, VerifiedTrace};
use engine::{apply_with_binding_with_limits, Execution, ExecutionLimits};
use graph_core::{Graph, Kind, NewAtom, Role, Target};

fn runner(source: &str, profile: ExecutionProfile, seed: Option<u64>) -> Runner<Scheduler> {
    let mut r = Runner::with_profile(
        parser::load(source).unwrap().into_graph(),
        ExecutionLimits::default(),
        seed.map_or(Scheduler::Deterministic, Scheduler::seeded),
        Observer::all(),
        profile,
    )
    .unwrap();
    r.enable_recording(Default::default()).unwrap();
    r
}

#[test]
fn profiled_runs_preserve_complete_set_selection_and_portable_continuation() {
    for source in [
        include_str!("../../../examples/practical/work-items.graph"),
        include_str!("../../../examples/practical/policy-enable.graph"),
        include_str!("../../../examples/programs/self-modify.graph"),
        include_str!("../../../examples/programs/restoration.graph"),
        include_str!("../../../examples/behaviour/oscillating.graph"),
    ] {
        for seed in [None, Some(0), Some(42)] {
            let limits = RunLimits {
                max_steps: 4,
                detect_cycles: true,
                ..Default::default()
            };
            let mut a = runner(source, ExecutionProfile::ReferenceV1, seed);
            let mut b = runner(source, ExecutionProfile::IndexedV2, seed);
            let expected = a.run(limits, None, || false).unwrap();
            b.begin_run(limits, None).unwrap();
            assert!(matches!(
                b.advance(1, || false).unwrap(),
                RunProgress::Yielded
            ));
            let doc = TraceDocument::from_runner(
                &b,
                None,
                TraceOptions::default(),
                TraceLimits::default(),
            )
            .unwrap();
            assert_eq!(
                doc.settings.execution_profile,
                Some(ExecutionProfile::IndexedV2)
            );
            let verified = VerifiedTrace::verify(doc, TraceLimits::default()).unwrap();
            let mut resumed = verified.resume_at(1).unwrap();
            assert_eq!(resumed.profile(), ExecutionProfile::IndexedV2);
            let actual = loop {
                if let RunProgress::Finished(report) = resumed.advance(1, || false).unwrap() {
                    break report;
                }
            };
            assert_eq!(expected.outcome, actual.outcome);
            assert_eq!(expected.scheduler, actual.scheduler);
            assert_eq!(
                a.recording().unwrap().events(),
                resumed.recording().unwrap().events()
            );
            assert_eq!(
                a.execution().graph().snapshot(),
                resumed.execution().graph().snapshot()
            );
            TraceDocument::from_runner(
                &resumed,
                None,
                TraceOptions::default(),
                TraceLimits::default(),
            )
            .unwrap();
        }
    }
}

#[test]
fn explicit_profiles_separate_build_provenance_from_replay_contract() {
    let mut r = runner(
        include_str!("../../../examples/behaviour/oscillating.graph"),
        ExecutionProfile::ReferenceV1,
        None,
    );
    r.run(
        RunLimits {
            max_steps: 1,
            ..Default::default()
        },
        None,
        || false,
    )
    .unwrap();
    let mut doc =
        TraceDocument::from_runner(&r, None, TraceOptions::default(), TraceLimits::default())
            .unwrap();
    doc.implementation = "historical-build-not-the-current-source".into();
    VerifiedTrace::verify(doc.clone(), TraceLimits::default()).unwrap();
    // Legacy traces have no algorithm declaration; their source guard survives.
    doc.settings.execution_profile = None;
    assert!(VerifiedTrace::verify(doc.clone(), TraceLimits::default()).is_err());
    let bytes = doc.to_json(1_000_000).unwrap();
    let migrated = VerifiedTrace::migrate_reference(&bytes, TraceLimits::default()).unwrap();
    assert_eq!(migrated.document().events, doc.events);
    assert_eq!(migrated.document().implementation, doc.implementation);
    let mut corrupt = doc.clone();
    corrupt.events.clear();
    assert!(VerifiedTrace::migrate_reference(
        &corrupt.to_json(1_000_000).unwrap(),
        TraceLimits::default()
    )
    .is_err());
    let duplicate = String::from_utf8(bytes).unwrap().replacen(
        "\"schema\":",
        "\"schema\": \"reflective-trace-v1\", \"schema\":",
        1,
    );
    assert!(
        VerifiedTrace::migrate_reference(duplicate.as_bytes(), TraceLimits::default()).is_err()
    );
}

#[test]
fn actual_legacy_build_histories_migrate_without_changing_recorded_evidence() {
    for text in [
        include_str!("../../../benchmarks/results/s6-loop-b-compatibility/normal.legacy.json"),
        include_str!("../../../benchmarks/results/s6-loop-b-compatibility/recurrence.legacy.json"),
        include_str!("../../../benchmarks/results/s6-loop-b-compatibility/reflective.legacy.json"),
        include_str!("../../../benchmarks/results/s6-loop-b-compatibility/exhausted.legacy.json"),
        include_str!("../../../benchmarks/results/s6-loop-b-compatibility/yielded.legacy.json"),
    ] {
        let original: TraceDocument = serde_json::from_str(text).unwrap();
        assert!(VerifiedTrace::import(text.as_bytes(), TraceLimits::default()).is_err());
        let migrated =
            VerifiedTrace::migrate_reference(text.as_bytes(), TraceLimits::default()).unwrap();
        assert_eq!(migrated.document().events, original.events);
        assert_eq!(migrated.document().checkpoints, original.checkpoints);
        assert_eq!(migrated.document().report, original.report);
        assert_eq!(migrated.document().initial, original.initial);
    }
}

#[test]
fn indexed_gate_and_exhaustion_leave_graph_and_rng_unpublished() {
    let source = include_str!("../../../examples/behaviour/oscillating.graph");
    let mut r = runner(source, ExecutionProfile::IndexedV2, Some(42));
    let before = r.execution().graph().snapshot();
    let scheduler = r.scheduler().description();
    r.begin_run(RunLimits::default(), None).unwrap();
    assert!(matches!(
        r.advance_guarded(1, || false, |_| false).unwrap(),
        RunProgress::Yielded
    ));
    assert_eq!(r.execution().graph().snapshot(), before);
    assert_eq!(r.scheduler().description(), scheduler);
    assert_eq!(r.retained_states(), 0);
    let mut limits = ExecutionLimits::default();
    limits.search.max_work = 0;
    let mut r = Runner::with_profile(
        parser::load(source).unwrap().into_graph(),
        limits,
        Scheduler::seeded(42),
        Observer::all(),
        ExecutionProfile::IndexedV2,
    )
    .unwrap();
    assert_eq!(
        r.run(RunLimits::default(), None, || false).unwrap().outcome,
        Outcome::ResourceLimit
    );
    assert_eq!(r.execution().committed_steps(), 0);
    assert_eq!(r.scheduler().description(), scheduler);
}

#[derive(Clone)]
struct Last;
impl SchedulingStrategy for Last {
    fn select(&mut self, steps: &[engine::PreparedStep]) -> Option<usize> {
        assert!(steps.iter().all(|s| !s.ground_rule().fresh().is_empty()));
        steps.len().checked_sub(1)
    }
    fn description(&self) -> String {
        "last-full-candidate-test".into()
    }
}
#[test]
fn custom_candidate_inspecting_strategy_receives_complete_candidates() {
    let graph = parser::load(include_str!("../../../examples/practical/work-items.graph"))
        .unwrap()
        .into_graph();
    for profile in [ExecutionProfile::IndexedV2, ExecutionProfile::FramedV3] {
        let mut a = Runner::with_profile(
            graph.clone(),
            ExecutionLimits::default(),
            Last,
            Observer::all(),
            ExecutionProfile::ReferenceV1,
        )
        .unwrap();
        let mut b = Runner::with_profile(
            graph.clone(),
            ExecutionLimits::default(),
            Last,
            Observer::all(),
            profile,
        )
        .unwrap();
        for r in [&mut a, &mut b] {
            r.run(
                RunLimits {
                    max_steps: 3,
                    ..Default::default()
                },
                None,
                || false,
            )
            .unwrap();
        }
        assert_eq!(
            a.execution().graph().snapshot(),
            b.execution().graph().snapshot()
        );
    }
}

#[test]
fn cached_admission_agrees_with_independently_revalidated_baselines() {
    for source in [
        include_str!("../../../examples/programs/self-modify.graph"),
        include_str!("../../../examples/programs/restoration.graph"),
        include_str!("../../../examples/programs/arithmetic.graph"),
        include_str!("../../../examples/behaviour/oscillating.graph"),
    ] {
        let mut run = Execution::new(
            parser::load(source).unwrap().into_graph(),
            ExecutionLimits::default(),
        )
        .unwrap();
        for _ in 0..8 {
            let heads: Vec<_> = run.view().unwrap().live_rules().map(|r| r.head).collect();
            let mut chosen = None;
            for head in heads {
                for binding in run.bindings(head).unwrap() {
                    let mut reference = run.graph().clone();
                    let expected = apply_with_binding_with_limits(
                        &mut reference,
                        head,
                        binding.assignments(),
                        &run.limits().graph,
                    );
                    match (expected, run.prepare(binding)) {
                        (Ok(event), Ok(step)) => {
                            assert_eq!(&event, step.event());
                            if chosen.is_none() {
                                chosen = Some((step, reference));
                            }
                        }
                        (Err(a), Err(b)) => {
                            assert_eq!(a.kind(), b.kind());
                            assert_eq!(a.to_string(), b.to_string());
                        }
                        (a, b) => panic!("admission mismatch: {a:?} / {b:?}"),
                    }
                }
            }
            let Some((step, expected)) = chosen else {
                break;
            };
            run.commit(step).unwrap();
            assert_eq!(run.graph().snapshot(), expected.snapshot());
            assert_eq!(run.graph().next_identity(), expected.next_identity());
        }
    }
}

// Functional directed graphs represented by constructors and ordered links.
// No rules, matching, colours or production isomorphism code in the oracle.
fn graph(targets: &[usize], offset: usize) -> Graph {
    let mut g = Graph::new();
    let removed: Vec<_> = (0..offset)
        .map(|_| g.insert(Kind::Nat, &[]).unwrap())
        .collect();
    g.remove_set(&removed.into_iter().collect()).unwrap();
    let mut batch: Vec<_> = targets
        .iter()
        .map(|_| NewAtom {
            kind: Kind::Constructor("vertex".into()),
            fields: vec![],
        })
        .collect();
    batch.extend(targets.iter().enumerate().map(|(i, target)| NewAtom {
        kind: Kind::Record {
            role: Role::ObjectLink,
            label: Some("edge".into()),
        },
        fields: vec![Target::Fresh(*target), Target::Fresh(i)],
    }));
    g.insert_batch(&batch).unwrap();
    g
}
fn oracle(a: &[usize], b: &[usize]) -> bool {
    fn visit(a: &[usize], b: &[usize], permutation: &mut Vec<usize>) -> bool {
        if permutation.len() == a.len() {
            return (0..a.len()).all(|i| permutation[a[i]] == b[permutation[i]]);
        }
        for j in 0..a.len() {
            if !permutation.contains(&j) {
                permutation.push(j);
                if visit(a, b, permutation) {
                    return true;
                }
                permutation.pop();
            }
        }
        false
    }
    visit(a, b, &mut Vec::new())
}
#[test]
fn propagated_search_matches_exhaustive_permutation_oracle() {
    let cases: Vec<Vec<usize>> = (0..27)
        .map(|mut code| {
            (0..3)
                .map(|_| {
                    let d = code % 3;
                    code /= 3;
                    d
                })
                .collect()
        })
        .collect();
    for a in &cases {
        for b in &cases {
            let left = all(&graph(a, 0), Default::default(), Default::default()).unwrap();
            let right = all(&graph(b, 7), Default::default(), Default::default()).unwrap();
            let expected = oracle(a, b);
            assert_eq!(
                isomorphic_indexed(&left, &right, 1_000_000).unwrap(),
                expected,
                "{a:?} / {b:?}"
            );
            assert_eq!(isomorphic(&left, &right, 1_000_000).unwrap(), expected);
        }
    }
}
#[test]
fn identical_colours_do_not_certify_isomorphism_and_keys_are_not_equality() {
    let six_cycle = [1, 2, 3, 4, 5, 0];
    let two_triangles = [1, 2, 0, 4, 5, 3];
    let a = all(
        &graph(&six_cycle, 0),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let b = all(
        &graph(&two_triangles, 0),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(!isomorphic_indexed(&a, &b, 100_000).unwrap());
    let relabelled = all(
        &graph(&six_cycle, 10),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(isomorphic_indexed(&a, &relabelled, 100_000).unwrap());
    assert!(isomorphic_indexed(&a, &a, 0).is_err());
}
#[test]
fn verified_identity_witness_avoids_quadratic_self_comparison() {
    let g = graph(&(0..1000).collect::<Vec<_>>(), 0);
    let a = all(&g, Default::default(), Default::default()).unwrap();
    assert!(isomorphic_indexed(&a, &a, 50_000).unwrap());
    assert!(isomorphic(&a, &a, 50_000).is_err());
}

#[test]
fn negative_signature_screen_is_useful_but_never_an_equality_certificate() {
    // Research counterfactual, NOT the production profile's work contract.
    // Canonical full kind serialization avoids hashing or identity assumptions.
    fn signature(
        o: &engine::observation::Observation,
    ) -> std::collections::BTreeMap<String, usize> {
        let mut result = std::collections::BTreeMap::new();
        for atom in o.atoms().values() {
            let key = serde_json::to_string(&(&atom.kind, atom.fields.len())).unwrap();
            *result.entry(key).or_insert(0) += 1;
        }
        result
    }
    let mut run = Execution::new(
        parser::load(include_str!("../../../examples/practical/work-items.graph"))
            .unwrap()
            .into_graph(),
        ExecutionLimits::default(),
    )
    .unwrap();
    let mut states = Vec::new();
    loop {
        states.push(all(run.graph(), Default::default(), Default::default()).unwrap());
        let Some(next) = run.enabled().unwrap().into_steps().into_iter().next() else {
            break;
        };
        run.commit(next).unwrap();
    }
    assert_eq!(states.len(), 11);
    for i in 0..states.len() {
        for j in 0..i {
            assert_ne!(signature(&states[i]), signature(&states[j]));
            assert!(!isomorphic_indexed(&states[i], &states[j], 1_000_000).unwrap());
        }
    }
    let cycle = all(
        &graph(&[1, 2, 3, 4, 5, 0], 0),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let triangles = all(
        &graph(&[1, 2, 0, 4, 5, 3], 0),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(signature(&cycle), signature(&triangles));
    assert!(!isomorphic_indexed(&cycle, &triangles, 100_000).unwrap());
}
