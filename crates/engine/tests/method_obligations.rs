//! S6.I/M01 transfer obligations for future incremental validation.
//! These exercise the actual engine; they do not install a cache or change traces.
#[allow(dead_code)]
mod support;
use engine::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{ErrorKind, Graph, Role, Sort};
use std::collections::BTreeMap;
use support::*;

fn agreement(e: &Execution) {
    let reference = e.enabled().unwrap();
    let indexed = e.enabled_compact().unwrap();
    let mut a: Vec<_> = reference
        .steps()
        .iter()
        .map(|s| s.binding().clone())
        .collect();
    let mut b = indexed.bindings().to_vec();
    a.sort_by_key(|b| (b.source(), b.assignments().clone()));
    b.sort_by_key(|b| (b.source(), b.assignments().clone()));
    assert_eq!(a, b);
    let mut a: Vec<_> = reference
        .rejected()
        .iter()
        .map(|r| format!("{:?}:{:?}", r.binding, r.diagnostic.kind()))
        .collect();
    let mut b: Vec<_> = indexed
        .rejected()
        .iter()
        .map(|r| format!("{:?}:{:?}", r.binding, r.diagnostic.kind()))
        .collect();
    a.sort();
    b.sort();
    assert_eq!(a, b);
}

#[test]
fn unchanged_matched_atom_and_revision_do_not_certify_cached_admission() {
    let mut g = Graph::new();
    let delete = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "A");
    sides(&mut g, delete, &[p], &[]);
    let attach = rule(&mut g, 0, false, true);
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    let incoming = link(&mut g, "incoming", b, a);
    sides(&mut g, attach, &[a], &[a, b, incoming]);
    let object = ctor(&mut g, "A");
    let revision = owned(&g, delete, Role::Revision);
    let old_atom = g.get(object).unwrap().clone();
    let mut e = Execution::new(g, ExecutionLimits::default()).unwrap();
    agreement(&e);
    let stale = e.enabled_for(delete).unwrap().into_steps().pop().unwrap();
    let old_binding = stale.binding().clone();
    let edit = e.enabled_for(attach).unwrap().into_steps().pop().unwrap();
    e.commit(edit).unwrap();
    assert_eq!(e.graph().get(object).unwrap(), &old_atom);
    assert_eq!(owned(e.graph(), delete, Role::Revision), revision);
    assert!(e.bindings_indexed(delete).unwrap().contains(&old_binding));
    assert_eq!(
        e.prepare(old_binding).unwrap_err().kind(),
        ErrorKind::Dangling
    );
    assert_eq!(
        e.commit(stale).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    agreement(&e);
}

#[test]
fn cross_owner_shared_quotation_is_not_a_valid_cache_test_domain() {
    let mut g = Graph::new();
    let hidden = ctor(&mut g, "Hidden");
    for _ in 0..2 {
        let h = rule(&mut g, 0, false, false);
        sides(&mut g, h, &[], &[hidden]);
    }
    assert_eq!(
        graph_core::validate(&g).unwrap_err().kind(),
        ErrorKind::InvalidConfiguration
    );
}

#[test]
fn both_side_memberships_must_be_removed_before_object_matching() {
    let mut g = Graph::new();
    let hidden = ctor(&mut g, "Hidden");
    let owner = rule(&mut g, 0, false, false);
    sides(&mut g, owner, &[hidden], &[hidden]);
    let mut removers = Vec::new();
    for role in [Role::Lhs, Role::Rhs] {
        let expose = rule(&mut g, 1, true, true);
        let h = variable(&mut g, Sort::RuleHead);
        let c = ctor(&mut g, "Hidden");
        let membership = record(&mut g, role, &[h, c]);
        let revision = record(&mut g, Role::Revision, &[h]);
        let fresh = record(&mut g, Role::Revision, &[h]);
        sides(
            &mut g,
            expose,
            &[h, c, membership, revision],
            &[h, c, fresh],
        );
        removers.push((role, expose, h, c, membership, revision));
    }
    let reader = rule(&mut g, 0, false, true);
    let pattern = ctor(&mut g, "Hidden");
    let seen = ctor(&mut g, "Seen");
    sides(&mut g, reader, &[pattern], &[pattern, seen]);
    let reader_revision = owned(&g, reader, Role::Revision);
    let old_atom = g.get(hidden).unwrap().clone();
    let mut e = Execution::new(g, ExecutionLimits::default()).unwrap();
    for (i, (role, expose, h, c, membership, revision)) in removers.into_iter().enumerate() {
        assert!(e.bindings_indexed(reader).unwrap().is_empty());
        agreement(&e);
        let binding = OperationalBinding::new(
            expose,
            BTreeMap::from([
                (h, owner),
                (c, hidden),
                (membership, owned(e.graph(), owner, role)),
                (revision, owned(e.graph(), owner, Role::Revision)),
            ]),
        );
        let step = e.prepare(binding).unwrap();
        let event = e.commit(step).unwrap();
        assert!(event.altered.contains(&owner));
        assert_eq!(e.graph().get(hidden).unwrap(), &old_atom);
        assert_eq!(owned(e.graph(), reader, Role::Revision), reader_revision);
        assert_eq!(
            e.bindings_indexed(reader).unwrap().len(),
            usize::from(i == 1)
        );
    }
    agreement(&e);
    assert_eq!(e.enabled_for(reader).unwrap().steps().len(), 1);
}
