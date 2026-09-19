#[path = "../examples/support/recovery.rs"]
mod recovery;
#[allow(dead_code)]
mod support;
use engine::{Execution, ExecutionLimits, SearchLimits};
use graph_core::{AtomKey, Graph, Kind, Role};
use std::collections::{BTreeMap, BTreeSet};

fn limits() -> ExecutionLimits {
    ExecutionLimits {
        search: SearchLimits {
            max_work: 100_000_000,
            max_candidates: 100_000,
        },
        ..Default::default()
    }
}

#[test]
fn actual_rule_changes_finish_jobs_then_continue_without_changing_results() {
    let f = recovery::build(2, true);
    let mut run = Execution::new(f.graph.clone(), limits()).unwrap();
    let mut edits = 0;
    let mut completed = None;
    for _ in 0..9 {
        let enabled = run.enabled_compact().unwrap();
        let mut actual = enabled.bindings().to_vec();
        let mut expected = f.bindings(run.graph()).unwrap();
        actual.sort_by_key(|b| (b.source(), b.assignments().clone()));
        expected.sort_by_key(|b| (b.source(), b.assignments().clone()));
        assert_eq!(actual, expected, "restricted enumeration must be complete");
        assert!(!enabled.bindings().is_empty());
        let (step, _) = enabled
            .select(&engine::scheduling::Scheduler::Deterministic)
            .unwrap()
            .unwrap();
        let event = run.commit(step).unwrap();
        if !f.processors.contains(&event.source) {
            assert_eq!(
                event.altered,
                std::collections::BTreeSet::from(f.processors)
            );
            edits += 1;
        }
        let status = f.check(run.graph()).unwrap().statuses;
        if status.values().all(|s| s != "pending") {
            if let Some(old) = &completed {
                assert_eq!(old, &status);
            } else {
                completed = Some(status);
            }
        }
    }
    assert!(completed.is_some());
    assert!(edits > 2);
}

fn step(f: &recovery::Family, e: &mut Execution) {
    let b = f.bindings(e.graph()).unwrap().remove(0);
    let candidate = e.prepare(b).unwrap();
    e.commit(candidate).unwrap();
}

#[test]
fn every_small_processing_order_recovers_but_routes_are_not_confluent() {
    fn visit(f: &recovery::Family, g: Graph, depth: usize, outcomes: &mut BTreeSet<Vec<String>>) {
        let before = f.check(&g).unwrap();
        let bindings = f.bindings(&g).unwrap();
        if bindings.is_empty() {
            assert_eq!(before.pending, 0);
            assert!(!before.switch);
            assert_eq!(depth, 2 * f.items.len());
            outcomes.insert(before.statuses.into_values().collect());
            return;
        }
        assert!(depth < 2 * f.items.len());
        for b in bindings {
            let mut e = Execution::new(g.clone(), limits()).unwrap();
            let candidate = e.prepare(b).unwrap();
            e.commit(candidate).unwrap();
            let after = f.check(e.graph()).unwrap();
            assert_eq!(before.potential(), after.potential() + 1);
            assert_eq!(e.graph().len(), 190 + 2 * f.items.len() + 2 * after.pending);
            if before.pending == 0 {
                assert_eq!(before.statuses, after.statuses);
            }
            visit(f, e.graph().clone(), depth + 1, outcomes);
        }
    }
    for n in 0..=4 {
        let f = recovery::build(n, false);
        let mut outcomes = BTreeSet::new();
        visit(&f, f.graph.clone(), 0, &mut outcomes);
        assert!(!outcomes.is_empty());
        if n >= 2 {
            assert!(
                outcomes.len() > 1,
                "route-labelled outputs depend on item order"
            );
        }
    }
}

fn object_link(g: &Graph, label: &str, from: AtomKey) -> AtomKey {
    g.iter()
        .find(|(_, a)| {
            matches!(&a.kind, Kind::Record {
        role: Role::ObjectLink, label: Some(l) } if l == label)
                && a.fields[0] == from
        })
        .unwrap()
        .0
}

/// External disturbances create a new baseline; they are not hidden rewrite events.
fn reopen(f: &recovery::Family, g: &mut Graph, item: AtomKey) {
    let status = g
        .iter()
        .find(|(_, a)| a.kind.role() == Some(Role::ObjectLink) && a.fields == [item, item])
        .unwrap()
        .0;
    let remaining = object_link(g, "remaining", f.control);
    let tail = g.get(remaining).unwrap().fields[1];
    g.remove_set(&BTreeSet::from([status, remaining])).unwrap();
    let count = support::ctor(g, "Count");
    support::link(g, "next", count, tail);
    support::link(g, "remaining", f.control, count);
    support::link(g, "pending", item, item);
}

#[test]
fn finite_valid_disturbances_recover_from_both_control_phases() {
    for disturb_in_switch in [false, true] {
        let f = recovery::build(3, true);
        let mut e = Execution::new(f.graph.clone(), limits()).unwrap();
        for _ in 0..if disturb_in_switch { 5 } else { 6 } {
            step(&f, &mut e);
        }
        for item in &f.items {
            let mut g = e.graph().clone();
            reopen(&f, &mut g, *item);
            let initial = f.check(&g).unwrap();
            assert_eq!(initial.pending, 1);
            let bound = initial.potential();
            e.replace_graph(g).unwrap();
            for _ in 0..bound {
                step(&f, &mut e);
            }
            assert_eq!(f.check(e.graph()).unwrap().pending, 0);
        }
        let stable = f.check(e.graph()).unwrap().statuses;
        for _ in 0..5 {
            step(&f, &mut e);
            assert_eq!(f.check(e.graph()).unwrap().statuses, stable);
        }
    }
}

#[test]
fn core_valid_aliases_and_harmless_extras_are_outside_the_certificate() {
    let f = recovery::build(1, true);
    let mut g = f.graph.clone();
    support::ctor(&mut g, "harmless but not certified");
    graph_core::validate(&g).unwrap();
    assert!(f.check(&g).is_err());
    let mut g = f.graph.clone();
    let phase = object_link(&g, "phase", f.control);
    let value = g.get(phase).unwrap().fields[1];
    let quoted_work = g
        .iter()
        .find(|(id, a)| *id != value && a.kind == Kind::Constructor("Work".into()))
        .unwrap()
        .0;
    g.remove_set(&BTreeSet::from([phase, value])).unwrap();
    support::link(&mut g, "phase", f.control, quoted_work);
    graph_core::validate(&g).unwrap();
    assert!(f.check(&g).unwrap_err().contains("phase must be separate"));
    let e = Execution::new(g, limits()).unwrap();
    assert!(e.enabled_compact().unwrap().bindings().is_empty());
}

#[test]
fn guard_alone_allows_a_nonprogressing_loop_outside_the_useful_region() {
    let f = recovery::build(1, true);
    let mut g = f.graph.clone();
    let noop = support::rule(&mut g, 0, false, true);
    graph_core::validate(&g).unwrap();
    assert!(f.check(&g).is_err());
    let mut e = Execution::new(g, limits()).unwrap();
    for _ in 0..4 {
        let before = e.graph().snapshot();
        let p = e
            .prepare(engine::OperationalBinding::new(noop, BTreeMap::new()))
            .unwrap();
        e.commit(p).unwrap();
        assert_eq!(before, e.graph().snapshot());
        assert_eq!(
            recovery::statuses(e.graph(), &f.items).unwrap()[&f.items[0]],
            "pending"
        );
    }
}

#[test]
fn countdown_must_respect_object_link_endpoint_sorts() {
    let mut g = Graph::new();
    let c = support::ctor(&mut g, "control");
    let n = g.insert(Kind::Nat, &[]).unwrap();
    support::link(&mut g, "remaining", c, n);
    assert!(graph_core::validate(&g).is_err());
}

#[test]
fn family_selected_events_replay_through_existing_portable_validation() {
    let f = recovery::build(2, true);
    let mut e = Execution::new(f.graph.clone(), ExecutionLimits::default()).unwrap();
    let mut log = engine::recording::Recording::new(&e, Default::default()).unwrap();
    for _ in 0..8 {
        let b = f.bindings(e.graph()).unwrap().remove(0);
        let p = e.prepare(b).unwrap();
        log.commit(&mut e, p).unwrap();
    }
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
    assert_eq!(replay.state_at(8).unwrap().snapshot(), e.graph().snapshot());
    assert_eq!(f.check(e.graph()).unwrap().pending, 0);
}

#[test]
fn generated_source_loads_and_zero_item_normality_agrees_with_general_search() {
    for modulating in [false, true] {
        let f = recovery::build(0, modulating);
        let source = parser::print(&f.graph).unwrap();
        let loaded = parser::load(&source).unwrap().into_graph();
        assert_eq!(loaded.snapshot(), f.graph.snapshot());
        let e = Execution::new(loaded, limits()).unwrap();
        assert_eq!(
            e.enabled_compact().unwrap().bindings(),
            f.bindings(e.graph()).unwrap()
        );
    }
}

#[test]
fn reopening_without_counter_update_is_not_recovery_from_arbitrary_corruption() {
    let f = recovery::build(1, true);
    let mut e = Execution::new(f.graph.clone(), limits()).unwrap();
    step(&f, &mut e);
    let mut g = e.graph().clone();
    let status = object_link(&g, "route-a", f.items[0]);
    g.remove_set(&BTreeSet::from([status])).unwrap();
    support::link(&mut g, "pending", f.items[0], f.items[0]);
    graph_core::validate(&g).unwrap();
    assert!(f.check(&g).is_err());
}
