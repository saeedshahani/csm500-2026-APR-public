//! Independent set-intersection and petgraph oracles; no shared extractor helpers.
#[allow(dead_code)]
mod support;
use engine::causal::{AnalysisLimits, DependencyKind as K, DependencyOrder, Edges, Witness};
use engine::recording::{
    AtomId, EventId, Footprint, RecordedEvent, Recording, RecordingLimits, RedexId,
};
use engine::runner::{Outcome, RunLimits, Runner};
use engine::scheduling::{Scheduler, SchedulingStrategy};
use engine::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{fixture, ErrorKind, Graph, Role, Sort};
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use support::*;

fn oracle(records: &[RecordedEvent]) -> Edges {
    let mut edges: Edges = BTreeMap::new();
    for (j, b) in records.iter().enumerate() {
        for a in &records[..j] {
            for (x, y, k) in [
                (&a.footprint.produced, &b.footprint.read, K::ProduceRead),
                (
                    &a.footprint.produced,
                    &b.footprint.consumed,
                    K::ProduceConsume,
                ),
                (&a.footprint.read, &b.footprint.consumed, K::ReadConsume),
            ] {
                for atom in x.intersection(y) {
                    let set = edges.entry((a.id, b.id)).or_default();
                    set.insert(Witness {
                        kind: k,
                        atom: *atom,
                    });
                    if k == K::ProduceRead && *atom == b.redex.revision {
                        set.insert(Witness {
                            kind: K::ExactRevision,
                            atom: *atom,
                        });
                    }
                }
            }
        }
    }
    edges
}
fn compare(records: &[RecordedEvent]) {
    let order = DependencyOrder::extract(records, AnalysisLimits::default()).unwrap();
    assert_eq!(order.edges(), &oracle(records));
    assert_eq!(order.relation(), "positive_resource_order");
    assert!(order.limitation().contains("absent edges"));
    let closure = order.closure(AnalysisLimits::default()).unwrap();
    let mut independent = petgraph::graph::DiGraph::<(), ()>::new();
    let nodes: Vec<_> = records.iter().map(|_| independent.add_node(())).collect();
    for &(a, b) in order.edges().keys() {
        independent.add_edge(nodes[a.0 as usize], nodes[b.0 as usize], ());
    }
    for a in 0..records.len() {
        for b in 0..records.len() {
            assert_eq!(
                closure.contains(&(EventId(a as u64), EventId(b as u64))),
                a != b
                    && petgraph::algo::has_path_connecting(&independent, nodes[a], nodes[b], None)
            );
        }
    }
    assert!(closure.iter().all(|(a, b)| a < b));
    for &(a, b) in &closure {
        for &(c, d) in &closure {
            if b == c {
                assert!(closure.contains(&(a, d)));
            }
        }
    }
}
fn synthetic(i: u64, produced: &[u64], read: &[u64], consumed: &[u64]) -> RecordedEvent {
    let ids = |s: &[u64]| s.iter().copied().map(AtomId).collect();
    RecordedEvent {
        id: EventId(i),
        post_state: i + 1,
        redex: RedexId {
            state: i,
            source: AtomId(900),
            revision: AtomId(901),
            binding: vec![],
        },
        footprint: Footprint {
            produced: ids(produced),
            read: ids(read),
            consumed: ids(consumed),
            preserved: BTreeSet::new(),
            entered: BTreeSet::new(),
            exited: BTreeSet::new(),
            altered: BTreeSet::new(),
        },
        fresh: BTreeMap::new(),
    }
}
#[test]
fn hand_derived_diamond_chain_unrelated_and_shared_read() {
    compare(&[]);
    compare(&[synthetic(0, &[], &[1], &[])]);
    let records = vec![
        synthetic(0, &[1], &[], &[]),
        synthetic(1, &[2], &[1, 99], &[]),
        synthetic(2, &[3], &[1, 99], &[]),
        synthetic(3, &[], &[2, 3], &[]),
        synthetic(4, &[], &[99], &[]),
        synthetic(5, &[], &[99], &[99]),
    ];
    compare(&records);
    let order = DependencyOrder::extract(&records, AnalysisLimits::default()).unwrap();
    let endpoints: BTreeSet<_> = order.edges().keys().map(|(a, b)| (a.0, b.0)).collect();
    assert_eq!(
        endpoints,
        BTreeSet::from([(0, 1), (0, 2), (1, 3), (2, 3), (1, 5), (2, 5), (4, 5)])
    );
    assert_eq!(
        order
            .ancestors(EventId(3), AnalysisLimits::default())
            .unwrap(),
        BTreeSet::from([EventId(0), EventId(1), EventId(2)])
    );
    assert_eq!(
        order
            .descendants(EventId(0), AnalysisLimits::default())
            .unwrap()
            .len(),
        4
    );
    assert!(order
        .ancestors(EventId(8), AnalysisLimits::default())
        .is_err());
}
proptest! {
    #![proptest_config(ProptestConfig { cases: std::env::var("PROPTEST_CASES").map(|v| v.parse().expect("numeric PROPTEST_CASES")).unwrap_or(64), ..ProptestConfig::default() })]
    #[test]
    fn generated_legal_resource_histories_agree_with_both_oracles(ops in prop::collection::vec((any::<bool>(),0usize..8,any::<bool>()),0..24)) {
        let mut live=vec![100u64,101,102]; let mut next=103; let mut records=vec![];
        for (produce,choice,consume) in ops {
            let selected=(!live.is_empty()).then(||live[choice%live.len()]);
            let reads:Vec<_>=selected.into_iter().collect();
            let consumed=if consume { reads.clone() } else { vec![] };
            live.retain(|id| !consumed.contains(id));
            let produced=if produce {let id=next;next+=1;live.push(id);vec![id]} else {vec![]};
            records.push(synthetic(records.len() as u64,&produced,&reads,&consumed));
        }
        compare(&records);
    }
}
#[test]
fn extraction_and_closure_limits_never_publish_partial_results() {
    let records = vec![synthetic(0, &[1], &[], &[]), synthetic(1, &[], &[1], &[])];
    for l in [
        AnalysisLimits {
            max_work: 0,
            ..AnalysisLimits::default()
        },
        AnalysisLimits {
            max_units: 0,
            ..AnalysisLimits::default()
        },
    ] {
        assert_eq!(
            DependencyOrder::extract(&records, l).unwrap_err().kind(),
            ErrorKind::ResourceExhaustion
        );
        let order = DependencyOrder::extract(&records, AnalysisLimits::default()).unwrap();
        assert_eq!(
            order.closure(l).unwrap_err().kind(),
            ErrorKind::ResourceExhaustion
        );
    }
    let zero = AnalysisLimits {
        max_work: 0,
        max_units: 0,
    };
    assert!(DependencyOrder::extract(&[], zero)
        .unwrap()
        .closure(zero)
        .unwrap()
        .is_empty());
    let mut bad = records;
    bad[1].id = EventId(5);
    assert!(DependencyOrder::extract(&bad, AnalysisLimits::default()).is_err());
}

#[test]
fn exact_recording_and_analysis_storage_boundaries() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let mut e = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
    let mut log = Recording::new(&e, RecordingLimits::default()).unwrap();
    let p = e
        .prepare(OperationalBinding::new(h, BTreeMap::new()))
        .unwrap();
    log.commit(&mut e, p).unwrap();
    let exact = log.units();
    for units in [exact - 1, exact] {
        let mut e = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
        let mut log = Recording::new(
            &e,
            RecordingLimits {
                max_units: units,
                ..RecordingLimits::default()
            },
        )
        .unwrap();
        let p = e
            .prepare(OperationalBinding::new(h, BTreeMap::new()))
            .unwrap();
        assert_eq!(log.commit(&mut e, p).is_ok(), units == exact);
        assert_eq!(e.committed_steps(), u64::from(units == exact));
        assert_eq!(log.events().len(), usize::from(units == exact));
    }
    let records = vec![synthetic(0, &[1], &[], &[]), synthetic(1, &[], &[1], &[])];
    for work in [true, false] {
        let succeeds = |n| {
            DependencyOrder::extract(
                &records,
                if work {
                    AnalysisLimits {
                        max_work: n,
                        ..AnalysisLimits::default()
                    }
                } else {
                    AnalysisLimits {
                        max_units: n,
                        ..AnalysisLimits::default()
                    }
                },
            )
            .is_ok()
        };
        let first = (0..100).find(|&n| succeeds(n)).unwrap();
        assert!(first > 0);
        assert!(!succeeds(first - 1));
        assert!(succeeds(first));
    }
}
fn run(g: Graph) -> Runner<Scheduler> {
    Runner::new(
        g,
        ExecutionLimits::default(),
        Scheduler::seeded(7),
        engine::observation::Observer::all(),
    )
    .unwrap()
}
#[test]
fn runner_records_noops_without_ordering_shared_program_reads() {
    let mut g = Graph::new();
    rule(&mut g, 0, false, true);
    let mut r = run(g);
    r.enable_recording(RecordingLimits::default()).unwrap();
    assert_eq!(
        r.run(
            RunLimits {
                max_steps: 3,
                ..RunLimits::default()
            },
            None,
            || false
        )
        .unwrap()
        .outcome,
        Outcome::StepLimit
    );
    let log = r.recording().unwrap();
    assert_eq!(log.events().len(), 3);
    compare(log.events());
    assert!(
        DependencyOrder::extract(log.events(), AnalysisLimits::default())
            .unwrap()
            .edges()
            .is_empty()
    );
    assert_eq!(log.events()[2].redex.state, 2);
}
#[test]
fn recording_capacity_cancellation_replacement_and_rng_are_atomic() {
    let mut g = Graph::new();
    rule(&mut g, 0, false, true);
    let mut r = run(g.clone());
    r.enable_recording(RecordingLimits {
        max_events: 0,
        ..RecordingLimits::default()
    })
    .unwrap();
    let initial = r.execution().graph().snapshot();
    let rng = r.scheduler().description();
    assert_eq!(
        r.run(RunLimits::default(), None, || false).unwrap().outcome,
        Outcome::ResourceLimit
    );
    assert_eq!(r.execution().graph().snapshot(), initial);
    assert_eq!(r.scheduler().description(), rng);
    assert!(r.recording().unwrap().events().is_empty());
    let mut r = run(g.clone());
    r.enable_recording(RecordingLimits::default()).unwrap();
    let rng = r.scheduler().description();
    let mut calls = 0;
    assert_eq!(
        r.run(RunLimits::default(), None, || {
            calls += 1;
            calls == 3
        })
        .unwrap()
        .outcome,
        Outcome::Cancelled
    );
    assert!(r.recording().unwrap().events().is_empty());
    assert_eq!(r.scheduler().description(), rng);
    r.run(
        RunLimits {
            max_steps: 1,
            ..RunLimits::default()
        },
        None,
        || false,
    )
    .unwrap();
    r.replace_graph(g.clone(), engine::observation::Observer::all())
        .unwrap();
    assert!(r.recording().unwrap().events().is_empty());
    assert_eq!(r.scheduler().description(), rng);
    let e = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
    let units = Recording::new(&e, RecordingLimits::default())
        .unwrap()
        .units();
    let mut r = run(g);
    r.enable_recording(RecordingLimits {
        max_units: units,
        ..RecordingLimits::default()
    })
    .unwrap();
    let mut too_large = r.execution().graph().clone();
    ctor(&mut too_large, "extra");
    assert_eq!(
        r.replace_graph(too_large, engine::observation::Observer::all())
            .unwrap_err()
            .kind(),
        ErrorKind::ResourceExhaustion
    );
    assert_eq!(r.execution().graph().snapshot(), initial);
    assert_eq!(
        r.run(RunLimits::default(), None, || false).unwrap().outcome,
        Outcome::ResourceLimit
    );
    assert!(r.recording().unwrap().events().is_empty());
}
#[test]
fn stale_prepared_steps_and_external_edits_do_not_append_events() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let mut e = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
    let mut log = Recording::new(&e, RecordingLimits::default()).unwrap();
    let other = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
    let binding = OperationalBinding::new(h, BTreeMap::new());
    let p = other.prepare(binding.clone()).unwrap();
    assert_eq!(
        log.commit(&mut e, p).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert!(log.events().is_empty());
    let candidate = e.prepare(binding.clone()).unwrap();
    let stale = e.prepare(binding.clone()).unwrap();
    log.commit(&mut e, candidate).unwrap();
    assert_eq!(
        log.commit(&mut e, stale).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert_eq!(log.events().len(), 1);
    e.replace_graph(g).unwrap();
    let p = e.prepare(binding).unwrap();
    assert_eq!(
        log.commit(&mut e, p).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert_eq!(log.events().len(), 1);
}
#[test]
fn consumed_payload_and_completed_support_remain_inspectable() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let v = variable(&mut g, Sort::Record(Role::ObjectLink));
    sides(&mut g, h, &[v], &[]);
    let a = ctor(&mut g, "α");
    let b = ctor(&mut g, "β");
    let edge = link(&mut g, "old", a, b);
    let representation = graph_core::validate(&g).unwrap().rules[&h]
        .representation
        .clone();
    let mut e = Execution::new(g, ExecutionLimits::default()).unwrap();
    let mut log = Recording::new(&e, RecordingLimits::default()).unwrap();
    let p = e
        .prepare(OperationalBinding::new(h, BTreeMap::from([(v, edge)])))
        .unwrap();
    log.commit(&mut e, p).unwrap();
    assert!(e.graph().get(edge).is_none());
    assert_eq!(
        log.atom(edge.into()).unwrap().fields,
        vec![a.into(), b.into()]
    );
    let event = &log.events()[0];
    assert_eq!(
        event.footprint.preserved,
        BTreeSet::from([a.into(), b.into()])
    );
    assert!(representation
        .iter()
        .all(|k| event.footprint.read.contains(&(*k).into())));
    assert!(event.footprint.read.contains(&edge.into()));
    compare(log.events());
}
#[test]
fn self_rewrite_has_exact_revision_links_and_read_consume_overlap() {
    let f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let mut r = run(f.graph);
    r.enable_recording(RecordingLimits::default()).unwrap();
    r.run(
        RunLimits {
            max_steps: 3,
            ..RunLimits::default()
        },
        None,
        || false,
    )
    .unwrap();
    let records = r.recording().unwrap().events();
    assert!(!records.is_empty());
    assert!(!records[0]
        .footprint
        .read
        .is_disjoint(&records[0].footprint.consumed));
    compare(records);
    let f = fixture::load(include_str!("../../../examples/fixtures/c1.atoms")).unwrap();
    let source = f.names["p"];
    let mut rejected = run(f.graph);
    rejected
        .enable_recording(RecordingLimits::default())
        .unwrap();
    let before = rejected.execution().graph().snapshot();
    let report = rejected
        .run(RunLimits::default(), Some(source), || false)
        .unwrap();
    assert_eq!(report.outcome, Outcome::RequestedRuleUnavailable);
    assert!(report.rejected > 0);
    assert!(rejected.recording().unwrap().events().is_empty());
    assert_eq!(rejected.execution().graph().snapshot(), before);
}
