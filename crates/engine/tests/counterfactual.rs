#[allow(dead_code)]
mod support;
use engine::counterfactual::{analyse, Limits, Outcome};
use engine::observation::Observer;
use engine::recording::{EventId, Recording};
use engine::trace::{TraceDocument, VerifiedTrace};
use engine::{Execution, OperationalBinding};
use graph_core::{ErrorKind, Graph, Role, Sort};
use std::collections::BTreeMap;
use support::*;

fn verify(log: &Recording) -> VerifiedTrace {
    VerifiedTrace::verify(
        TraceDocument::from_recording(
            log,
            &Observer::all(),
            None,
            Default::default(),
            Default::default(),
        )
        .unwrap(),
        Default::default(),
    )
    .unwrap()
}
fn fire(run: &mut Execution, log: &mut Recording, source: graph_core::AtomKey) {
    let step = run.enabled_for(source).unwrap().into_steps().pop().unwrap();
    log.commit(run, step).unwrap();
}

#[test]
fn omitted_allocation_is_transported_not_confused_with_another_atom() {
    let mut g = Graph::new();
    let noise = rule(&mut g, 0, false, true);
    let noise_atom = ctor(&mut g, "Noise");
    sides(&mut g, noise, &[], &[noise_atom]);
    let producer = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "Product");
    sides(&mut g, producer, &[], &[p]);
    let reader = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "Product");
    let result = ctor(&mut g, "Result");
    let edge = link(&mut g, "uses", result, p);
    sides(&mut g, reader, &[p], &[p, result, edge]);
    let mut run = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&run, Default::default()).unwrap();
    for source in [noise, producer, reader] {
        fire(&mut run, &mut log, source);
    }
    let trace = verify(&log);
    let report = analyse(&trace, EventId(2), Default::default()).unwrap();
    assert_eq!(report.minimal_supports, vec![vec![EventId(1)]]);
    assert_eq!(report.necessary_in_this_universe, vec![EventId(1)]);
    assert!(matches!(
        report.trials[0].outcome,
        Outcome::TargetUnavailable { .. }
    ));
    assert_eq!(report.trials[2].outcome, Outcome::Admitted);
    assert_eq!(trace.document().events, verify(&log).document().events);
    for limits in [
        Limits {
            max_prefix: 1,
            ..Default::default()
        },
        Limits {
            analysis: engine::causal::AnalysisLimits {
                max_work: 0,
                max_units: 10000,
            },
            ..Default::default()
        },
        Limits {
            analysis: engine::causal::AnalysisLimits {
                max_work: 10000,
                max_units: 0,
            },
            ..Default::default()
        },
    ] {
        assert_eq!(
            analyse(&trace, EventId(2), limits).unwrap_err().kind(),
            ErrorKind::ResourceExhaustion
        );
    }
    assert_eq!(
        analyse(&trace, EventId(3), Default::default())
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidConfiguration
    );
}

#[test]
fn new_quotation_then_removal_is_nonmonotone_and_version_sensitive() {
    let mut g = Graph::new();
    let data = ctor(&mut g, "Data");
    let owner = rule(&mut g, 0, false, false);
    let mut editors = Vec::new();
    for create in [true, false] {
        let source = rule(&mut g, 1, true, true);
        let h = variable(&mut g, Sort::RuleHead);
        let c = ctor(&mut g, "Data");
        let m = record(&mut g, Role::Rhs, &[h, c]);
        let old = record(&mut g, Role::Revision, &[h]);
        let new = record(&mut g, Role::Revision, &[h]);
        if create {
            sides(&mut g, source, &[h, c, old], &[h, c, new, m]);
        } else {
            sides(&mut g, source, &[h, c, old, m], &[h, c, new]);
        }
        editors.push((source, h, c, m, old, create));
    }
    let reader = rule(&mut g, 0, false, true);
    let c = ctor(&mut g, "Data");
    sides(&mut g, reader, &[c], &[c]);
    let mut run = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&run, Default::default()).unwrap();
    // Repeat the exposure/quotation cycle: the same surviving data identity is
    // quoted twice, with distinct memberships and exact owner revisions.
    for (source, h, c, m, rev, create) in editors.into_iter().cycle().take(4) {
        let mut assignments = BTreeMap::from([
            (h, owner),
            (c, data),
            (rev, owned(run.graph(), owner, Role::Revision)),
        ]);
        if !create {
            assignments.insert(m, owned(run.graph(), owner, Role::Rhs));
        }
        let step = run
            .prepare(OperationalBinding::new(source, assignments))
            .unwrap();
        log.commit(&mut run, step).unwrap();
        assert_eq!(
            run.enabled_for(reader).unwrap().steps().len(),
            usize::from(!create)
        );
    }
    fire(&mut run, &mut log, reader);
    let trace = verify(&log);
    assert!(engine::conditions::extract(&trace, Default::default()).is_err());
    let report = analyse(&trace, EventId(4), Default::default()).unwrap();
    for (mask, trial) in report.trials.iter().enumerate() {
        if [0, 3, 15].contains(&mask) {
            assert_eq!(trial.outcome, Outcome::Admitted);
        } else if [1, 7].contains(&mask) {
            assert!(matches!(trial.outcome, Outcome::TargetRejected { .. }));
        } else {
            assert!(matches!(trial.outcome, Outcome::InfeasiblePrefix { .. }));
        }
    }
    assert_eq!(report.minimal_supports, vec![vec![]]);
    assert!(report.necessary_in_this_universe.is_empty());
}

#[test]
fn dangling_is_checked_not_misreported_as_missing_positive_dependency() {
    let mut g = Graph::new();
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    link(&mut g, "blocks", a, b);
    let remover = rule(&mut g, 0, false, true);
    let pa = ctor(&mut g, "A");
    let pb = ctor(&mut g, "B");
    let pl = link(&mut g, "blocks", pa, pb);
    sides(&mut g, remover, &[pa, pb, pl], &[pa, pb]);
    let target = rule(&mut g, 0, false, true);
    let pa = ctor(&mut g, "A");
    sides(&mut g, target, &[pa], &[]);
    let mut run = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&run, Default::default()).unwrap();
    for source in [remover, target] {
        fire(&mut run, &mut log, source);
    }
    let report = analyse(&verify(&log), EventId(1), Default::default()).unwrap();
    assert!(
        matches!(&report.trials[0].outcome, Outcome::TargetRejected { reason } if reason.contains("dangling"))
    );
    assert_eq!(report.minimal_supports, vec![vec![EventId(0)]]);
}

#[test]
fn alternatives_depend_on_whether_the_query_fixes_the_binding() {
    // An independent Boolean oracle: some Product exists iff producer 0 OR 1
    // ran. The exact recorded Product, in contrast, has exactly one producer.
    for choose_first in [false, true] {
        let mut g = Graph::new();
        let mut producers = Vec::new();
        for _ in 0..2 {
            let h = rule(&mut g, 0, false, true);
            let p = ctor(&mut g, "Product");
            sides(&mut g, h, &[], &[p]);
            producers.push(h);
        }
        let reader = rule(&mut g, 0, false, true);
        let p = ctor(&mut g, "Product");
        sides(&mut g, reader, &[p], &[p]);
        let mut run = Execution::new(g, Default::default()).unwrap();
        let mut log = Recording::new(&run, Default::default()).unwrap();
        for h in producers {
            fire(&mut run, &mut log, h);
        }
        let mut steps = run.enabled_for(reader).unwrap().into_steps();
        let step = steps.remove(usize::from(!choose_first));
        log.commit(&mut run, step).unwrap();
        let trace = verify(&log);
        let exact = analyse(&trace, EventId(2), Default::default()).unwrap();
        assert_eq!(exact.minimal_supports.len(), 1);
        assert_eq!(exact.minimal_supports[0].len(), 1);
        let existential = engine::counterfactual::analyse_query(
            &trace,
            EventId(2),
            engine::counterfactual::Query::SourceRevision,
            Default::default(),
        )
        .unwrap();
        for (mask, trial) in existential.trials.iter().enumerate() {
            assert_eq!(trial.outcome == Outcome::Admitted, mask != 0);
            assert_eq!(trial.witness_binding.is_some(), mask != 0);
        }
        assert_eq!(
            existential.minimal_supports,
            vec![vec![EventId(0)], vec![EventId(1)]]
        );
        assert!(existential.necessary_in_this_universe.is_empty());
    }
}

#[test]
fn indirect_completed_context_visibility_is_replayed() {
    // The matched link itself is visible. Its outgoing closure reaches a quoted
    // constructor. V1 inspects bound constructors only and misses this condition.
    let mut g = Graph::new();
    let hidden = ctor(&mut g, "Hidden");
    let visible = ctor(&mut g, "Visible");
    link(&mut g, "reaches", visible, hidden);
    let owner = rule(&mut g, 0, false, false);
    sides(&mut g, owner, &[], &[hidden]);
    let remover = rule(&mut g, 1, true, true);
    let h = variable(&mut g, Sort::RuleHead);
    let c = ctor(&mut g, "Hidden");
    let m = record(&mut g, Role::Rhs, &[h, c]);
    let old = record(&mut g, Role::Revision, &[h]);
    let new = record(&mut g, Role::Revision, &[h]);
    sides(&mut g, remover, &[h, c, m, old], &[h, c, new]);
    let reader = rule(&mut g, 0, false, true);
    let edge = variable(&mut g, Sort::Record(Role::ObjectLink));
    sides(&mut g, reader, &[edge], &[edge]);
    let binding = OperationalBinding::new(
        remover,
        BTreeMap::from([
            (h, owner),
            (c, hidden),
            (m, owned(&g, owner, Role::Rhs)),
            (old, owned(&g, owner, Role::Revision)),
        ]),
    );
    let mut run = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&run, Default::default()).unwrap();
    let step = run.prepare(binding).unwrap();
    log.commit(&mut run, step).unwrap();
    fire(&mut run, &mut log, reader);
    let trace = verify(&log);
    assert!(engine::conditions::extract(&trace, Default::default())
        .unwrap()
        .witnesses
        .is_empty());
    let report = analyse(&trace, EventId(1), Default::default()).unwrap();
    assert!(
        matches!(&report.trials[0].outcome,Outcome::TargetRejected{reason} if reason.contains("representation"))
    );
    assert_eq!(report.minimal_supports, vec![vec![EventId(0)]]);
}

#[test]
fn existential_enumeration_exhaustion_is_not_a_false_negative() {
    let mut g = Graph::new();
    let first = ctor(&mut g, "Product");
    ctor(&mut g, "Product");
    let target = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "Product");
    sides(&mut g, target, &[p], &[p]);
    let mut limits = engine::ExecutionLimits::default();
    limits.search.max_candidates = 1;
    let mut run = Execution::new(g, limits).unwrap();
    let mut log = Recording::new(&run, Default::default()).unwrap();
    let step = run
        .prepare(OperationalBinding::new(
            target,
            BTreeMap::from([(p, first)]),
        ))
        .unwrap();
    log.commit(&mut run, step).unwrap();
    let trace = verify(&log);
    assert_eq!(
        analyse(&trace, EventId(0), Default::default())
            .unwrap()
            .trials[0]
            .outcome,
        Outcome::Admitted
    );
    assert_eq!(
        engine::counterfactual::analyse_query(
            &trace,
            EventId(0),
            engine::counterfactual::Query::SourceRevision,
            Default::default()
        )
        .unwrap_err()
        .kind(),
        ErrorKind::ResourceExhaustion
    );
}

#[test]
fn target_revision_cannot_be_replaced_by_an_older_rule_with_the_same_head() {
    let mut g = Graph::new();
    let target = rule(&mut g, 0, false, true);
    let a = ctor(&mut g, "A");
    sides(&mut g, target, &[], &[a]);
    let editor = rule(&mut g, 1, true, true);
    let h = variable(&mut g, Sort::RuleHead);
    let c = ctor(&mut g, "A");
    let old_m = record(&mut g, Role::Rhs, &[h, c]);
    let old_v = record(&mut g, Role::Revision, &[h]);
    let new_v = record(&mut g, Role::Revision, &[h]);
    let b = ctor(&mut g, "B");
    let new_m = record(&mut g, Role::Rhs, &[h, b]);
    sides(
        &mut g,
        editor,
        &[h, c, old_m, old_v],
        &[h, c, new_v, b, new_m],
    );
    let binding = OperationalBinding::new(
        editor,
        BTreeMap::from([
            (h, target),
            (c, a),
            (old_m, owned(&g, target, Role::Rhs)),
            (old_v, owned(&g, target, Role::Revision)),
        ]),
    );
    let mut run = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&run, Default::default()).unwrap();
    let step = run.prepare(binding).unwrap();
    log.commit(&mut run, step).unwrap();
    fire(&mut run, &mut log, target);
    let trace = verify(&log);
    for query in [
        engine::counterfactual::Query::ExactOccurrence,
        engine::counterfactual::Query::SourceRevision,
    ] {
        let report =
            engine::counterfactual::analyse_query(&trace, EventId(1), query, Default::default())
                .unwrap();
        assert!(
            matches!(&report.trials[0].outcome,Outcome::TargetUnavailable{reason} if reason.contains("revision"))
        );
        assert_eq!(report.minimal_supports, vec![vec![EventId(0)]]);
    }
}
