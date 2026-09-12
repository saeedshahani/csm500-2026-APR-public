//! Falsification and independent complete-carrier expectations for link normalisation.
//! Selection here is explicit test control, not the production scheduler.
#[allow(dead_code)]
mod support;
use engine::{Execution, ExecutionLimits};
use graph_core::{validate, AtomKey, Graph, Kind, Role, Sort};
use std::collections::BTreeSet;
use support::{ctor, link, rule, sides, variable};

fn relabel(g: &mut Graph, from: &str, to: &str) -> AtomKey {
    let h = rule(g, 0, false, true);
    let x = variable(g, Sort::Constructor);
    let y = variable(g, Sort::Constructor);
    let old = link(g, from, x, y);
    let new = link(g, to, x, y);
    sides(g, h, &[x, y, old], &[x, y, new]);
    h
}

fn visible_links(g: &Graph) -> Vec<(String, Vec<AtomKey>)> {
    let config = validate(g).unwrap();
    let hidden: BTreeSet<_> = config
        .rules
        .values()
        .flat_map(|r| &r.representation)
        .collect();
    let mut result: Vec<_> = g
        .iter()
        .filter_map(|(id, a)| {
            if hidden.contains(&id) {
                return None;
            }
            match &a.kind {
                Kind::Record {
                    role: Role::ObjectLink,
                    label: Some(label),
                } => Some((label.clone(), a.fields.clone())),
                _ => None,
            }
        })
        .collect();
    result.sort();
    result
}

fn check_all_orders(run: Execution, invariant: &Graph, pending: usize) {
    let enabled = run.enabled().unwrap();
    assert_eq!(enabled.steps().len(), pending);
    assert!(enabled.rejected().is_empty());
    for (id, atom) in invariant.iter() {
        assert_eq!(run.graph().get(id), Some(atom));
    }
    assert_eq!(run.graph().len(), invariant.len() + 3);
    if pending == 0 {
        let a = invariant
            .iter()
            .find(|(_, a)| a.kind == Kind::Constructor("A".into()))
            .unwrap()
            .0;
        let b = invariant
            .iter()
            .find(|(_, a)| a.kind == Kind::Constructor("B".into()))
            .unwrap()
            .0;
        let mut expected = vec![
            ("done".into(), vec![a, b]),
            ("done".into(), vec![a, b]),
            ("done".into(), vec![b, a]),
        ];
        expected.sort();
        assert_eq!(visible_links(run.graph()), expected);
    }
    for step in enabled.into_steps() {
        let mut next = Execution::new(run.graph().clone(), ExecutionLimits::default()).unwrap();
        // A candidate is state-stamped, so prepare for the new test branch.
        let candidate = next.prepare(step.binding().clone()).unwrap();
        let event = next.commit(candidate).unwrap();
        assert_eq!(event.consumed.len(), 1);
        assert_eq!(event.produced.len(), 1);
        assert!(event.altered.is_empty() && event.entered.is_empty() && event.exited.is_empty());
        check_all_orders(next, invariant, pending - 1);
    }
}

#[test]
fn all_orders_with_shared_endpoints_and_parallel_links_have_the_expected_carrier() {
    let mut g = Graph::new();
    relabel(&mut g, "pending", "done");
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    let invariant = g.clone();
    link(&mut g, "pending", a, b);
    link(&mut g, "pending", a, b);
    link(&mut g, "pending", b, a);
    check_all_orders(
        Execution::new(g, ExecutionLimits::default()).unwrap(),
        &invariant,
        3,
    );
}

#[test]
fn generated_link_families_preserve_every_other_atom_and_normalise_exactly_once() {
    // A fixed LCG produces topology and selection choices, not semantic expectations.
    let mut seed = 0x533342_u64;
    for case in 0..32 {
        let mut g = Graph::new();
        relabel(&mut g, "pending", "done");
        let vertices: Vec<_> = (0..4).map(|i| ctor(&mut g, &format!("v{i}"))).collect();
        link(&mut g, "value", vertices[0], vertices[1]);
        ctor(&mut g, "unattached");
        let invariant = g.clone();
        let count = case % 6;
        let mut expected = visible_links(&invariant);
        for _ in 0..count {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let a = seed as usize % 4;
            let b = (a + 1 + ((seed >> 8) as usize % 3)) % 4;
            link(&mut g, "pending", vertices[a], vertices[b]);
            expected.push(("done".into(), vec![vertices[a], vertices[b]]));
        }
        expected.sort();
        for reverse in [false, true] {
            let mut run = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
            for remaining in (1..=count).rev() {
                let mut steps = run.enabled().unwrap().into_steps();
                assert_eq!(steps.len(), remaining);
                let step = if reverse {
                    steps.pop().unwrap()
                } else {
                    steps.remove(0)
                };
                let event = run.commit(step).unwrap();
                assert_eq!(event.consumed.len(), 1);
                assert_eq!(event.produced.len(), 1);
            }
            assert!(run.enabled().unwrap().steps().is_empty());
            assert_eq!(visible_links(run.graph()), expected);
            assert_eq!(run.graph().len(), invariant.len() + count);
            for (id, atom) in invariant.iter() {
                assert_eq!(run.graph().get(id), Some(atom));
            }
        }
    }
}

#[test]
fn competing_outputs_refute_confluence_despite_strict_decrease() {
    let mut g = Graph::new();
    relabel(&mut g, "pending", "left");
    relabel(&mut g, "pending", "right");
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    link(&mut g, "pending", a, b);
    let run = Execution::new(g, ExecutionLimits::default()).unwrap();
    let steps = run.enabled().unwrap().into_steps();
    assert_eq!(steps.len(), 2);
    let mut outputs = Vec::new();
    for step in steps {
        let mut branch = Execution::new(run.graph().clone(), ExecutionLimits::default()).unwrap();
        let prepared = branch.prepare(step.binding().clone()).unwrap();
        branch.commit(prepared).unwrap();
        assert!(branch.enabled().unwrap().steps().is_empty());
        outputs.push(visible_links(branch.graph()));
    }
    assert_ne!(outputs[0], outputs[1]);
}

#[test]
fn allowing_reverse_relabelling_refutes_termination_under_gc() {
    let mut g = Graph::new();
    relabel(&mut g, "pending", "done");
    relabel(&mut g, "done", "pending");
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    link(&mut g, "pending", a, b);
    let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
    for i in 0..12 {
        let mut steps = run.enabled().unwrap().into_steps();
        assert_eq!(steps.len(), 1);
        let event = run.commit(steps.pop().unwrap()).unwrap();
        assert!(event.altered.is_empty());
        assert_eq!(
            visible_links(run.graph())[0].0,
            if i % 2 == 0 { "done" } else { "pending" }
        );
    }
}

#[test]
fn self_link_is_not_a_match_for_two_injectively_distinct_endpoints() {
    let mut g = Graph::new();
    relabel(&mut g, "pending", "done");
    let a = ctor(&mut g, "A");
    link(&mut g, "pending", a, a);
    let run = Execution::new(g, ExecutionLimits::default()).unwrap();
    assert!(run.enabled().unwrap().steps().is_empty());
    assert_eq!(visible_links(run.graph())[0].0, "pending");
}

#[test]
fn equal_data_observations_do_not_preserve_normality() {
    let mut active = Graph::new();
    let h = relabel(&mut active, "pending", "done");
    let a = ctor(&mut active, "A");
    let b = ctor(&mut active, "B");
    link(&mut active, "pending", a, b);
    let mut inactive = active.clone();
    inactive
        .remove_set(&BTreeSet::from([support::owned(&active, h, Role::Active)]))
        .unwrap();
    assert_eq!(visible_links(&active), visible_links(&inactive));
    assert_eq!(
        Execution::new(active, ExecutionLimits::default())
            .unwrap()
            .enabled()
            .unwrap()
            .steps()
            .len(),
        1
    );
    assert!(Execution::new(inactive, ExecutionLimits::default())
        .unwrap()
        .enabled()
        .unwrap()
        .steps()
        .is_empty());
}
