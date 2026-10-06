#[allow(dead_code)]
mod support;
use engine::causal::provenance::{analyse, Label, Selection};
use engine::causal::{AnalysisLimits, DependencyKind, DependencyOrder};
use engine::observation::Observer;
use engine::recording::{EventId, Recording};
use engine::trace::{Number, TraceDocument, VerifiedTrace};
use engine::Execution;
use graph_core::{AtomKey, ErrorKind, Graph};
use std::collections::{BTreeMap, BTreeSet};
use support::*;

fn verified(log: &Recording) -> VerifiedTrace {
    let doc = TraceDocument::from_recording(
        log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let bytes = doc.to_json(16 * 1024 * 1024).unwrap();
    VerifiedTrace::import(&bytes, Default::default()).unwrap()
}
fn step(ex: &mut Execution, log: &mut Recording, source: AtomKey) {
    let query = ex.enabled_for(source).unwrap();
    let candidate = query.into_steps().into_iter().next().unwrap();
    log.commit(ex, candidate).unwrap();
}
fn chain() -> (Execution, Recording, Vec<AtomKey>) {
    let mut g = Graph::new();
    let mut sources = vec![];
    for (before, after) in [("a", "b"), ("b", "c"), ("c", "d")] {
        let h = rule(&mut g, 0, false, true);
        let a = ctor(&mut g, before);
        let b = ctor(&mut g, after);
        sides(&mut g, h, &[a], &[b]);
        sources.push(h);
    }
    ctor(&mut g, "a");
    let no_op = rule(&mut g, 0, false, true);
    sources.push(no_op);
    let ex = Execution::new(g, Default::default()).unwrap();
    let log = Recording::new(&ex, Default::default()).unwrap();
    (ex, log, sources)
}

#[test]
fn chronology_direct_creator_and_transitive_support_are_distinct() {
    let (mut ex, mut log, sources) = chain();
    for i in [3, 0, 1, 2] {
        step(&mut ex, &mut log, sources[i]);
    }
    let trace = verified(&log);
    let report = analyse(
        &trace,
        &Selection::Occurrences(BTreeSet::from([EventId(0), EventId(1)])),
        AnalysisLimits::default(),
    )
    .unwrap();
    let last = &report.observations[3];
    assert_eq!(
        last.selected_pre_action_history,
        BTreeSet::from([Number(0), Number(1)])
    );
    assert_eq!(
        last.selected_birth_read_ancestry,
        BTreeSet::from([Number(1)])
    );
    assert_eq!(last.input_origins.len(), 1);
    assert_eq!(last.input_origins[0].created_by, EventId(2));
    assert!(last.input_origins[0].selected_direct_creator.is_empty());
    assert_eq!(report.global_necessity, "unknown");
    assert_eq!(report.alternative_execution_coverage, "not_explored");
    assert_eq!(trace.document().events.len(), 4);
    assert_eq!(
        report.canonical_trace_sha256,
        engine::trace::sha256(&serde_json::to_vec(trace.document()).unwrap())
    );
}

#[test]
fn read_consume_order_is_not_birth_read_ancestry() {
    let mut g = Graph::new();
    let reader = rule(&mut g, 0, false, true);
    let a = ctor(&mut g, "resource");
    sides(&mut g, reader, &[a], &[a]);
    let consumer = rule(&mut g, 0, false, true);
    let b = ctor(&mut g, "resource");
    sides(&mut g, consumer, &[b], &[]);
    ctor(&mut g, "resource");
    let mut ex = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&ex, Default::default()).unwrap();
    step(&mut ex, &mut log, reader);
    step(&mut ex, &mut log, consumer);
    let trace = verified(&log);
    let order =
        DependencyOrder::extract(&trace.document().events, AnalysisLimits::default()).unwrap();
    assert!(order.edges()[&(EventId(0), EventId(1))]
        .iter()
        .any(|w| w.kind == DependencyKind::ReadConsume));
    let report = analyse(
        &trace,
        &Selection::AllOccurrences,
        AnalysisLimits::default(),
    )
    .unwrap();
    assert!(report.observations[1]
        .selected_birth_read_ancestry
        .is_empty());
    assert_eq!(
        report.observations[1].selected_pre_action_history,
        BTreeSet::from([Number(0)])
    );
}

/// Independent oracle: construct event adjacency from pairwise intersections,
/// traverse predecessors, then project labels. No incremental atom annotations.
fn compare_oracle(trace: &VerifiedTrace) {
    let records = &trace.document().events;
    let all = analyse(trace, &Selection::AllOccurrences, AnalysisLimits::default()).unwrap();
    let assignment: BTreeMap<_, _> = records
        .iter()
        .filter(|e| e.id.0 % 3 != 2)
        .map(|e| (e.id, BTreeSet::from([format!("class-{}", e.id.0 % 2)])))
        .collect();
    let classes = analyse(
        trace,
        &Selection::Classes(assignment.clone()),
        AnalysisLimits::default(),
    )
    .unwrap();
    for (i, record) in records.iter().enumerate() {
        let mut found = BTreeSet::new();
        let mut pending = vec![i];
        while let Some(j) = pending.pop() {
            for (k, prior) in records[..j].iter().enumerate() {
                if !prior
                    .footprint
                    .produced
                    .is_disjoint(&records[j].footprint.read)
                    && found.insert(k)
                {
                    pending.push(k);
                }
            }
        }
        assert_eq!(
            all.observations[i].selected_birth_read_ancestry,
            found.iter().map(|k| Number(*k as u64)).collect()
        );
        assert_eq!(all.observations[i].redex, record.redex);
        let projected: BTreeSet<_> = found
            .iter()
            .flat_map(|k| assignment.get(&EventId(*k as u64)).into_iter().flatten())
            .cloned()
            .map(Label::Class)
            .collect();
        let decoded: BTreeSet<_> = classes.observations[i]
            .selected_birth_read_ancestry
            .iter()
            .map(|n| classes.labels[n.0 as usize].clone())
            .collect();
        assert_eq!(decoded, projected);
        for origin in &all.observations[i].input_origins {
            assert!(records[origin.created_by.0 as usize]
                .footprint
                .produced
                .contains(&origin.atom));
            assert!(record.footprint.read.contains(&origin.atom));
        }
    }
}

#[test]
fn generated_histories_and_class_projection_match_independent_dag() {
    for seed in 0..64u64 {
        let (mut ex, mut log, sources) = chain();
        let mut random = seed + 1;
        for source in &sources[..3] {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            for _ in 0..random % 4 {
                step(&mut ex, &mut log, sources[3]);
            }
            step(&mut ex, &mut log, *source);
        }
        compare_oracle(&verified(&log));
    }
}

#[test]
fn reflective_examples_include_revision_and_representation_reads() {
    for fixture in [
        include_str!("../../../examples/fixtures/b2.atoms"),
        include_str!("../../../examples/fixtures/a2.atoms"),
    ] {
        let mut runner = engine::runner::Runner::new(
            graph_core::fixture::load(fixture).unwrap().graph,
            Default::default(),
            engine::scheduling::Scheduler::seeded(42),
            Observer::all(),
        )
        .unwrap();
        runner.enable_recording(Default::default()).unwrap();
        runner
            .run(
                engine::runner::RunLimits {
                    max_steps: 6,
                    ..Default::default()
                },
                None,
                || false,
            )
            .unwrap();
        let trace = verified(runner.recording().unwrap());
        assert!(!trace.document().events.is_empty());
        compare_oracle(&trace);
        let mut bad = trace.document().clone();
        bad.events[0].redex.revision = engine::recording::AtomId(u64::MAX);
        assert!(VerifiedTrace::verify(bad, Default::default()).is_err());
    }
}

#[test]
fn invalid_selection_and_exhaustion_never_publish_partial_results() {
    let (mut ex, mut log, sources) = chain();
    for source in sources {
        step(&mut ex, &mut log, source);
    }
    let trace = verified(&log);
    for selection in [
        Selection::Occurrences(BTreeSet::from([EventId(99)])),
        Selection::Classes(BTreeMap::from([(
            EventId(99),
            BTreeSet::from(["update".into()]),
        )])),
        Selection::Classes(BTreeMap::from([(EventId(0), BTreeSet::new())])),
        Selection::Classes(BTreeMap::from([(
            EventId(0),
            BTreeSet::from(["x".repeat(257)]),
        )])),
    ] {
        assert_eq!(
            analyse(&trace, &selection, AnalysisLimits::default())
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidConfiguration
        );
    }
    for selection in [
        Selection::Occurrences(BTreeSet::new()),
        Selection::Classes(BTreeMap::new()),
    ] {
        let report = analyse(&trace, &selection, AnalysisLimits::default()).unwrap();
        assert!(report.labels.is_empty());
        assert!(report
            .observations
            .iter()
            .all(|o| o.selected_birth_read_ancestry.is_empty()));
    }
    for work in [true, false] {
        let succeeds = |n| {
            analyse(
                &trace,
                &Selection::AllOccurrences,
                AnalysisLimits {
                    max_work: if work { n } else { 2_000_000 },
                    max_units: if work { 2_000_000 } else { n },
                },
            )
        };
        let (mut low, mut high) = (0, 2_000_000);
        while low < high {
            let mid = (low + high) / 2;
            if succeeds(mid).is_ok() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        assert!(low > 0);
        assert!(succeeds(low).is_ok());
        assert_eq!(
            succeeds(low - 1).unwrap_err().kind(),
            ErrorKind::ResourceExhaustion
        );
    }
    // A failed analysis cannot alter the checked trace or poison later analysis.
    compare_oracle(&trace);
}

#[test]
fn empty_trace_has_no_vacuous_necessity() {
    let ex = Execution::new(Graph::new(), Default::default()).unwrap();
    let log = Recording::new(&ex, Default::default()).unwrap();
    let report = analyse(
        &verified(&log),
        &Selection::AllOccurrences,
        AnalysisLimits::default(),
    )
    .unwrap();
    assert!(report.observations.is_empty());
    assert_eq!(report.global_necessity, "unknown");
    // API accepts only VerifiedTrace, never a caller-supplied report or event list.
}
