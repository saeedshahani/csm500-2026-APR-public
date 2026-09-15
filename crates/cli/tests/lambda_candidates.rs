//! Retained local encoding experiments. These are deliberately incomplete
//! candidates, not implementations or refutations of all direct/interaction nets.
use graph_core::{AtomKey, Binding, Graph, Kind, Role, Sort};
use std::collections::BTreeSet;
fn c(g: &mut Graph, s: &str) -> AtomKey {
    g.insert(Kind::Constructor(s.into()), &[]).unwrap()
}
fn var(g: &mut Graph) -> AtomKey {
    g.insert(Kind::variable(Sort::Constructor, Binding::Free), &[])
        .unwrap()
}
fn edge(g: &mut Graph, s: &str, a: AtomKey, b: AtomKey) -> AtomKey {
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(s.into()),
        },
        &[a, b],
    )
    .unwrap()
}
fn rule(g: &mut Graph, l: &[AtomKey], r: &[AtomKey]) -> AtomKey {
    let h = g.insert(Kind::RuleHead, &[]).unwrap();
    let z = g.insert(Kind::Nat, &[]).unwrap();
    for (role, fields) in [
        (Role::Rank, vec![h, z]),
        (Role::Revision, vec![h]),
        (Role::ModeObject, vec![h]),
        (Role::Active, vec![h]),
    ] {
        g.insert(Kind::record(role), &fields).unwrap();
    }
    for id in l {
        g.insert(Kind::record(Role::Lhs), &[h, *id]).unwrap();
    }
    for id in r {
        g.insert(Kind::record(Role::Rhs), &[h, *id]).unwrap();
    }
    h
}

#[test]
fn direct_single_binding_cell_accumulates_ambiguous_reentrant_bindings() {
    let mut g = Graph::new();
    let cell = var(&mut g);
    let value = var(&mut g);
    let request = edge(&mut g, "request", cell, value);
    let bound = edge(&mut g, "binding", cell, value);
    let h = rule(&mut g, &[cell, value, request], &[cell, value, bound]);
    let host = c(&mut g, "binder");
    let first = c(&mut g, "first");
    let second = c(&mut g, "second");
    edge(&mut g, "request", host, first);
    edge(&mut g, "request", host, second);
    let mut execution = engine::Execution::new(g, engine::ExecutionLimits::default()).unwrap();
    for _ in 0..2 {
        let step = execution.enabled_for(h).unwrap().into_steps().remove(0);
        execution.commit(step).unwrap();
    }
    let targets: BTreeSet<_> = execution
        .graph()
        .iter()
        .filter(|(_, a)| {
            a.fields.first() == Some(&host)
                && a.kind
                    == Kind::Record {
                        role: Role::ObjectLink,
                        label: Some("binding".into()),
                    }
        })
        .map(|(_, a)| a.fields[1])
        .collect();
    assert_eq!(targets, BTreeSet::from([first, second]));
    // Appending to one binder cell is not a lexical closure environment. Deleting
    // the old edge instead would discard a still-captured prior activation.
}

#[test]
fn active_pair_identity_splice_works_but_unaccounted_fan_port_dangles() {
    for extra_port in [false, true] {
        let mut g = Graph::new();
        let app = c(&mut g, "ApplyPort");
        let lam = c(&mut g, "LambdaPort");
        let result = var(&mut g);
        let arg = var(&mut g);
        let active = edge(&mut g, "principal", app, lam);
        let out = edge(&mut g, "result", app, result);
        let input = edge(&mut g, "argument", app, arg);
        let bound = edge(&mut g, "identity-body", lam, lam);
        let wire = edge(&mut g, "wire", result, arg);
        let h = rule(
            &mut g,
            &[app, lam, result, arg, active, out, input, bound],
            &[result, arg, wire],
        );
        let a = c(&mut g, "ApplyPort");
        let l = c(&mut g, "LambdaPort");
        let r = c(&mut g, "result");
        let x = c(&mut g, "value");
        edge(&mut g, "principal", a, l);
        edge(&mut g, "result", a, r);
        edge(&mut g, "argument", a, x);
        edge(&mut g, "identity-body", l, l);
        if extra_port {
            let fan = c(&mut g, "FanPort");
            edge(&mut g, "second-use", fan, l);
        }
        let before = g.snapshot();
        let mut execution = engine::Execution::new(g, engine::ExecutionLimits::default()).unwrap();
        let enabled = execution.enabled_for(h).unwrap();
        if extra_port {
            assert!(enabled.steps().is_empty());
            assert_eq!(enabled.rejected().len(), 1);
            assert_eq!(
                enabled.rejected()[0].diagnostic.kind(),
                graph_core::ErrorKind::Dangling
            );
            assert_eq!(execution.graph().snapshot(), before);
        } else {
            assert_eq!(enabled.steps().len(), 1);
            let event = execution
                .commit(enabled.into_steps().pop().unwrap())
                .unwrap();
            assert_eq!(event.produced.len(), 1);
            assert_eq!(event.consumed.len(), 6);
            assert!(execution.graph().iter().any(|(_, a)| a.kind
                == Kind::Record {
                    role: Role::ObjectLink,
                    label: Some("wire".into())
                }
                && a.fields == [r, x]));
        }
    }
}
