use engine::scheduling::{candidate_key, Scheduler, SchedulingStrategy};
use engine::{Execution, ExecutionLimits, SearchLimits};
use graph_core::{ErrorKind, Graph};

fn limits() -> ExecutionLimits {
    ExecutionLimits {
        search: SearchLimits {
            max_candidates: 100_000,
            max_work: 100_000_000,
        },
        ..Default::default()
    }
}
fn compare(graph: Graph, seed: Option<u64>, rounds: usize) {
    let mut reference = Execution::new(graph.clone(), limits()).unwrap();
    let mut indexed = Execution::new(graph, limits()).unwrap();
    let mut a = seed
        .map(Scheduler::seeded)
        .unwrap_or(Scheduler::Deterministic);
    let mut b = a.clone();
    for round in 0..rounds {
        let old = reference.enabled().unwrap();
        let compact = indexed.enabled_compact().unwrap();
        let mut old_rejected: Vec<_> = old
            .rejected()
            .iter()
            .map(|r| format!("{:?}:{:?}", r.binding, r.diagnostic.kind()))
            .collect();
        let mut new_rejected: Vec<_> = compact
            .rejected()
            .iter()
            .map(|r| format!("{:?}:{:?}", r.binding, r.diagnostic.kind()))
            .collect();
        old_rejected.sort();
        new_rejected.sort();
        assert_eq!(old_rejected, new_rejected);
        let mut steps = old.into_steps();
        steps.sort_by_key(candidate_key);
        assert_eq!(
            steps.iter().map(|s| s.binding()).collect::<Vec<_>>(),
            compact.bindings().iter().collect::<Vec<_>>()
        );
        let selected = a.select(&steps);
        let new = compact.select(&b).unwrap();
        match (selected, new) {
            (Some(i), Some((step, scheduler))) => {
                assert_eq!(
                    reference.commit(steps.remove(i)).unwrap(),
                    indexed.commit(step).unwrap()
                );
                b = scheduler;
                assert_eq!(a.description(), b.description());
                assert_eq!(reference.graph().snapshot(), indexed.graph().snapshot());
                assert_eq!(
                    reference.graph().next_identity(),
                    indexed.graph().next_identity()
                );
            }
            (None, None) => break,
            _ => panic!("selection mismatch"),
        }
        if round % 2 == 0 {
            // Even identical external replacement starts a new baseline. All
            // indexes must be reconstructed, never retained across this boundary.
            reference.replace_graph(reference.graph().clone()).unwrap();
            indexed.replace_graph(indexed.graph().clone()).unwrap();
        }
    }
}
#[test]
fn lifecycle_revisions_quotation_and_seeded_selection_match_reference() {
    for text in [
        include_str!("../../../examples/fixtures/a2.atoms"),
        include_str!("../../../examples/fixtures/a4.atoms"),
        include_str!("../../../examples/fixtures/b2.atoms"),
        include_str!("../../../examples/fixtures/c1.atoms"),
    ] {
        for seed in [None, Some(0), Some(42)] {
            compare(graph_core::fixture::load(text).unwrap().graph, seed, 4);
        }
    }
    for text in [
        include_str!("../../../examples/programs/self-modify.graph"),
        include_str!("../../../examples/programs/restoration.graph"),
        include_str!("../../../examples/programs/arithmetic.graph"),
        include_str!("../../../examples/behaviour/oscillating.graph"),
    ] {
        for seed in [None, Some(42)] {
            compare(parser::load(text).unwrap().into_graph(), seed, 8);
        }
    }
}
#[test]
fn resource_failure_does_not_publish_partial_candidates_or_modify_state() {
    let graph = parser::load(include_str!("../../../examples/programs/arithmetic.graph"))
        .unwrap()
        .into_graph();
    let mut limits = limits();
    limits.search.max_candidates = 0;
    let run = Execution::new(graph.clone(), limits).unwrap();
    assert_eq!(
        run.enabled_compact().unwrap_err().kind(),
        ErrorKind::ResourceExhaustion
    );
    assert_eq!(run.graph().snapshot(), graph.snapshot());
    assert_eq!(run.committed_steps(), 0);
}
