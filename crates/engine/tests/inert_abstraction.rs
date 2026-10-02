#[allow(dead_code)]
#[path = "../examples/support/inert_cases.rs"]
mod cases;
use engine::{
    meta_activity::{analyse, analyse_inert, project_inert},
    stability::Limits,
    Execution,
};
use graph_core::{ErrorKind, Kind, Role};
fn limits() -> Limits {
    Limits {
        max_states: 16,
        max_edges: 10000,
        max_retained_units: 10_000_000,
        max_comparison_work: 100_000_000,
        ..Default::default()
    }
}

#[test]
fn unbounded_logs_do_not_prevent_exact_finite_edit_bounds() {
    for name in ["spawn", "credit-restoration"] {
        for n in 0..=4 {
            let (g, c) = cases::case(name, n);
            let r = analyse_inert(g, c.as_ref(), limits(), "Log").unwrap();
            assert_eq!(r.total_edits.initial_bound, Some(n), "{name} {n}");
            assert!(r.abstraction.unwrap().edge_erasure_count > 0);
            assert!(r.edges.iter().any(|e| e.from == e.to && !e.edits()));
        }
    }
    let (g, c) = cases::case("spawn", 2);
    assert_eq!(
        analyse(g, c.as_ref(), limits()).unwrap_err().kind(),
        ErrorKind::ResourceExhaustion
    );
}

#[test]
fn retained_stuttering_distinguishes_starvation_from_post_recovery_logging() {
    for (name, want) in [
        ("output", "refuted"),
        ("gated-output", "certified"),
        ("gated", "certified"),
        ("ungated", "refuted"),
    ] {
        let (g, c) = cases::case(name, 1);
        let r = analyse_inert(g, c.as_ref(), limits(), "Log").unwrap();
        assert_eq!(r.productive_modulation, Some(want), "{name}");
        if name == "gated" || name == "ungated" {
            assert_eq!(r.total_edits.verdict, "refuted");
        }
    }
}

#[test]
fn readers_including_future_readers_are_unsupported_not_refuted() {
    for name in ["literal-reader", "wildcard-reader", "new-reader"] {
        let (g, c) = cases::case(name, 0);
        if name == "new-reader" {
            project_inert(
                &Execution::new(g.clone(), limits().execution).unwrap(),
                "Log",
                limits(),
            )
            .unwrap();
        }
        assert_eq!(
            analyse_inert(g, c.as_ref(), limits(), "Log")
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidBinding,
            "{name}"
        );
    }
}

#[test]
fn referenced_and_quoted_atoms_remain_and_projection_is_idempotent() {
    let (mut g, _) = cases::case("logger", 0);
    let isolated = g.insert(Kind::Constructor("Log".into()), &[]).unwrap();
    let connected = g.insert(Kind::Constructor("Log".into()), &[]).unwrap();
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some("hold".into()),
        },
        &[connected, connected],
    )
    .unwrap();
    let other = g.insert(Kind::Constructor("Other".into()), &[]).unwrap();
    let e = Execution::new(g.clone(), limits().execution).unwrap();
    let (p, n) = project_inert(&e, "Log", limits()).unwrap();
    assert_eq!(n, 1);
    assert!(p.atom(isolated).is_err());
    assert!(p.atom(connected).is_ok());
    assert!(p.atom(other).is_ok());
    assert_eq!(g.len(), e.graph().len());
    let (p2, n2) = project_inert(
        &Execution::new(p.clone(), limits().execution).unwrap(),
        "Log",
        limits(),
    )
    .unwrap();
    assert_eq!(n2, 0);
    assert_eq!(format!("{p:?}"), format!("{p2:?}"));
    assert!(p
        .iter()
        .any(|(id, a)| matches!(&a.kind,Kind::Constructor(s) if s=="Log")
            && !p.references_to(id).is_empty()));
}

#[test]
fn essential_growth_and_exhaustion_remain_inconclusive() {
    for name in ["connected", "unfunded-spawn"] {
        let (g, c) = cases::case(name, 0);
        assert_eq!(
            analyse_inert(g, c.as_ref(), limits(), "Log")
                .unwrap_err()
                .kind(),
            ErrorKind::ResourceExhaustion
        );
    }
    let (g, _) = cases::case("logger", 0);
    assert_eq!(
        analyse_inert(
            g,
            None,
            Limits {
                max_comparison_work: 0,
                ..limits()
            },
            "Log"
        )
        .unwrap_err()
        .kind(),
        ErrorKind::ResourceExhaustion
    );
}

#[test]
fn payloads_cannot_be_erased_and_default_schema_is_unchanged() {
    let (g, c) = cases::case("gated-output", 0);
    assert_eq!(
        analyse_inert(g, c.as_ref(), limits(), "Ticket")
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidBinding
    );
    let (g, c) = cases::meta::case("spawn", 1);
    let r = serde_json::to_value(analyse(g, c.as_ref(), limits()).unwrap()).unwrap();
    assert_eq!(r["schema"], "meta-activity-analysis-v1");
    assert!(r.get("abstraction").is_none());
}

// Independent field scan rather than the implementation's reverse-incidence index.
fn erase_by_scan(g: &graph_core::Graph) -> graph_core::Graph {
    let erased = g
        .iter()
        .filter(|(id, a)| {
            matches!(&a.kind,Kind::Constructor(s) if s=="Log")
                && !g.iter().any(|(_, b)| b.fields.contains(id))
        })
        .map(|(id, _)| id)
        .collect();
    let mut p = g.clone();
    p.remove_set(&erased).unwrap();
    p
}

#[test]
fn native_bindings_and_successors_commute_with_independent_erasure() {
    let mut checked = 0;
    for name in [
        "spawn",
        "credit-restoration",
        "gated",
        "ungated",
        "output",
        "gated-output",
    ] {
        for n in 0..=3 {
            let (mut graph, _) = cases::case(name, n);
            for _ in 0..n {
                graph.insert(Kind::Constructor("Log".into()), &[]).unwrap();
            }
            // Explore several different schedules, keeping concrete logs intact.
            for schedule in 0..3 {
                let mut concrete = graph.clone();
                for depth in 0..6 {
                    let e = Execution::new(concrete.clone(), limits().execution).unwrap();
                    let p = erase_by_scan(&concrete);
                    assert_eq!(
                        project_inert(&e, "Log", limits()).unwrap().0.snapshot(),
                        p.snapshot()
                    );
                    let reduced = Execution::new(p.clone(), limits().execution).unwrap();
                    let bindings: Vec<_> = e
                        .enabled()
                        .unwrap()
                        .into_steps()
                        .into_iter()
                        .map(|s| s.binding().clone())
                        .collect();
                    let reduced_bindings: Vec<_> = reduced
                        .enabled()
                        .unwrap()
                        .into_steps()
                        .into_iter()
                        .map(|s| s.binding().clone())
                        .collect();
                    assert_eq!(bindings, reduced_bindings, "{name} {n} {depth}");
                    let mut next = Vec::new();
                    for b in bindings {
                        let mut a = Execution::new(concrete.clone(), limits().execution).unwrap();
                        let mut bside = Execution::new(p.clone(), limits().execution).unwrap();
                        let ae = a.commit(a.prepare(b.clone()).unwrap()).unwrap();
                        let be = bside.commit(bside.prepare(b).unwrap()).unwrap();
                        assert_eq!(
                            (ae.entered, ae.exited, ae.altered),
                            (be.entered, be.exited, be.altered)
                        );
                        assert_eq!(
                            erase_by_scan(a.graph()).snapshot(),
                            erase_by_scan(bside.graph()).snapshot()
                        );
                        assert_eq!(a.graph().next_identity(), bside.graph().next_identity());
                        next.push(a.graph().clone());
                        checked += 1;
                    }
                    if next.is_empty() {
                        break;
                    }
                    concrete = next.swap_remove((schedule + depth) % next.len());
                }
            }
        }
    }
    assert!(checked > 500, "{checked}");
    eprintln!("checked {checked} paired native transitions");
}

#[test]
fn concrete_logging_really_grows_and_naive_label_erasure_changes_behaviour() {
    let (g, _) = cases::case("logger", 0);
    let mut e = Execution::new(g, limits().execution).unwrap();
    let initial = e.graph().len();
    for _ in 0..50 {
        let b = e
            .enabled()
            .unwrap()
            .into_steps()
            .remove(0)
            .binding()
            .clone();
        e.commit(e.prepare(b).unwrap()).unwrap();
    }
    assert_eq!(e.graph().len(), initial + 50);
    assert_eq!(project_inert(&e, "Log", limits()).unwrap().1, 50);
    let (g, _) = cases::case("literal-reader", 0);
    let before = Execution::new(g.clone(), limits().execution)
        .unwrap()
        .enabled()
        .unwrap()
        .steps()
        .len();
    let after = Execution::new(erase_by_scan(&g), limits().execution)
        .unwrap()
        .enabled()
        .unwrap()
        .steps()
        .len();
    assert!(
        before > after,
        "erasing an isolated but readable atom loses a native transition"
    );
}
