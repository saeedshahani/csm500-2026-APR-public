#[allow(dead_code)]
mod support;
use engine::causal::{AnalysisLimits, DependencyOrder};
use engine::conflict::{LocalSnapshot, Residual};
use engine::recording::{Recording, RecordingLimits};
use engine::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{ErrorKind, Graph, Kind, Role, Sort};
use std::collections::BTreeMap;
use support::*;

fn relabel(g: &mut Graph, from: &str, to: &str) -> graph_core::AtomKey {
    let h = rule(g, 0, false, true);
    let x = variable(g, Sort::Constructor);
    let y = variable(g, Sort::Constructor);
    let old = link(g, from, x, y);
    let new = link(g, to, x, y);
    sides(g, h, &[x, y, old], &[x, y, new]);
    h
}
#[test]
fn disjoint_and_shared_read_rewrites_commute_with_corresponding_fresh_atoms() {
    for shared in [false, true] {
        let mut g = Graph::new();
        relabel(&mut g, "pending", "done");
        let a = ctor(&mut g, "A");
        let b = ctor(&mut g, "B");
        let c = if shared { a } else { ctor(&mut g, "C") };
        let d = if shared { b } else { ctor(&mut g, "D") };
        link(&mut g, "pending", a, b);
        link(&mut g, "pending", c, d);
        let mut e = Execution::new(g, ExecutionLimits::default()).unwrap();
        let before = e.graph().snapshot();
        let next = e.graph().clone().insert(Kind::Nat, &[]).unwrap();
        let snapshot = LocalSnapshot::analyse(&e, AnalysisLimits::default()).unwrap();
        assert_eq!(snapshot.alternatives.len(), 2);
        assert_eq!(snapshot.pairs.len(), 1);
        let pair = &snapshot.pairs[0];
        assert!(!pair.conflicts());
        assert_eq!(pair.commutes, Some(true));
        assert_eq!(pair.corresponding_events, Some(true));
        assert_eq!(e.graph().snapshot(), before);
        assert_eq!(e.graph().clone().insert(Kind::Nat, &[]).unwrap(), next);
        let selected = e.enabled().unwrap().into_steps().remove(0);
        let mut log = Recording::new(&e, RecordingLimits::default()).unwrap();
        log.commit(&mut e, selected).unwrap();
        assert!(snapshot.alternatives.contains(&log.events()[0].redex));
        assert_eq!(snapshot.pre_state.len(), before.len());
        let selected = e.enabled().unwrap().into_steps().remove(0);
        log.commit(&mut e, selected).unwrap();
        assert!(
            DependencyOrder::extract(log.events(), AnalysisLimits::default())
                .unwrap()
                .edges()
                .is_empty()
        );
    }
}
#[test]
fn consuming_the_same_occurrence_conflicts_even_when_another_match_remains() {
    let mut g = Graph::new();
    relabel(&mut g, "pending", "red");
    relabel(&mut g, "pending", "blue");
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    link(&mut g, "pending", a, b);
    link(&mut g, "pending", a, b);
    let e = Execution::new(g, ExecutionLimits::default()).unwrap();
    let snapshot = LocalSnapshot::analyse(&e, AnalysisLimits::default()).unwrap();
    assert_eq!(snapshot.alternatives.len(), 4);
    assert_eq!(snapshot.pairs.iter().filter(|p| p.conflicts()).count(), 2);
    assert!(snapshot
        .pairs
        .iter()
        .filter(|p| !p.conflicts())
        .all(|p| p.commutes == Some(true)));
    assert!(snapshot
        .pairs
        .iter()
        .filter(|p| p.conflicts())
        .all(|p| p.commutes.is_none()));
}
#[test]
fn fresh_incoming_link_causes_dangling_interference() {
    let mut g = Graph::new();
    let delete = rule(&mut g, 0, false, true);
    let pattern = ctor(&mut g, "A");
    sides(&mut g, delete, &[pattern], &[]);
    let create = rule(&mut g, 0, false, true);
    let pattern = ctor(&mut g, "A");
    let edge = link(&mut g, "new", pattern, pattern);
    sides(&mut g, create, &[pattern], &[pattern, edge]);
    ctor(&mut g, "A");
    let e = Execution::new(g, ExecutionLimits::default()).unwrap();
    let s = LocalSnapshot::analyse(&e, AnalysisLimits::default()).unwrap();
    assert_eq!(s.pairs.len(), 1);
    assert!(s.pairs[0].conflicts());
    let reasons = [
        &s.pairs[0].second_after_first,
        &s.pairs[0].first_after_second,
    ];
    assert!(reasons
        .iter()
        .any(|r| matches!(r,Residual::Invalidated(d) if d.kind()==ErrorKind::Dangling)));
}
#[test]
fn promotion_and_deactivation_invalidate_exact_source_occurrences() {
    for mode in [true, false] {
        let mut g = Graph::new();
        let target = rule(&mut g, 0, false, true);
        let source = rule(&mut g, 1, true, true);
        let h = variable(&mut g, Sort::RuleHead);
        let role = if mode { Role::ModeObject } else { Role::Active };
        let old = record(&mut g, role, &[h]);
        let rev = record(&mut g, Role::Revision, &[h]);
        let fresh = record(&mut g, Role::Revision, &[h]);
        let mut rhs = vec![h, fresh];
        if mode {
            rhs.push(record(&mut g, Role::ModeMeta, &[h]));
        }
        sides(&mut g, source, &[h, old, rev], &rhs);
        let e = Execution::new(g, ExecutionLimits::default()).unwrap();
        let s = LocalSnapshot::analyse(&e, AnalysisLimits::default()).unwrap();
        let pair = s
            .pairs
            .iter()
            .find(|p| {
                s.alternatives[p.first].source == target.into()
                    || s.alternatives[p.second].source == target.into()
            })
            .unwrap();
        assert!(pair.conflicts());
        assert!(
            matches!(pair.second_after_first, Residual::Survives)
                || matches!(pair.first_after_second, Residual::Survives)
        );
    }
}
#[test]
fn conflict_exhaustion_is_inconclusive_and_does_not_mutate_baseline() {
    let mut g = Graph::new();
    rule(&mut g, 0, false, true);
    rule(&mut g, 0, false, true);
    let e = Execution::new(g, ExecutionLimits::default()).unwrap();
    let before = e.graph().snapshot();
    for limits in [
        AnalysisLimits {
            max_work: 0,
            ..AnalysisLimits::default()
        },
        AnalysisLimits {
            max_units: 0,
            ..AnalysisLimits::default()
        },
        AnalysisLimits {
            max_work: 100,
            ..AnalysisLimits::default()
        },
    ] {
        assert_eq!(
            LocalSnapshot::analyse(&e, limits).unwrap_err().kind(),
            ErrorKind::ResourceExhaustion
        );
        assert_eq!(e.graph().snapshot(), before);
    }
    let mut limits = ExecutionLimits::default();
    limits.search.max_work = 0;
    let e = Execution::new(e.graph().clone(), limits).unwrap();
    assert_eq!(
        LocalSnapshot::analyse(&e, AnalysisLimits::default())
            .unwrap_err()
            .kind(),
        ErrorKind::ResourceExhaustion
    );
    let empty = Execution::new(Graph::new(), ExecutionLimits::default()).unwrap();
    assert!(LocalSnapshot::analyse(&empty, AnalysisLimits::default())
        .unwrap()
        .pairs
        .is_empty());
}
#[test]
fn unquoting_enables_a_read_without_any_positive_resource_edge() {
    let mut g = Graph::new();
    let target = rule(&mut g, 0, false, false);
    let hidden = ctor(&mut g, "Hidden");
    sides(&mut g, target, &[], &[hidden]);
    let membership = owned(&g, target, Role::Rhs);
    let revision = owned(&g, target, Role::Revision);
    let expose = rule(&mut g, 1, true, true);
    let h = variable(&mut g, Sort::RuleHead);
    let c = ctor(&mut g, "Hidden");
    let member = record(&mut g, Role::Rhs, &[h, c]);
    let rev = record(&mut g, Role::Revision, &[h]);
    let fresh = record(&mut g, Role::Revision, &[h]);
    sides(&mut g, expose, &[h, c, member, rev], &[h, c, fresh]);
    let reader = rule(&mut g, 0, false, true);
    let template = ctor(&mut g, "Hidden");
    let flag = ctor(&mut g, "Seen");
    sides(&mut g, reader, &[template], &[template, flag]);
    let mut e = Execution::new(g, ExecutionLimits::default()).unwrap();
    assert!(e.enabled_for(reader).unwrap().steps().is_empty());
    let mut log = Recording::new(&e, RecordingLimits::default()).unwrap();
    let first = e
        .prepare(OperationalBinding::new(
            expose,
            BTreeMap::from([
                (h, target),
                (c, hidden),
                (member, membership),
                (rev, revision),
            ]),
        ))
        .unwrap();
    log.commit(&mut e, first).unwrap();
    let second = e
        .prepare(OperationalBinding::new(
            reader,
            BTreeMap::from([(template, hidden)]),
        ))
        .unwrap();
    log.commit(&mut e, second).unwrap();
    assert!(log
        .events()
        .iter()
        .all(|e| e.footprint.read.contains(&hidden.into())));
    assert!(
        DependencyOrder::extract(log.events(), AnalysisLimits::default())
            .unwrap()
            .edges()
            .is_empty()
    );
}
#[test]
fn deleted_rule_representation_and_revision_are_retained_in_history() {
    let mut g = Graph::new();
    let target = rule(&mut g, 0, false, false);
    let rep = graph_core::validate(&g).unwrap().rules[&target]
        .representation
        .clone();
    let source = rule(&mut g, 0, true, true);
    let mut lhs = vec![];
    let mut binding = BTreeMap::new();
    for id in &rep {
        let sort = g.get(*id).unwrap().kind.sort();
        let v = variable(&mut g, sort);
        lhs.push(v);
        binding.insert(v, *id);
    }
    sides(&mut g, source, &lhs, &[]);
    let mut e = Execution::new(g, ExecutionLimits::default()).unwrap();
    let mut log = Recording::new(&e, RecordingLimits::default()).unwrap();
    let p = e.prepare(OperationalBinding::new(source, binding)).unwrap();
    log.commit(&mut e, p).unwrap();
    assert!(log.events()[0].footprint.exited.contains(&target.into()));
    for id in rep {
        assert!(e.graph().get(id).is_none());
        assert!(log.atom(id.into()).is_some());
    }
}
