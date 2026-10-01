#[allow(dead_code)]
#[path = "../examples/support/meta_cases.rs"]
mod cases;
use engine::meta_activity::{analyse, verify_potential, weighted_bound};
use engine::stability::Limits;
fn limits() -> Limits {
    Limits {
        max_states: 128,
        max_edges: 10000,
        max_retained_units: 10_000_000,
        max_comparison_work: 100_000_000,
        ..Default::default()
    }
}

#[test]
fn all_small_weighted_graphs_agree_with_independent_reachability_and_walk_oracles() {
    let mut count = 0;
    for n in 1..=3usize {
        for code in 0..3usize.pow((n * n) as u32) {
            let mut code = code;
            let mut edges = Vec::new();
            let mut reachable = vec![vec![false; n]; n];
            for (a, row) in reachable.iter_mut().enumerate() {
                row[a] = true;
                for (b, cell) in row.iter_mut().enumerate() {
                    let kind = code % 3;
                    code /= 3;
                    if kind != 0 {
                        edges.push((a, b, kind == 2));
                        *cell = true;
                    }
                }
            }
            for k in 0..n {
                for a in 0..n {
                    for b in 0..n {
                        reachable[a][b] |= reachable[a][k] && reachable[k][b];
                    }
                }
            }
            let infinite = edges.iter().any(|&(a, b, w)| w && reachable[b][a]);
            let r = weighted_bound(n, &edges, 10000).unwrap();
            assert_eq!(r.verdict == "refuted", infinite);
            if infinite {
                assert!(!r.cycle_edges.is_empty());
                let cycle: Vec<_> = r.cycle_edges.iter().map(|&i| edges[i]).collect();
                assert!(cycle.iter().any(|e| e.2));
                for i in 0..cycle.len() {
                    assert_eq!(cycle[i].1, cycle[(i + 1) % cycle.len()].0);
                }
            } else {
                fn walks(at: usize, edges: &[(usize, usize, bool)], seen: usize) -> usize {
                    edges
                        .iter()
                        .filter(|e| e.0 == at && (seen & (1 << e.1)) == 0)
                        .map(|e| usize::from(e.2) + walks(e.1, edges, seen | (1 << e.1)))
                        .max()
                        .unwrap_or(0)
                }
                let expected: Vec<_> = (0..n).map(|i| walks(i, &edges, 1 << i)).collect();
                assert_eq!(r.potential, expected);
                assert!(verify_potential(n, &edges, &r.potential));
            }
            count += 1;
        }
    }
    assert_eq!(count, 19767);
}

#[test]
fn harmless_modulation_and_unproductive_modulation_are_not_the_same_contract() {
    for (name, want) in [("gated", "certified"), ("ungated", "refuted")] {
        let (g, c) = cases::case(name, 1);
        let r = analyse(g, c.as_ref(), limits()).unwrap();
        assert_eq!(r.total_edits.verdict, "refuted");
        assert_eq!(r.edit_burst.verdict, "refuted");
        assert_eq!(r.productive_modulation, Some(want));
    }
}

#[test]
fn total_edit_bound_allows_nonterminating_object_work_and_distinguishes_meta_dispatch() {
    for (name, edits) in [
        ("object-loop", 0),
        ("meta-noop", 0),
        ("self", 1),
        ("finite-then-object-loop", 1),
    ] {
        let (g, c) = cases::case(name, 1);
        let r = analyse(g, c.as_ref(), limits()).unwrap();
        assert_eq!(r.total_edits.initial_bound, Some(edits), "{name}: {r:?}");
        if name == "meta-noop" {
            assert_eq!(r.meta_invocations.verdict, "refuted");
            assert_eq!(r.meta_burst.verdict, "refuted");
        }
    }
}

#[test]
fn restoration_and_fresh_spawning_are_accounted_for_not_only_body_edits() {
    let (g, c) = cases::case("restoration", 1);
    let r = analyse(g, c.as_ref(), limits()).unwrap();
    assert_eq!(r.total_edits.verdict, "refuted");
    for n in 0..=3 {
        for name in ["credit-restoration", "spawn"] {
            let (g, c) = cases::case(name, n);
            let r = analyse(g, c.as_ref(), limits()).unwrap();
            assert_eq!(r.total_edits.initial_bound, Some(n), "{name} {n}: {r:?}");
            if name == "spawn" && n > 0 {
                assert!(r.edges.iter().any(|e| !e.entered.is_empty()));
            }
        }
    }
    let (g, c) = cases::case("unfunded-spawn", 0);
    assert_eq!(
        analyse(
            g,
            c.as_ref(),
            Limits {
                max_states: 3,
                ..limits()
            }
        )
        .unwrap_err()
        .kind(),
        graph_core::ErrorKind::ResourceExhaustion
    );
}

#[test]
fn certificates_are_not_trusted_without_all_edges_and_resource_failures_are_not_refutations() {
    assert!(verify_potential(1, &[], &[0]));
    assert!(!verify_potential(1, &[(0, 0, true)], &[0]));
    assert!(!verify_potential(1, &[(0, 1, true)], &[2]));
    assert!(weighted_bound(usize::MAX, &[], 0).is_err());
    assert!(weighted_bound(1, &[(0, 0, true)], 0).is_err());
    let (g, c) = cases::case("self", 1);
    assert_eq!(
        analyse(
            g,
            c.as_ref(),
            Limits {
                max_states: 0,
                ..limits()
            }
        )
        .unwrap_err()
        .kind(),
        graph_core::ErrorKind::ResourceExhaustion
    );
}

#[test]
fn bounded_bursts_and_repeated_object_steps_do_not_imply_finite_total_edits() {
    let (g, c) = cases::case("interleaved", 1);
    let report = analyse(g, c.as_ref(), limits()).unwrap();
    assert_eq!(report.total_edits.verdict, "refuted");
    assert_eq!(report.edit_burst.maximum_bound, Some(1));
    assert_eq!(report.meta_burst.maximum_bound, Some(1));
    assert_eq!(report.productive_modulation, Some("refuted"));
    assert_eq!(
        report.edits_without_object_progress.unwrap().verdict,
        "refuted"
    );
    let all = [(0, 1, true), (1, 0, false)];
    assert_eq!(weighted_bound(2, &all, 100).unwrap().verdict, "refuted");
    assert_eq!(
        weighted_bound(2, &all[..1], 100).unwrap().maximum_bound,
        Some(1)
    );
    // Recharging a credit after every edit is the same positive cycle.
    // Two-phase size change needs a measure/phase proof, not strict n each edge.
    for n in 1..100usize {
        let states = 2 * n + 1;
        let phases: Vec<_> = (1..states).map(|i| (i, i - 1, true)).collect();
        let scalar: Vec<_> = (0..states).map(|i| i.div_ceil(2)).collect();
        let phased: Vec<_> = (0..states).map(|i| i + 1).collect();
        assert!(!verify_potential(states, &phases, &scalar));
        assert!(verify_potential(states, &phases, &phased));
        assert_eq!(
            weighted_bound(states, &phases, 200000)
                .unwrap()
                .maximum_bound,
            Some(2 * n)
        );
    }
}

#[test]
fn retirement_counts_as_an_edit_and_renaming_preserves_the_result() {
    let (g, c) = cases::case("delete-head", 0);
    let mut renamed = graph_core::Graph::new();
    cases::bodies::append(&mut renamed, &g, "renamed:");
    for graph in [g, renamed] {
        let r = analyse(graph, c.as_ref(), limits()).unwrap();
        assert_eq!(r.total_edits.initial_bound, Some(1));
        assert!(r.edges.iter().any(|e| !e.exited.is_empty()));
    }
}

#[test]
fn finite_edit_gaps_do_not_hide_object_starvation_or_output_disagreement() {
    let (mut g, c) = cases::case("guard", 1);
    let idle=parser::load("program {rule idle {atom z=Nat();rank r=z;mode m=ModeObject;revision v;active a;lhs {} rhs {}}}").unwrap().into_graph();
    cases::bodies::append(&mut g, &idle, "idle:");
    let r = analyse(g, c.as_ref(), limits()).unwrap();
    assert_eq!(r.total_edits.initial_bound, Some(1));
    assert_eq!(
        r.edits_without_object_progress.unwrap().maximum_bound,
        Some(1)
    );
    assert_eq!(r.productive_modulation, Some("refuted"));
    for agreement in [false, true] {
        let (g, mut c) = cases::bodies::output();
        c.schema = "recovery-contract-v2".into();
        c.require_agreement = agreement;
        let r = analyse(g, Some(&c), limits()).unwrap();
        assert!(!r.recovery.as_ref().unwrap().agreement);
        assert_eq!(
            r.productive_modulation,
            Some(if agreement { "refuted" } else { "certified" })
        );
    }
}
