#[path = "../examples/support/stabilisation.rs"]
mod model;
use model::bodies;
#[allow(dead_code)]
mod support;
use engine::stability::{analyse, reconstruct_statuses, verify_reconstruction, Limits};
use engine::Execution;
use graph_core::{Kind, Role};
use std::collections::BTreeSet;
fn limits() -> Limits {
    Limits {
        max_states: 1024,
        max_edges: 10000,
        max_retained_units: 10_000_000,
        max_comparison_work: 100_000_000,
        ..Default::default()
    }
}

#[test]
fn within_run_stability_does_not_imply_agreement_but_normalisation_repairs_it() {
    let (g, mut c) = bodies::output();
    let old = analyse(g.clone(), &c, limits()).unwrap();
    assert_eq!(old.verdict, "certified");
    assert_eq!(old.agreement, "disagreed");
    assert_eq!(old.legitimate_outputs.len(), 2);
    c.schema = "recovery-contract-v2".into();
    c.require_agreement = true;
    assert_eq!(analyse(g, &c, limits()).unwrap().verdict, "refuted");
    let (g, c) = model::single("pending", false);
    let r = analyse(g, &c, limits()).unwrap();
    assert_eq!(r.verdict, "certified", "{r:?}");
    assert_eq!(r.agreement, "agreed");
    assert_eq!(r.recovery_bound, Some(3));
}

#[test]
fn every_one_and_two_component_fault_baseline_recovers_with_same_output() {
    for n in 1..=2 {
        for code in 0..6usize.pow(n) {
            let mut value = code;
            let states: Vec<_> = (0..n)
                .map(|_| {
                    let s = value % 6;
                    value /= 6;
                    s
                })
                .collect();
            let (g, c) = model::family(&states);
            let r = analyse(g, &c, limits()).unwrap();
            assert_eq!(r.verdict, "certified", "{states:?}: {r:?}");
            assert_eq!(r.agreement, "agreed");
            assert!(r.recovery_bound.unwrap() <= model::potential(&states));
            assert!(r.legitimate_outputs[0]
                .iter()
                .enumerate()
                .all(|(i, x)| x == &vec![(format!("{i}:done"), format!("{i}:Ticket"))]));
        }
    }
}

#[test]
fn all_concrete_steps_match_independent_six_state_transition_table() {
    for s in 0..6 {
        let (g, c) = model::single(["pending", "audited", "done"][s % 3], s >= 3);
        let run = Execution::new(g.clone(), Default::default()).unwrap();
        let mut got = Vec::new();
        for step in run.enabled().unwrap().into_steps() {
            let mut next = g.clone();
            engine::apply_with_binding(
                &mut next,
                step.binding().source(),
                step.binding().assignments(),
            )
            .unwrap();
            let cfg = graph_core::validate(&next).unwrap();
            let meta = cfg.rules.values().find(|r| r.meta).unwrap().head;
            let ex = Execution::new(next.clone(), Default::default()).unwrap();
            let upgraded = !ex
                .enabled()
                .unwrap()
                .steps()
                .iter()
                .any(|s| s.binding().source() == meta);
            let visible = next
                .iter()
                .filter(|(k, _)| cfg.rules.values().all(|r| !r.representation.contains(k)));
            let label = visible
                .filter_map(|(_, a)| match &a.kind {
                    Kind::Record {
                        role: Role::ObjectLink,
                        label: Some(l),
                    } => Some(l.as_str()),
                    _ => None,
                })
                .next()
                .unwrap();
            got.push(
                ["pending", "audited", "done"]
                    .iter()
                    .position(|x| *x == label)
                    .unwrap()
                    + 3 * usize::from(upgraded),
            );
            assert!(analyse(next, &c, limits()).unwrap().verdict == "certified");
        }
        got.sort();
        let mut expected = model::abstract_next(s);
        expected.sort();
        assert_eq!(got, expected);
    }
}

#[test]
fn missing_and_duplicate_statuses_need_explicit_external_reconstruction() {
    for statuses in [
        vec![],
        vec!["audited", "pending"],
        vec!["done", "done"],
        vec!["audited"],
    ] {
        let (mut g, c) = model::single("done", true);
        model::set_status(&mut g, "Ticket", &statuses);
        let original = g.snapshot();
        let next_id = g.next_identity();
        let (fixed, receipt) = reconstruct_statuses(&g, &c, "pending", Default::default()).unwrap();
        verify_reconstruction(&g, &fixed, &c, &receipt, "pending", Default::default()).unwrap();
        assert_eq!(g.snapshot(), original);
        assert_eq!(g.next_identity(), next_id);
        let r = analyse(fixed.clone(), &c, limits()).unwrap();
        assert_eq!(r.verdict, "certified");
        assert_eq!(
            r.recovery_bound,
            Some(if statuses.len() == 1 { 1 } else { 2 })
        );
        let (_, again) = reconstruct_statuses(&fixed, &c, "pending", Default::default()).unwrap();
        assert!(again.removed.is_empty() && again.created.is_empty());
        let mut forged = receipt.clone();
        forged.created.push(999);
        assert!(
            verify_reconstruction(&g, &fixed, &c, &forged, "pending", Default::default()).is_err()
        );
        if statuses.is_empty() {
            assert_eq!(analyse(g, &c, limits()).unwrap().verdict, "refuted");
        }
    }
}

#[test]
fn reconstruction_rejects_lost_payload_invalid_core_ambiguity_and_invalid_incidence() {
    let (g, c) = model::single("pending", false);
    let mut bad = c.clone();
    assert!(reconstruct_statuses(&g, &bad, "done", Default::default()).is_err());
    bad = c.clone();
    bad.requirements.push(bad.requirements[0].clone());
    assert!(reconstruct_statuses(&g, &bad, "pending", Default::default()).is_err());
    let mut ambiguous = g.clone();
    ambiguous
        .insert(Kind::Constructor("Ticket".into()), &[])
        .unwrap();
    graph_core::validate(&ambiguous).unwrap();
    assert!(reconstruct_statuses(&ambiguous, &c, "pending", Default::default()).is_err());
    let mut wrong_target = g.clone();
    model::set_status(&mut wrong_target, "Ticket", &[]);
    let cfg = graph_core::validate(&wrong_target).unwrap();
    let ticket = wrong_target
        .iter()
        .find(|(k, a)| {
            a.kind == Kind::Constructor("Ticket".into())
                && cfg.rules.values().all(|r| !r.representation.contains(k))
        })
        .unwrap()
        .0;
    let other = wrong_target
        .insert(Kind::Constructor("Other".into()), &[])
        .unwrap();
    support::link(&mut wrong_target, "pending", ticket, other);
    graph_core::validate(&wrong_target).unwrap();
    assert!(reconstruct_statuses(&wrong_target, &c, "pending", Default::default()).is_err());
    let mut missing = g.clone();
    model::set_status(&mut missing, "Ticket", &[]);
    let cfg = graph_core::validate(&missing).unwrap();
    let item = missing
        .iter()
        .find(|(k, a)| {
            a.kind == Kind::Constructor("Ticket".into())
                && cfg.rules.values().all(|r| !r.representation.contains(k))
        })
        .unwrap()
        .0;
    missing.remove_set(&BTreeSet::from([item])).unwrap();
    assert!(reconstruct_statuses(&missing, &c, "pending", Default::default()).is_err());
    let mut invalid = g.clone();
    let h = graph_core::validate(&invalid)
        .unwrap()
        .rules
        .values()
        .next()
        .unwrap()
        .head;
    let rev = support::owned(&invalid, h, Role::Revision);
    invalid.remove_set(&BTreeSet::from([rev])).unwrap();
    assert!(reconstruct_statuses(&invalid, &c, "pending", Default::default()).is_err());
    let mut dangling = g.clone();
    model::set_status(&mut dangling, "Ticket", &["pending", "pending"]);
    let cfg = graph_core::validate(&dangling).unwrap();
    let status = dangling
        .iter()
        .find(|(k, a)| {
            matches!(&a.kind,Kind::Record{role:Role::ObjectLink,label:Some(l)} if l=="pending")
                && cfg.rules.values().all(|r| !r.representation.contains(k))
        })
        .unwrap()
        .0;
    support::link(&mut dangling, "dependent", status, status);
    // ObjectLink endpoints must be constructors: this is rejected at the core
    // boundary, not evidence that a core-valid dangling deletion was attempted.
    assert_eq!(
        graph_core::validate(&dangling).unwrap_err().kind(),
        graph_core::ErrorKind::InvalidConfiguration
    );
    let snapshot = dangling.snapshot();
    assert!(reconstruct_statuses(&dangling, &c, "pending", Default::default()).is_err());
    assert_eq!(dangling.snapshot(), snapshot);
}

#[test]
fn arbitrary_rule_corruption_is_not_repaired_by_fixing_statuses() {
    let source = include_str!("../../../examples/practical/repair-policy.graph").replacen(
        "\"done\", item, item",
        "\"pending\", item, item",
        1,
    );
    let g = parser::load(&source).unwrap().into_graph();
    let c = model::single("done", false).1;
    assert_eq!(analyse(g, &c, limits()).unwrap().verdict, "refuted");
}

#[test]
fn different_schedules_and_finite_fault_epochs_replay_and_preserve_payload() {
    for strategy in 0..3 {
        for epoch in 0..6 {
            let (graph, c) = model::family(&[epoch, (epoch + 2) % 6]);
            let anchors: Vec<_> = graph
                .iter()
                .filter(|(_, a)| matches!(&a.kind,Kind::Constructor(l) if l.ends_with(":Ticket")))
                .map(|(k, a)| (k, a.clone()))
                .collect();
            let mut e = Execution::new(graph, Default::default()).unwrap();
            let mut log = engine::recording::Recording::new(&e, Default::default()).unwrap();
            let mut count = 0;
            loop {
                let mut steps = e.enabled().unwrap().into_steps();
                if steps.is_empty() {
                    break;
                }
                let index = match strategy {
                    0 => 0,
                    1 => steps.len() - 1,
                    _ => count % steps.len(),
                };
                log.commit(&mut e, steps.remove(index)).unwrap();
                count += 1;
                assert!(count <= model::potential(&[epoch, (epoch + 2) % 6]));
            }
            for (key, atom) in anchors {
                assert_eq!(e.graph().get(key), Some(&atom));
            }
            let r = analyse(e.graph().clone(), &c, limits()).unwrap();
            assert_eq!(r.recovery_bound, Some(0));
            assert_eq!(r.agreement, "agreed");
            let trace = engine::trace::TraceDocument::from_recording(
                &log,
                &engine::observation::Observer::all(),
                None,
                Default::default(),
                Default::default(),
            )
            .unwrap();
            let replay = engine::trace::VerifiedTrace::import(
                &trace.to_json(2_000_000).unwrap(),
                Default::default(),
            )
            .unwrap();
            assert_eq!(
                replay.state_at(count).unwrap().snapshot(),
                e.graph().snapshot()
            );
        }
    }
}

#[test]
fn old_contracts_cannot_silently_enable_new_guarantees_and_limits_stay_inconclusive() {
    let (g, mut c) = model::single("pending", false);
    c.schema = "recovery-contract-v1".into();
    assert!(analyse(g.clone(), &c, limits()).is_err());
    c.schema = "recovery-contract-v2".into();
    for bound in [0, 1] {
        let l = Limits {
            max_states: bound,
            ..limits()
        };
        assert_eq!(
            analyse(g.clone(), &c, l).unwrap_err().kind(),
            graph_core::ErrorKind::ResourceExhaustion
        );
    }
    assert_eq!(
        analyse(
            g,
            &c,
            Limits {
                max_retained_units: 1,
                ..limits()
            }
        )
        .unwrap_err()
        .kind(),
        graph_core::ErrorKind::ResourceExhaustion
    );
}

#[test]
fn an_unrestricted_meta_match_can_corrupt_the_repair_rule_itself() {
    let source = include_str!("../../../examples/practical/repair-policy.graph")
        .replace(" lg: input_guard; lgm: input_member;", "")
        .replace(" rg: input_guard; rgm: input_member;", "")
        .replace(
            "  atom input_guard = Record[ObjectLink](\"pending\", quoted_item, quoted_item);\n",
            "",
        )
        .replace(
            "  atom input_member = Record[Lhs](target, input_guard);\n",
            "",
        );
    let graph = parser::load(&source).unwrap().into_graph();
    let c = model::single("pending", false).1;
    let r = analyse(graph, &c, limits()).unwrap();
    assert_eq!(r.verdict, "refuted");
}

#[test]
fn repeated_live_status_faults_restart_explicitly_and_recover_after_the_last_fault() {
    let (mut graph, c) = model::single("pending", false);
    let original = graph.snapshot();
    for statuses in [vec![], vec!["pending", "done"], vec!["audited"], vec![]] {
        model::set_status(&mut graph, "Ticket", &statuses);
        let (fixed, receipt) =
            reconstruct_statuses(&graph, &c, "pending", Default::default()).unwrap();
        verify_reconstruction(&graph, &fixed, &c, &receipt, "pending", Default::default()).unwrap();
        let mut run = Execution::new(fixed, Default::default()).unwrap();
        if let Some(step) = run.enabled().unwrap().into_steps().into_iter().next() {
            run.commit(step).unwrap();
        }
        graph = run.graph().clone();
    }
    let report = analyse(graph.clone(), &c, limits()).unwrap();
    assert_eq!(report.verdict, "certified");
    assert!(report.recovery_bound.unwrap() <= 3);
    let mut run = Execution::new(graph, Default::default()).unwrap();
    let mut count = 0;
    while let Some(step) = run.enabled().unwrap().into_steps().into_iter().next() {
        run.commit(step).unwrap();
        count += 1;
        assert!(count <= 3);
    }
    let final_report = analyse(run.graph().clone(), &c, limits()).unwrap();
    assert_eq!(final_report.recovery_bound, Some(0));
    // All constructors, including the protected visible payload, survive these status faults.
    for (key, atom) in original
        .into_iter()
        .filter(|(_, a)| matches!(&a.kind, Kind::Constructor(_)))
    {
        assert_eq!(run.graph().get(key), Some(&atom));
    }
}
