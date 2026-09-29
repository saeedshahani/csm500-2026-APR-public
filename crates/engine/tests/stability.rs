use engine::stability::{analyse, Contract, Limits};
#[path = "../examples/support/body_revisions.rs"]
mod bodies;
#[allow(dead_code)]
mod support;
use engine::stability::rank_quotient;
use graph_core::{ErrorKind, Graph, Kind};
use std::collections::BTreeSet;
#[test]
fn real_rhs_and_lhs_revisions_are_certified_without_a_generator_manifest() {
    for (source, contract, bound) in [
        (
            include_str!("../../../examples/practical/revise-output.graph"),
            include_str!("../../../examples/practical/revise-output.contract.json"),
            2,
        ),
        (
            include_str!("../../../examples/practical/revise-guard.graph"),
            include_str!("../../../examples/practical/revise-guard.contract.json"),
            3,
        ),
    ] {
        let graph = parser::load(source).unwrap().graph().clone();
        let contract: Contract = serde_json::from_str(contract).unwrap();
        let report = analyse(graph, &contract, Limits::default()).unwrap();
        assert_eq!(report.verdict, "certified", "{report:?}");
        assert_eq!(report.recovery_bound, Some(bound));
        assert!(report.edges.iter().any(|e| !e.altered_rules.is_empty()));
    }
}

#[test]
fn genuine_body_changes_can_continue_after_recovery_but_cannot_starve_it() {
    let (g, c) = bodies::modulating(true);
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "certified", "{r:?}");
    assert_eq!(r.recovery_bound, Some(1));
    assert!(
        r.edges
            .iter()
            .filter(|e| !e.altered_rules.is_empty())
            .count()
            >= 2
    );
    assert!(r
        .edges
        .iter()
        .any(|e| e.to.is_some_and(|j| j <= e.from && r.legitimate[j])));
    let (g, c) = bodies::modulating(false);
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "refuted");
    assert!(r.reason.contains("cycle"));
    assert!(!r.witness_states.is_empty());
}

#[test]
fn unrelated_atoms_and_noninterfering_packages_are_not_rejected_by_membership() {
    let (mut g, c) = bodies::guard(2);
    support::ctor(&mut g, "unrelated note");
    assert_eq!(
        analyse(g, &c, Limits::default()).unwrap().verdict,
        "certified"
    );
    let (a, ca) = bodies::output();
    let (b, cb) = bodies::guard(2);
    let mut g = Graph::new();
    bodies::append(&mut g, &a, "A:");
    bodies::append(&mut g, &b, "B:");
    let mut c = bodies::prefixed(&ca, "A:");
    let cb = bodies::prefixed(&cb, "B:");
    c.requirements.extend(cb.requirements);
    c.preserve.extend(cb.preserve);
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "certified", "{r:?}");
    assert_eq!(r.recovery_bound, Some(5));
}

#[test]
fn background_work_can_break_unfair_composition_even_without_shared_atoms() {
    let (a, ca) = bodies::guard(1);
    let (mut b, cb) = bodies::modulating(false);
    let pending = b
        .iter()
        .find(|(_, v)| {
            matches!(&v.kind,Kind::Record{label:Some(s),..} if s=="pending")
                && v.fields[0] == v.fields[1]
                && b.get(v.fields[0]).unwrap().kind == Kind::Constructor("Ticket".into())
                && graph_core::validate(&b)
                    .unwrap()
                    .rules
                    .values()
                    .all(|r| !r.representation.contains(&v.fields[0]))
        })
        .map(|(k, v)| (k, v.fields[0]))
        .unwrap();
    b.remove_set(&BTreeSet::from([pending.0])).unwrap();
    support::link(&mut b, "done", pending.1, pending.1);
    assert_eq!(
        analyse(b.clone(), &cb, Limits::default()).unwrap().verdict,
        "certified"
    );
    let mut g = Graph::new();
    bodies::append(&mut g, &a, "A:");
    bodies::append(&mut g, &b, "B:");
    let mut c = bodies::prefixed(&ca, "A:");
    let cb = bodies::prefixed(&cb, "B:");
    c.requirements.extend(cb.requirements);
    c.preserve.extend(cb.preserve);
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "refuted");
    assert!(r.reason.contains("cycle"));
}

#[test]
fn active_empty_rules_and_repair_undoing_are_real_counterexamples() {
    let (mut g, c) = bodies::guard(1);
    support::rule(&mut g, 0, false, true);
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "refuted");
    assert!(r.reason.contains("cycle"));
    let (mut g, c) = bodies::guard(1);
    let h = support::rule(&mut g, 0, false, true);
    let x = support::variable(&mut g, graph_core::Sort::Constructor);
    let done = support::link(&mut g, "done", x, x);
    let pending = support::link(&mut g, "pending", x, x);
    support::sides(&mut g, h, &[x, done], &[x, pending]);
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "refuted");
    assert!(r.reason.contains("closure"));
}

#[test]
fn resource_exhaustion_never_becomes_a_certificate_or_a_counterexample() {
    for changed in 0..6 {
        let (g, c) = bodies::guard(2);
        let mut l = Limits::default();
        match changed {
            0 => l.max_states = 0,
            1 => l.max_edges = 0,
            2 => l.max_retained_units = 0,
            3 => l.max_comparison_work = 0,
            4 => l.execution.search.max_work = 0,
            _ => l.execution.search.max_candidates = 0,
        }
        assert_eq!(
            analyse(g, &c, l).unwrap_err().kind(),
            ErrorKind::ResourceExhaustion,
            "bound {changed}"
        );
    }
    // A stable but growing legitimate carrier has no finite explored quotient.
    let (g, c) = bodies::guard(1);
    let mut e = engine::Execution::new(g, Default::default()).unwrap();
    for _ in 0..2 {
        let step = e.enabled().unwrap().into_steps().remove(0);
        e.commit(step).unwrap();
    }
    let mut g = e.graph().clone();
    let h = support::rule(&mut g, 0, false, true);
    let noise = support::ctor(&mut g, "unbounded noise");
    support::sides(&mut g, h, &[], &[noise]);
    assert_eq!(
        analyse(
            g,
            &c,
            Limits {
                max_states: 8,
                ..Default::default()
            }
        )
        .unwrap_err()
        .kind(),
        ErrorKind::ResourceExhaustion
    );
}

#[test]
fn deleting_and_recreating_identical_payload_is_not_preservation() {
    let (mut g, c) = bodies::guard(1);
    let h = support::rule(&mut g, 0, false, true);
    let old = support::ctor(&mut g, "Work:0");
    let new = support::ctor(&mut g, "Work:0");
    let pending = support::link(&mut g, "pending", old, old);
    let done = support::link(&mut g, "done", new, new);
    support::sides(&mut g, h, &[old, pending], &[new, done]);
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "refuted");
    assert!(r.reason.contains("identity"));
}

#[test]
fn rank_algorithm_agrees_with_independent_bounded_path_enumeration() {
    fn bad_path(i: usize, left: usize, legit: &[bool], edges: &[Vec<usize>]) -> bool {
        if legit[i] {
            return false;
        }
        if left == 0 || edges[i].is_empty() {
            return true;
        }
        edges[i]
            .iter()
            .any(|j| bad_path(*j, left - 1, legit, edges))
    }
    let mut checked = 0;
    for n in 1..=3 {
        for bits in 0..(1usize << (n * n)) {
            let edges: Vec<Vec<usize>> = (0..n)
                .map(|i| (0..n).filter(|j| bits & (1 << (i * n + j)) != 0).collect())
                .collect();
            for mask in 0..(1usize << n) {
                let legit: Vec<_> = (0..n).map(|i| mask & (1 << i) != 0).collect();
                let expected = !(0..n).any(|i| {
                    (legit[i] && edges[i].iter().any(|j| !legit[*j]))
                        || bad_path(i, n, &legit, &edges)
                });
                let result = rank_quotient(&legit, &edges);
                assert_eq!(result.is_ok(), expected, "{legit:?} {edges:?}");
                if let Ok(ranks) = result {
                    for i in 0..n {
                        if !legit[i] {
                            for j in &edges[i] {
                                assert!(ranks[i].unwrap() > ranks[*j].unwrap_or(0));
                            }
                        }
                    }
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 4164);
}

#[test]
fn individual_version_proofs_do_not_certify_arbitrary_switching() {
    let legit = [false, false, true];
    assert!(rank_quotient(&legit, &[vec![1], vec![2], vec![]]).is_ok());
    assert!(rank_quotient(&legit, &[vec![2], vec![0], vec![]]).is_ok());
    assert!(rank_quotient(&legit, &[vec![1, 2], vec![0, 2], vec![]])
        .unwrap_err()
        .contains("cycle"));
    // An online filter that rejects the harmful step still cannot infer progress.
    assert!(rank_quotient(&legit, &[vec![], vec![2], vec![]])
        .unwrap_err()
        .contains("deadlock"));
}

#[test]
fn body_changes_cannot_be_merged_by_payload_equality() {
    let (g, _) = bodies::guard(1);
    let mut e = engine::Execution::new(g, Default::default()).unwrap();
    let before = graph_core::validate(e.graph()).unwrap();
    let data = |g: &Graph| {
        let c = graph_core::validate(g).unwrap();
        g.iter()
            .filter(|(k, _)| c.rules.values().all(|r| !r.representation.contains(k)))
            .filter(|(_, a)| !matches!(a.kind, Kind::RuleHead))
            .map(|(k, a)| (k, a.clone()))
            .collect::<Vec<_>>()
    };
    let payload = data(e.graph());
    let step = e.enabled().unwrap().into_steps().remove(0);
    assert!(before.rules[&step.binding().source()].meta);
    let mut reference = e.graph().clone();
    let expected = engine::apply_with_binding(
        &mut reference,
        step.binding().source(),
        step.binding().assignments(),
    )
    .unwrap();
    let event = e.commit(step).unwrap();
    assert_eq!(event, expected);
    assert_eq!(
        e.graph().iter().collect::<Vec<_>>(),
        reference.iter().collect::<Vec<_>>()
    );
    assert_eq!(payload, data(e.graph()));
    let after = graph_core::validate(e.graph()).unwrap();
    let next = e.enabled().unwrap();
    assert!(next
        .steps()
        .iter()
        .all(|s| !after.rules[&s.binding().source()].meta));
}

#[test]
fn guard_family_has_predicted_complete_state_space_and_rank() {
    for n in 1..=5 {
        let (g, c) = bodies::guard(n);
        let r = analyse(g, &c, Limits::default()).unwrap();
        assert_eq!(r.verdict, "certified");
        assert_eq!(r.states, 1 + (1 << n));
        assert_eq!(r.edges.len(), 1 + n * (1 << (n - 1)));
        assert_eq!(r.recovery_bound, Some(n + 1));
    }
}

#[test]
fn invalid_contracts_and_ambiguous_payloads_are_not_accepted() {
    let (g, mut c) = bodies::guard(1);
    c.requirements[0].legitimate_labels.insert("unknown".into());
    assert!(analyse(g, &c, Limits::default()).is_err());
    let (mut g, c) = bodies::guard(1);
    support::ctor(&mut g, "Work:0");
    assert!(analyse(g, &c, Limits::default())
        .unwrap_err()
        .to_string()
        .contains("unique"));
}

#[test]
fn legitimate_output_constancy_is_an_additional_obligation() {
    let (mut g, mut c) = bodies::guard(1);
    c.requirements[0].labels.insert("audited".into());
    c.requirements[0].legitimate_labels.insert("audited".into());
    for (a, b) in [("done", "audited"), ("audited", "done")] {
        let h = support::rule(&mut g, 0, false, true);
        let x = support::variable(&mut g, graph_core::Sort::Constructor);
        let old = support::link(&mut g, a, x, x);
        let new = support::link(&mut g, b, x, x);
        support::sides(&mut g, h, &[x, old], &[x, new]);
    }
    let r = analyse(g.clone(), &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "refuted");
    assert!(r.edges.last().unwrap().to.is_none());
    c.stable_when_legitimate = false;
    let r = analyse(g, &c, Limits::default()).unwrap();
    assert_eq!(r.verdict, "certified");
    assert_eq!(r.recovery_bound, Some(2));
}
