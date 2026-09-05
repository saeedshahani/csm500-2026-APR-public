use graph_core::{fixture, validate, Binding, Graph, Kind, NewAtom, Role, Sort, Target};
use std::collections::BTreeSet;

#[test]
fn identities_are_not_reused_and_failed_batches_are_atomic() {
    let mut g = Graph::new();
    let old = g.insert(Kind::Constructor("A".into()), &[]).unwrap();
    g.remove_set(&BTreeSet::from([old])).unwrap();
    let next = g.insert(Kind::Constructor("B".into()), &[]).unwrap();
    assert!(g.get(old).is_none());
    assert!(next.number() > old.number());
    let before = g.snapshot();
    assert!(g
        .insert_batch(&[NewAtom {
            kind: Kind::Nat,
            fields: vec![Target::Fresh(9)]
        }])
        .is_err());
    assert_eq!(before, g.snapshot());
    assert_eq!(
        g.insert(Kind::Nat, &[]).unwrap().number(),
        next.number() + 1
    );
}

#[test]
fn reverse_queries_and_closed_deletion_agree() {
    let mut g = Graph::new();
    let a = g.insert(Kind::Constructor("A".into()), &[]).unwrap();
    let b = g.insert(Kind::Constructor("B".into()), &[]).unwrap();
    let e = g
        .insert(
            Kind::Record {
                role: Role::ObjectLink,
                label: Some("next".into()),
            },
            &[a, b],
        )
        .unwrap();
    assert_eq!(g.references_to(a), BTreeSet::from([e]));
    assert!(g.remove_set(&BTreeSet::from([a])).is_err());
    g.remove_set(&BTreeSet::from([a, e])).unwrap();
    assert!(g.references_to(a).is_empty());
    assert_eq!(g.of_sort(Sort::Constructor), BTreeSet::from([b]));
}

#[test]
fn invalid_ledger_syntax_is_rejected() {
    for text in [
        "a Nat\na Nat",
        "a Nat s0=missing",
        "a Nat s1=a",
        "a Nat bogus=yes",
        "a PatternVariable admits={} binding=Free",
        "a PatternVariable admits={Constructor}",
        "a Record[ObjectLink] s0=a s1=a",
    ] {
        assert!(fixture::load(text).is_err(), "{text}");
    }
}

#[test]
fn malformed_quotations_and_heads_are_rejected() {
    let b2 = include_str!("../../../examples/fixtures/b2.atoms");
    for extra in [
        "duplicate Record[Lhs] s0=sl s1=b_self",
        "extra_mode Record[ModeObject] s0=sl",
        "bad_head RuleHead",
        "bad_rank Record[Rank] s0=sl s1=sl_r0",
    ] {
        let f = fixture::load(&format!("{b2}\n{extra}")).unwrap();
        assert!(validate(&f.graph).is_err(), "{extra}");
    }
    let cycle = "h RuleHead\na Record[Rhs] s0=h s1=b\nb Record[Rhs] s0=h s1=a";
    assert!(validate(&fixture::load(cycle).unwrap().graph).is_err());
    let a4 = include_str!("../../../examples/fixtures/a4.atoms");
    let incomplete = a4.replace("iw_R_wrev     Record[Rhs] s0=iw s1=wrev", "");
    assert!(validate(&fixture::load(&incomplete).unwrap().graph).is_err());
    let literal_lhs = format!("{a4}\nwrong Record[Lhs] s0=iw s1=wh");
    assert!(validate(&fixture::load(&literal_lhs).unwrap().graph).is_err());
}

#[test]
fn rank_cycles_and_shared_chains_are_rejected() {
    let base="a RuleHead\nz Nat\nar Record[Rank] s0=a s1=z\nam Record[ModeMeta] s0=a\nav Record[Revision] s0=a";
    assert_eq!(
        validate(&fixture::load(base).unwrap().graph)
            .unwrap()
            .rules
            .values()
            .next()
            .unwrap()
            .rank,
        0
    );
    assert!(validate(
        &fixture::load(&base.replace("z Nat", "z Nat s0=z"))
            .unwrap()
            .graph
    )
    .is_err());
    let shared=format!("{base}\nb RuleHead\nbr Record[Rank] s0=b s1=z\nbm Record[ModeMeta] s0=b\nbv Record[Revision] s0=b");
    assert!(validate(&fixture::load(&shared).unwrap().graph).is_err());
}

#[test]
fn variable_declarations_and_field_typing_are_checked() {
    let mut g = Graph::new();
    assert!(g
        .insert(Kind::variable(Sort::Nat, Binding::AppliedRule), &[])
        .is_err());
    let a = g.insert(Kind::Constructor("A".into()), &[]).unwrap();
    g.insert(Kind::Nat, &[a]).unwrap();
    assert!(validate(&g).is_err());
}

#[test]
fn two_live_heads_cannot_quote_the_same_syntax() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/c1.atoms")).unwrap();
    validate(&f.graph).unwrap();
    f.graph
        .insert(Kind::record(Role::Lhs), &[f.names["q"], f.names["p_tag"]])
        .unwrap();
    assert_eq!(validate(&f.graph).unwrap_err().0, "shared quoted syntax");
}

#[test]
fn quoted_fields_cannot_cross_owner_boundaries() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/c1.atoms")).unwrap();
    validate(&f.graph).unwrap();
    let cross = f
        .graph
        .insert(
            Kind::Record {
                role: Role::ObjectLink,
                label: Some("cross".into()),
            },
            &[f.names["p_tag"], f.names["q_tag"]],
        )
        .unwrap();
    f.graph
        .insert(Kind::record(Role::Lhs), &[f.names["p"], cross])
        .unwrap();
    assert_eq!(
        validate(&f.graph).unwrap_err().0,
        "quoted record points outside its owner"
    );
}

#[test]
fn missing_live_metadata_is_rejected_even_when_inactive() {
    for role in [Role::Rank, Role::ModeMeta, Role::Revision] {
        let mut f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
        let metadata = f
            .graph
            .iter()
            .find(|(_, a)| a.kind.role() == Some(role) && a.fields[0] == f.names["sl"])
            .unwrap()
            .0;
        f.graph
            .remove_set(&BTreeSet::from([metadata, f.names["sl_active"]]))
            .unwrap();
        assert_eq!(
            validate(&f.graph).unwrap_err().0,
            "live head lacks unique rank/mode/revision/activity"
        );
    }
}

#[test]
fn inactive_installers_still_require_complete_fresh_packages() {
    let a4 = include_str!("../../../examples/fixtures/a4.atoms");
    let mut f = fixture::load(a4).unwrap();
    f.graph
        .remove_set(&BTreeSet::from([f.names["iw_active"]]))
        .unwrap();
    validate(&f.graph).unwrap();
    f.graph
        .remove_set(&BTreeSet::from([f.names["iw_R_wrev"], f.names["wrev"]]))
        .unwrap();
    assert_eq!(
        validate(&f.graph).unwrap_err().0,
        "live head lacks unique rank/mode/revision/activity"
    );
}

#[test]
fn active_rule_rejects_unbound_rhs_variables_and_wrong_mode() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/a2.atoms")).unwrap();
    let v = f
        .graph
        .insert(Kind::variable(Sort::Constructor, Binding::Free), &[])
        .unwrap();
    let membership = f
        .graph
        .insert(Kind::record(Role::Rhs), &[f.names["az"], v])
        .unwrap();
    assert_eq!(validate(&f.graph).unwrap_err().0, "unbound RHS variable");
    f.graph
        .remove_set(&BTreeSet::from([membership, v]))
        .unwrap();
    let n = f.graph.insert(Kind::Nat, &[]).unwrap();
    f.graph
        .insert(Kind::record(Role::Rhs), &[f.names["az"], n])
        .unwrap();
    assert_eq!(
        validate(&f.graph).unwrap_err().0,
        "meta syntax in object rule"
    );
}

#[test]
fn fieldless_zero_and_successor_arity_are_enforced() {
    let mut g = Graph::new();
    let zero = g.insert(Kind::Nat, &[]).unwrap();
    let one = g.insert(Kind::Nat, &[zero]).unwrap();
    assert!(g.get(zero).unwrap().fields.is_empty());
    assert_eq!(g.get(one).unwrap().fields, vec![zero]);
    assert!(g.insert(Kind::Nat, &[zero, one]).is_err());
    validate(&g).unwrap();
}

#[test]
fn finite_object_cycles_parallel_links_and_self_links_are_valid() {
    let mut g = Graph::new();
    let a = g.insert(Kind::Constructor("A".into()), &[]).unwrap();
    let b = g.insert(Kind::Constructor("B".into()), &[]).unwrap();
    let kind = Kind::Record {
        role: Role::ObjectLink,
        label: Some("next".into()),
    };
    let first = g.insert(kind.clone(), &[a, b]).unwrap();
    let parallel = g.insert(kind.clone(), &[a, b]).unwrap();
    g.insert(kind.clone(), &[b, a]).unwrap();
    g.insert(kind, &[a, a]).unwrap();
    assert_ne!(first, parallel);
    assert_eq!(g.references_to(a).len(), 4);
    validate(&g).unwrap();
    assert_eq!(
        g.closure(&BTreeSet::from([first])).unwrap(),
        BTreeSet::from([first, a, b])
    );
}

#[test]
fn genuinely_cyclic_batch_closure_terminates_without_copying() {
    let mut g = Graph::new();
    let ids = g
        .insert_batch(&[
            NewAtom {
                kind: Kind::Nat,
                fields: vec![Target::Fresh(1)],
            },
            NewAtom {
                kind: Kind::Nat,
                fields: vec![Target::Fresh(0)],
            },
        ])
        .unwrap();
    // This tests finite carrier closure, not a valid natural-number rank.
    assert_eq!(
        g.closure(&BTreeSet::from([ids[0]])).unwrap(),
        ids.into_iter().collect()
    );
}
