use engine::apply_with_binding;

#[test]
fn consuming_a_variable_bound_record_preserves_its_completed_support() {
    let mut g = Graph::new();
    let source = rule(&mut g, 0, false, true);
    let edge = variable(&mut g, Sort::Record(Role::ObjectLink));
    sides(&mut g, source, &[edge], &[]);
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    let target = link(&mut g, "next", a, b);
    let parallel = link(&mut g, "next", a, b);
    let representation = validate(&g).unwrap().rules[&source].representation.clone();
    let before = g.snapshot();
    let event = apply_with_binding(&mut g, source, &BTreeMap::from([(edge, target)])).unwrap();
    assert_eq!(event.consumed, BTreeSet::from([target]));
    assert_eq!(event.preserved, BTreeSet::from([a, b]));
    assert!(event.produced.is_empty());
    let expected_reads = representation
        .union(&BTreeSet::from([target, a, b]))
        .copied()
        .collect();
    assert_eq!(event.read, expected_reads);
    for (id, atom) in before {
        if id != target {
            assert_eq!(g.get(id), Some(&atom));
        }
    }
    assert!(g.get(target).is_none());
    assert!(g.get(parallel).is_some());
}

#[test]
fn preserved_context_cannot_point_to_an_explicitly_consumed_atom() {
    let mut g = Graph::new();
    let source = rule(&mut g, 0, true, true);
    let keep = variable(&mut g, Sort::Nat);
    let delete = variable(&mut g, Sort::Nat);
    sides(&mut g, source, &[keep, delete], &[keep]);
    let zero = g.insert(Kind::Nat, &[]).unwrap();
    let one = g.insert(Kind::Nat, &[zero]).unwrap();
    let before = g.snapshot();
    let error = apply_with_binding(
        &mut g,
        source,
        &BTreeMap::from([(keep, one), (delete, zero)]),
    )
    .unwrap_err();
    assert_eq!(error.0, "dangling reference");
    assert_eq!(g.snapshot(), before);
}

#[test]
fn object_visibility_checks_completed_context_of_inactive_representations() {
    let mut g = Graph::new();
    let source = rule(&mut g, 0, false, true);
    let edge = variable(&mut g, Sort::Record(Role::ObjectLink));
    sides(&mut g, source, &[edge], &[edge]);
    let inactive = rule(&mut g, 0, false, false);
    let hidden = ctor(&mut g, "Hidden");
    sides(&mut g, inactive, &[hidden], &[hidden]);
    let visible = ctor(&mut g, "Visible");
    let target = link(&mut g, "next", visible, hidden);
    let config = validate(&g).unwrap();
    assert!(!config.rules[&inactive].representation.contains(&target));
    let before = g.snapshot();
    let error = apply_with_binding(&mut g, source, &BTreeMap::from([(edge, target)])).unwrap_err();
    assert_eq!(error.0, "object grounding enters rule representation");
    assert_eq!(g.snapshot(), before);
}

#[test]
fn same_shape_metadata_replacement_requires_a_fresh_revision() {
    for replace_revision in [false, true] {
        let mut g = Graph::new();
        let target = rule(&mut g, 0, false, true);
        let source = rule(&mut g, 1, true, true);
        let h = variable(&mut g, Sort::RuleHead);
        let old_mode = record(&mut g, Role::ModeObject, &[h]);
        let new_mode = record(&mut g, Role::ModeObject, &[h]);
        let old_rev = record(&mut g, Role::Revision, &[h]);
        let mut rhs = vec![h, new_mode];
        rhs.push(if replace_revision {
            record(&mut g, Role::Revision, &[h])
        } else {
            old_rev
        });
        sides(&mut g, source, &[h, old_mode, old_rev], &rhs);
        let b = BTreeMap::from([
            (h, target),
            (old_mode, owned(&g, target, Role::ModeObject)),
            (old_rev, owned(&g, target, Role::Revision)),
        ]);
        let before = g.snapshot();
        let result = apply_with_binding(&mut g, source, &b);
        if replace_revision {
            let event = result.unwrap();
            assert_eq!(event.altered, BTreeSet::from([target]));
            let view = &validate(&g).unwrap().rules[&target];
            assert!(!view.meta);
            assert_eq!(view.rank, 0);
        } else {
            assert_eq!(
                result.unwrap_err().0,
                "alteration requires revision replacement"
            );
            assert_eq!(g.snapshot(), before);
        }
    }
}

#[test]
fn lifecycle_and_gc_check_the_second_affected_head_and_rollback_allocations() {
    for (second_rank, replace_second_revision, expected_error) in [
        (0, true, None),
        (
            1,
            true,
            Some("GC: affected post-rank is not below source pre-rank"),
        ),
        (0, false, Some("alteration requires revision replacement")),
    ] {
        let mut g = Graph::new();
        let first = rule(&mut g, 0, false, true);
        let second = rule(&mut g, second_rank, false, true);
        let source = rule(&mut g, 1, true, true);
        let mut lhs = Vec::new();
        let mut rhs = Vec::new();
        let mut b = BTreeMap::new();
        for target in [first, second] {
            let h = variable(&mut g, Sort::RuleHead);
            let old_mode = record(&mut g, Role::ModeObject, &[h]);
            let new_mode = record(&mut g, Role::ModeMeta, &[h]);
            let old_rev = record(&mut g, Role::Revision, &[h]);
            let new_rev = if target == second && !replace_second_revision {
                old_rev
            } else {
                record(&mut g, Role::Revision, &[h])
            };
            lhs.extend([h, old_mode, old_rev]);
            rhs.extend([h, new_mode, new_rev]);
            b.extend([
                (h, target),
                (old_mode, owned(&g, target, Role::ModeObject)),
                (old_rev, owned(&g, target, Role::Revision)),
            ]);
        }
        sides(&mut g, source, &lhs, &rhs);
        let before = g.snapshot();
        let mut probe = g.clone();
        let next = probe.insert(Kind::Nat, &[]).unwrap();
        let result = apply_with_binding(&mut g, source, &b);
        if let Some(expected) = expected_error {
            assert_eq!(result.unwrap_err().0, expected);
            assert_eq!(g.snapshot(), before);
            assert_eq!(g.insert(Kind::Nat, &[]).unwrap(), next);
        } else {
            assert_eq!(result.unwrap().altered, BTreeSet::from([first, second]));
        }
    }
}

#[test]
fn caller_edited_rule_views_do_not_authorise_execution() {
    let mut g = Graph::new();
    let source = rule(&mut g, 0, false, false);
    let mut detached = validate(&g).unwrap();
    detached.rules.get_mut(&source).unwrap().active = true;
    let before = g.snapshot();
    assert_eq!(
        apply_with_binding(&mut g, source, &BTreeMap::new())
            .unwrap_err()
            .0,
        "source is inactive"
    );
    assert_eq!(g.snapshot(), before);
}

#[test]
fn c3_safe_rank_preserving_annotation_is_conservatively_rejected() {
    let mut g = Graph::new();
    let source = rule(&mut g, 0, true, true);
    let target = rule(&mut g, 0, false, true);
    let h = variable(&mut g, Sort::RuleHead);
    let mode = record(&mut g, Role::ModeObject, &[h]);
    let old = record(&mut g, Role::Revision, &[h]);
    let note = ctor(&mut g, "Audited");
    let add = record(&mut g, Role::Rhs, &[h, note]);
    let new = record(&mut g, Role::Revision, &[h]);
    sides(&mut g, source, &[h, mode, old], &[h, mode, note, add, new]);
    let binding = BTreeMap::from([
        (h, target),
        (mode, owned(&g, target, Role::ModeObject)),
        (old, owned(&g, target, Role::Revision)),
    ]);
    let before = g.snapshot();
    assert!(apply_with_binding(&mut g, source, &binding)
        .unwrap_err()
        .0
        .starts_with("GC:"));
    assert_eq!(before, g.snapshot());
}

#[test]
fn a5_object_growth_does_not_consume_meta_rank() {
    let mut g = Graph::new();
    let source = rule(&mut g, 0, false, true);
    let tick = ctor(&mut g, "Tick");
    sides(&mut g, source, &[], &[tick]);
    let initial = g.len();
    let revision = validate(&g).unwrap().rules[&source].revision;
    for step in 1..=32 {
        let event = apply_with_binding(&mut g, source, &BTreeMap::new()).unwrap();
        assert!(event.altered.is_empty() && event.entered.is_empty());
        assert_eq!(event.source_revision, revision);
        assert_eq!(g.len(), initial + step);
    }
    // The unchanged empty LHS remains enabled; a finite run alone is not a divergence proof.
}

struct Lowerer {
    head: AtomKey,
    h: AtomKey,
    zero: AtomKey,
    one: AtomKey,
    rank: AtomKey,
    revision: AtomKey,
    note: AtomKey,
    note_l: AtomKey,
    note_r: AtomKey,
}

fn lowerer(g: &mut Graph, label: &str, own_label: &str) -> Lowerer {
    let head = rule(g, 1, true, true);
    let h = variable(g, Sort::RuleHead);
    let zero = g.insert(Kind::Nat, &[]).unwrap();
    let one = g.insert(Kind::Nat, &[zero]).unwrap();
    let rank = record(g, Role::Rank, &[h, one]);
    let revision = record(g, Role::Revision, &[h]);
    let note = ctor(g, label);
    let note_l = record(g, Role::Lhs, &[h, note]);
    let note_r = record(g, Role::Rhs, &[h, note]);
    let own = ctor(g, own_label);
    let fresh = ctor(g, label);
    let fresh_l = record(g, Role::Lhs, &[h, fresh]);
    let fresh_r = record(g, Role::Rhs, &[h, fresh]);
    let new_rank = record(g, Role::Rank, &[h, zero]);
    let new_revision = record(g, Role::Revision, &[h]);
    sides(
        g,
        head,
        &[h, zero, one, rank, revision, note, note_l, note_r, own],
        &[
            h,
            zero,
            own,
            fresh,
            fresh_l,
            fresh_r,
            new_rank,
            new_revision,
        ],
    );
    Lowerer {
        head,
        h,
        zero,
        one,
        rank,
        revision,
        note,
        note_l,
        note_r,
    }
}

fn lower_binding(g: &Graph, source: &Lowerer, target: AtomKey) -> BTreeMap<AtomKey, AtomKey> {
    let configuration = validate(g).unwrap();
    let s = &configuration.rules[&source.head];
    let t = &configuration.rules[&target];
    let own = *s
        .lhs
        .intersection(&s.rhs)
        .find(|id| matches!(g.get(**id).unwrap().kind, Kind::Constructor(_)))
        .unwrap();
    let note = *t
        .lhs
        .intersection(&t.rhs)
        .find(|id| matches!(g.get(**id).unwrap().kind, Kind::Constructor(_)))
        .unwrap();
    let membership = |role| {
        g.iter()
            .find(|(_, a)| a.kind.role() == Some(role) && a.fields == [target, note])
            .unwrap()
            .0
    };
    let rank = owned(g, target, Role::Rank);
    let one = g.get(rank).unwrap().fields[1];
    let zero = g.get(one).unwrap().fields[0];
    BTreeMap::from([
        (source.h, target),
        (source.zero, zero),
        (source.one, one),
        (source.rank, rank),
        (source.revision, t.revision),
        (source.note, note),
        (source.note_l, membership(Role::Lhs)),
        (source.note_r, membership(Role::Rhs)),
        (own, own),
    ])
}

#[test]
fn c2_restoration_reattributes_revision_but_preserves_broader_causal_reads() {
    let mut g = Graph::new();
    let p = lowerer(&mut g, "NoteByP", "NoteByQ");
    let q = lowerer(&mut g, "NoteByQ", "NoteByP");
    let supervisor = rule(&mut g, 2, true, true);
    let h = variable(&mut g, Sort::RuleHead);
    let zero = g.insert(Kind::Nat, &[]).unwrap();
    let old_rank = record(&mut g, Role::Rank, &[h, zero]);
    let old_rev = record(&mut g, Role::Revision, &[h]);
    let one = g.insert(Kind::Nat, &[zero]).unwrap();
    let rank = record(&mut g, Role::Rank, &[h, one]);
    let rev = record(&mut g, Role::Revision, &[h]);
    sides(
        &mut g,
        supervisor,
        &[h, zero, old_rank, old_rev],
        &[h, zero, one, rank, rev],
    );
    validate(&g).unwrap();
    let initial = (g.len(), g.field_count());
    let mut expected_source_revision = None;
    let mut persistent_note = None;
    for _ in 0..8 {
        for (source, target) in [(&p, q.head), (&q, p.head)] {
            let b = lower_binding(&g, source, target);
            let lowered = apply_with_binding(&mut g, source.head, &b).unwrap();
            if let Some(expected) = expected_source_revision {
                assert_eq!(lowered.source_revision, expected);
            }
            if let Some(note) = persistent_note {
                assert!(lowered.read.contains(&note));
            }
            let current = validate(&g).unwrap();
            let target_revision = current.rules[&target].revision;
            let note = *lowered
                .produced
                .iter()
                .find(|id| matches!(g.get(**id).unwrap().kind, Kind::Constructor(_)))
                .unwrap();
            assert_eq!(current.quoted[&note], target);
            let target_rank = owned(&g, target, Role::Rank);
            let target_zero = g.get(target_rank).unwrap().fields[1];
            let b = BTreeMap::from([
                (h, target),
                (zero, target_zero),
                (old_rank, target_rank),
                (old_rev, target_revision),
            ]);
            let restored = apply_with_binding(&mut g, supervisor, &b).unwrap();
            let next_revision = validate(&g).unwrap().rules[&target].revision;
            assert!(restored.consumed.contains(&target_revision));
            assert!(g.get(target_revision).is_none());
            assert!(restored.produced.contains(&next_revision));
            assert!(g.get(note).is_some());
            assert_eq!((g.len(), g.field_count()), initial);
            expected_source_revision = Some(next_revision);
            persistent_note = Some(note);
        }
    }
}

#[test]
fn quoting_a_live_head_consumes_its_executable_revision() {
    for replace in [false, true] {
        let mut g = Graph::new();
        let p = rule(&mut g, 0, true, false);
        let q = rule(&mut g, 0, true, false);
        let package = validate(&g).unwrap().rules[&q].representation.clone();
        let old_q = owned(&g, q, Role::Revision);
        let old_p = owned(&g, p, Role::Revision);
        let source = rule(&mut g, 1, true, true);
        let pv = variable(&mut g, Sort::RuleHead);
        let mut l = vec![pv];
        let mut r = vec![pv];
        let mut b = BTreeMap::from([(pv, p)]);
        let mut qv = None;
        for id in package {
            let sort = g.get(id).unwrap().kind.sort();
            let v = variable(&mut g, sort);
            l.push(v);
            b.insert(v, id);
            if id == q {
                qv = Some(v);
            }
            if id != old_q || !replace {
                r.push(v);
                r.push(record(&mut g, Role::Rhs, &[pv, v]));
            }
        }
        if replace {
            let rev = record(&mut g, Role::Revision, &[qv.unwrap()]);
            r.extend([rev, record(&mut g, Role::Rhs, &[pv, rev])]);
        }
        let rev = record(&mut g, Role::Revision, &[pv]);
        l.push(rev);
        b.insert(rev, old_p);
        r.push(record(&mut g, Role::Revision, &[pv]));
        sides(&mut g, source, &l, &r);
        validate(&g).unwrap();
        let before = g.snapshot();
        let result = apply_with_binding(&mut g, source, &b);
        if replace {
            let event = result.unwrap();
            assert_eq!(event.exited, BTreeSet::from([q]));
            assert!(g.get(old_q).is_none());
            assert_eq!(validate(&g).unwrap().quoted[&q], p);
        } else {
            assert!(result.unwrap_err().0.contains("exit must consume"));
            assert_eq!(before, g.snapshot());
        }
    }
}

#[test]
fn mode_escalation_is_revision_sensitive_and_rank_guarded() {
    let mut g = Graph::new();
    let target = rule(&mut g, 0, false, true);
    let source = rule(&mut g, 1, true, true);
    let v = variable(&mut g, Sort::RuleHead);
    let mode = record(&mut g, Role::ModeObject, &[v]);
    let rev = record(&mut g, Role::Revision, &[v]);
    let meta = record(&mut g, Role::ModeMeta, &[v]);
    let fresh_rev = record(&mut g, Role::Revision, &[v]);
    sides(&mut g, source, &[v, mode, rev], &[v, meta, fresh_rev]);
    let b = BTreeMap::from([
        (v, target),
        (mode, owned(&g, target, Role::ModeObject)),
        (rev, owned(&g, target, Role::Revision)),
    ]);
    let event = apply_with_binding(&mut g, source, &b).unwrap();
    assert!(validate(&g).unwrap().rules[&target].meta);
    assert_eq!(event.altered, BTreeSet::from([target]));
}

#[test]
fn token_only_churn_is_not_a_substantive_alteration() {
    let mut g = Graph::new();
    let target = rule(&mut g, 0, false, true);
    let source = rule(&mut g, 1, true, true);
    let v = variable(&mut g, Sort::RuleHead);
    let old = record(&mut g, Role::Revision, &[v]);
    let new = record(&mut g, Role::Revision, &[v]);
    sides(&mut g, source, &[v, old], &[v, new]);
    let b = BTreeMap::from([(v, target), (old, owned(&g, target, Role::Revision))]);
    let before = g.snapshot();
    assert!(apply_with_binding(&mut g, source, &b)
        .unwrap_err()
        .0
        .contains("cannot churn"));
    assert_eq!(before, g.snapshot());
}
use graph_core::{
    fixture::{self, Fixture},
    validate, AtomKey, Binding, Graph, Kind, Role, Sort,
};
use std::collections::{BTreeMap, BTreeSet};

fn binding(f: &Fixture, pairs: &[(&str, &str)]) -> BTreeMap<AtomKey, AtomKey> {
    pairs
        .iter()
        .map(|(p, t)| (f.names[*p], f.names[*t]))
        .collect()
}
fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> AtomKey {
    g.insert(Kind::record(role), fields).unwrap()
}
fn variable(g: &mut Graph, sort: Sort) -> AtomKey {
    g.insert(Kind::variable(sort, Binding::Free), &[]).unwrap()
}
fn ctor(g: &mut Graph, label: &str) -> AtomKey {
    g.insert(Kind::Constructor(label.into()), &[]).unwrap()
}
fn link(g: &mut Graph, label: &str, a: AtomKey, b: AtomKey) -> AtomKey {
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(label.into()),
        },
        &[a, b],
    )
    .unwrap()
}
fn rule(g: &mut Graph, rank: usize, meta: bool, active: bool) -> AtomKey {
    let h = g.insert(Kind::RuleHead, &[]).unwrap();
    let mut n = g.insert(Kind::Nat, &[]).unwrap();
    for _ in 0..rank {
        n = g.insert(Kind::Nat, &[n]).unwrap();
    }
    record(g, Role::Rank, &[h, n]);
    record(g, Role::Revision, &[h]);
    record(
        g,
        if meta {
            Role::ModeMeta
        } else {
            Role::ModeObject
        },
        &[h],
    );
    if active {
        record(g, Role::Active, &[h]);
    }
    h
}
fn sides(g: &mut Graph, h: AtomKey, l: &[AtomKey], r: &[AtomKey]) {
    for a in l {
        record(g, Role::Lhs, &[h, *a]);
    }
    for a in r {
        record(g, Role::Rhs, &[h, *a]);
    }
}
fn owned(g: &Graph, h: AtomKey, role: Role) -> AtomKey {
    g.iter()
        .find(|(_, a)| a.kind.role() == Some(role) && a.fields[0] == h)
        .unwrap()
        .0
}

#[test]
fn audit_ledgers_validate_and_reproduce_counts() {
    for (text, atoms, fields, heads) in [
        (
            include_str!("../../../examples/fixtures/a2.atoms"),
            31,
            39,
            1,
        ),
        (
            include_str!("../../../examples/fixtures/a4.atoms"),
            33,
            47,
            1,
        ),
        (
            include_str!("../../../examples/fixtures/b2.atoms"),
            23,
            31,
            1,
        ),
        (
            include_str!("../../../examples/fixtures/c1.atoms"),
            81,
            117,
            2,
        ),
    ] {
        let f = fixture::load(text).unwrap();
        assert_eq!((f.graph.len(), f.graph.field_count()), (atoms, fields));
        assert_eq!(validate(&f.graph).unwrap().rules.len(), heads);
    }
}

#[test]
fn arithmetic_preserves_nonempty_result_and_rule() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/a2.atoms")).unwrap();
    let b = binding(
        &f,
        &[
            ("t_ev", "h_ev"),
            ("t_a", "h_a"),
            ("t_z", "h_z"),
            ("t_y", "h_y"),
            ("t_e0", "h_e0"),
            ("t_e1", "h_e1"),
            ("t_e2", "h_e2"),
        ],
    );
    let before = validate(&f.graph).unwrap();
    let event = apply_with_binding(&mut f.graph, f.names["az"], &b).unwrap();
    assert_eq!(
        (event.consumed.len(), event.produced.len(), f.graph.len()),
        (5, 1, 27)
    );
    assert_eq!(
        before.rules[&f.names["az"]].body,
        validate(&f.graph).unwrap().rules[&f.names["az"]].body
    );
    let hidden = &before.rules[&f.names["az"]].representation;
    let data: Vec<_> = f
        .graph
        .iter()
        .filter(|(id, _)| !hidden.contains(id))
        .collect();
    assert_eq!(data.len(), 3);
    assert!(!data
        .iter()
        .any(|(_, a)| a.kind == Kind::Constructor("Add".into())));
    // Singleton AddZero cannot match again: its required visible Add is absent.
    assert_eq!(
        data.iter()
            .filter(|(_, a)| matches!(a.kind, Kind::Constructor(_)))
            .count(),
        2
    );
}

#[test]
fn arithmetic_dangling_rejection_is_atomic() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/a2.atoms")).unwrap();
    let b = binding(
        &f,
        &[
            ("t_ev", "h_ev"),
            ("t_a", "h_a"),
            ("t_z", "h_z"),
            ("t_y", "h_y"),
            ("t_e0", "h_e0"),
            ("t_e1", "h_e1"),
            ("t_e2", "h_e2"),
        ],
    );
    let c = ctor(&mut f.graph, "Context");
    link(&mut f.graph, "child", c, f.names["h_a"]);
    let before = f.graph.snapshot();
    assert!(apply_with_binding(&mut f.graph, f.names["az"], &b)
        .unwrap_err()
        .0
        .contains("dangling"));
    assert_eq!(before, f.graph.snapshot());
}

#[test]
fn fresh_package_enters_with_literal_nested_variable() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let pre = validate(&f.graph).unwrap();
    assert!(!pre.rules.contains_key(&f.names["wh"]));
    assert!(pre.rules[&f.names["iw"]]
        .package_atoms
        .contains(&f.names["x"]));
    let event = apply_with_binding(&mut f.graph, f.names["iw"], &BTreeMap::new()).unwrap();
    assert_eq!((event.produced.len(), event.entered.len()), (13, 1));
    let after = validate(&f.graph).unwrap();
    let h = *event.entered.first().unwrap();
    assert_eq!(after.rules[&h].rank, 0);
    assert!(!after.rules[&h].meta);
    let p = *after.rules[&h].lhs.first().unwrap();
    assert!(matches!(
        f.graph.atom(p).unwrap().kind,
        Kind::PatternVariable { .. }
    ));
    let value = ctor(&mut f.graph, "Value");
    let event = apply_with_binding(&mut f.graph, h, &BTreeMap::from([(p, value)])).unwrap();
    assert_eq!(event.produced.len(), 2);
}

#[test]
fn self_lowering_works_above_one_and_reads_preserved_tail() {
    for initial_rank in 1..=5 {
        let mut f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
        let h = f.names["sl"];
        let mut top = f.names["sl_r1"];
        for _ in 1..initial_rank {
            top = f.graph.insert(Kind::Nat, &[top]).unwrap();
        }
        if initial_rank > 1 {
            f.graph
                .remove_set(&BTreeSet::from([f.names["sl_rank"]]))
                .unwrap();
            record(&mut f.graph, Role::Rank, &[h, top]);
        }
        for expected in (0..initial_rank).rev() {
            let rank = owned(&f.graph, h, Role::Rank);
            let top = f.graph.atom(rank).unwrap().fields[1];
            let rest = f.graph.atom(top).unwrap().fields[0];
            let revision = owned(&f.graph, h, Role::Revision);
            let b = BTreeMap::from([
                (f.names["b_self"], h),
                (f.names["b_rest"], rest),
                (f.names["b_top"], top),
                (f.names["b_old_rank"], rank),
                (f.names["b_old_rev"], revision),
            ]);
            let tail = f.graph.closure(&BTreeSet::from([rest])).unwrap();
            let event = apply_with_binding(&mut f.graph, h, &b).unwrap();
            assert!(tail.is_subset(&event.preserved) && tail.is_subset(&event.read));
            assert_eq!(validate(&f.graph).unwrap().rules[&h].rank, expected);
            assert_eq!(event.altered, BTreeSet::from([h]));
        }
    }
}

#[test]
fn mutual_rank_swap_fails_gc_atomically() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/c1.atoms")).unwrap();
    let b = binding(
        &f,
        &[
            ("p_self", "p"),
            ("p_other", "q"),
            ("p_self_zero", "p_r0"),
            ("p_self_one", "p_r1"),
            ("p_other_zero", "q_r0"),
            ("p_self_rank", "p_rank"),
            ("p_other_rank", "q_rank"),
            ("p_self_rev", "p_revision"),
            ("p_other_rev", "q_revision"),
            ("p_tag", "p_tag"),
        ],
    );
    let before = f.graph.snapshot();
    assert!(apply_with_binding(&mut f.graph, f.names["p"], &b)
        .unwrap_err()
        .0
        .starts_with("GC:"));
    assert_eq!(before, f.graph.snapshot());
}

fn unquote_case(
    target_rank: usize,
    replace_revision: bool,
) -> (Graph, AtomKey, AtomKey, BTreeMap<AtomKey, AtomKey>) {
    let mut g = Graph::new();
    let p = rule(&mut g, 0, true, false);
    let q = rule(&mut g, target_rank, true, false);
    let package = validate(&g).unwrap().rules[&q].representation.clone();
    let old_q_revision = owned(&g, q, Role::Revision);
    let old_p_revision = owned(&g, p, Role::Revision);
    let memberships: Vec<_> = package
        .iter()
        .map(|a| (*a, record(&mut g, Role::Rhs, &[p, *a])))
        .collect();
    let source = rule(&mut g, 1, true, true);
    let pv = variable(&mut g, Sort::RuleHead);
    let mut l = vec![pv];
    let mut r = vec![pv];
    let mut b = BTreeMap::from([(pv, p)]);
    let mut qv = None;
    for (a, membership) in memberships {
        let sort = g.atom(a).unwrap().kind.sort();
        let v = variable(&mut g, sort);
        let m = record(&mut g, Role::Rhs, &[pv, v]);
        l.extend([v, m]);
        b.extend([(v, a), (m, membership)]);
        if a != old_q_revision || !replace_revision {
            r.push(v);
        }
        if a == q {
            qv = Some(v);
        }
    }
    let old = record(&mut g, Role::Revision, &[pv]);
    l.push(old);
    b.insert(old, old_p_revision);
    r.push(record(&mut g, Role::Revision, &[pv]));
    if replace_revision {
        r.push(record(&mut g, Role::Revision, &[qv.unwrap()]));
    }
    sides(&mut g, source, &l, &r);
    validate(&g).unwrap();
    (g, source, q, b)
}

#[test]
fn preserved_quoted_head_cannot_bypass_entry_rank_check() {
    let (mut g, source, _, b) = unquote_case(2, true);
    let before = g.snapshot();
    assert!(apply_with_binding(&mut g, source, &b)
        .unwrap_err()
        .0
        .starts_with("GC:"));
    assert_eq!(before, g.snapshot());
}
#[test]
fn preserved_quoted_head_requires_new_revision_on_entry() {
    let (mut g, source, _, b) = unquote_case(0, false);
    let before = g.snapshot();
    assert!(apply_with_binding(&mut g, source, &b)
        .unwrap_err()
        .0
        .contains("entry requires fresh revision"));
    assert_eq!(before, g.snapshot());
}
#[test]
fn low_rank_unquoting_with_fresh_revision_is_admitted() {
    let (mut g, source, q, b) = unquote_case(0, true);
    let event = apply_with_binding(&mut g, source, &b).unwrap();
    assert_eq!(event.entered, BTreeSet::from([q]));
    assert!(event
        .produced
        .contains(&validate(&g).unwrap().rules[&q].revision));
}

#[test]
fn literal_self_rewrite_changes_later_object_output() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let h = f.names["sl"];
    let self_var = f.names["b_self"];
    let x = variable(&mut f.graph, Sort::Constructor);
    let old_member = record(&mut f.graph, Role::Rhs, &[self_var, x]);
    let replacement = ctor(&mut f.graph, "B");
    let attach = record(&mut f.graph, Role::Rhs, &[self_var, replacement]);
    let output = ctor(&mut f.graph, "A");
    sides(
        &mut f.graph,
        h,
        &[x, old_member],
        &[x, replacement, attach, output],
    );
    let membership = f
        .graph
        .iter()
        .find(|(_, a)| a.kind.role() == Some(Role::Rhs) && a.fields == [h, output])
        .unwrap()
        .0;
    let mut b = binding(
        &f,
        &[
            ("b_self", "sl"),
            ("b_rest", "sl_r0"),
            ("b_top", "sl_r1"),
            ("b_old_rank", "sl_rank"),
            ("b_old_rev", "sl_revision"),
        ],
    );
    b.extend([(x, output), (old_member, membership)]);
    let first = apply_with_binding(&mut f.graph, h, &b).unwrap();
    let state = validate(&f.graph).unwrap();
    let visible_output = |g: &Graph, e: &engine::Event| {
        let state = validate(g).unwrap();
        e.produced
            .iter()
            .filter(|id| !state.quoted.contains_key(id))
            .filter_map(|id| match &g.get(*id).unwrap().kind {
                Kind::Constructor(label) => Some(label.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(visible_output(&f.graph, &first), vec!["A"]);
    assert!(!state.rules[&h].rhs.contains(&output));
    let next_output = *first
        .produced
        .iter()
        .find(|id| {
            state.quoted.get(id) == Some(&h)
                && f.graph.get(**id).unwrap().kind == Kind::Constructor("B".into())
        })
        .unwrap();
    assert!(state.rules[&h].rhs.contains(&next_output));

    // A graph-resident supervisor restores rank one under its own rank-two bound.
    let supervisor = rule(&mut f.graph, 2, true, true);
    let target = variable(&mut f.graph, Sort::RuleHead);
    let zero = f.graph.insert(Kind::Nat, &[]).unwrap();
    let old_rank = record(&mut f.graph, Role::Rank, &[target, zero]);
    let old_rev = record(&mut f.graph, Role::Revision, &[target]);
    let one = f.graph.insert(Kind::Nat, &[zero]).unwrap();
    let new_rank = record(&mut f.graph, Role::Rank, &[target, one]);
    let new_rev = record(&mut f.graph, Role::Revision, &[target]);
    sides(
        &mut f.graph,
        supervisor,
        &[target, zero, old_rank, old_rev],
        &[target, zero, one, new_rank, new_rev],
    );
    let current_rank = owned(&f.graph, h, Role::Rank);
    let root = f.graph.get(current_rank).unwrap().fields[1];
    let promotion = BTreeMap::from([
        (target, h),
        (zero, root),
        (old_rank, current_rank),
        (old_rev, owned(&f.graph, h, Role::Revision)),
    ]);
    apply_with_binding(&mut f.graph, supervisor, &promotion).unwrap();
    let rank = owned(&f.graph, h, Role::Rank);
    let top = f.graph.get(rank).unwrap().fields[1];
    let membership = f
        .graph
        .iter()
        .find(|(_, a)| a.kind.role() == Some(Role::Rhs) && a.fields == [h, next_output])
        .unwrap()
        .0;
    b.extend([
        (f.names["b_top"], top),
        (f.names["b_old_rank"], rank),
        (f.names["b_old_rev"], owned(&f.graph, h, Role::Revision)),
        (x, next_output),
        (old_member, membership),
    ]);
    let second = apply_with_binding(&mut f.graph, h, &b).unwrap();
    assert_eq!(visible_output(&f.graph, &second), vec!["B"]);
    assert_eq!(validate(&f.graph).unwrap().rules[&h].rank, 0);
}

#[test]
fn linear_beta_feasibility_redirects_bound_occurrence() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let root = ctor(&mut g, "Eval");
    let outer = ctor(&mut g, "App");
    let lam = ctor(&mut g, "Lam");
    let var = ctor(&mut g, "Var");
    let body = ctor(&mut g, "App");
    let fun = variable(&mut g, Sort::Constructor);
    let arg = variable(&mut g, Sort::Constructor);
    let root_edge = link(&mut g, "expr", root, outer);
    let outer_fun = link(&mut g, "fun", outer, lam);
    let outer_arg = link(&mut g, "arg", outer, arg);
    let lam_body = link(&mut g, "body", lam, body);
    let binder = link(&mut g, "binder", var, lam);
    let body_fun = link(&mut g, "fun", body, fun);
    let body_arg = link(&mut g, "arg", body, var);
    let new_root = link(&mut g, "expr", root, body);
    let new_arg = link(&mut g, "arg", body, arg);
    let lhs = [
        root, outer, lam, var, body, fun, arg, root_edge, outer_fun, outer_arg, lam_body, binder,
        body_fun, body_arg,
    ];
    sides(
        &mut g,
        h,
        &lhs,
        &[root, body, fun, arg, body_fun, new_root, new_arg],
    );
    let mut b = BTreeMap::new();
    for p in &lhs {
        let atom = g.atom(*p).unwrap().clone();
        let kind = if *p == fun {
            Kind::Constructor("FunctionValue".into())
        } else if *p == arg {
            Kind::Constructor("ArgumentValue".into())
        } else {
            atom.kind
        };
        let fields = atom.fields.iter().map(|a| b[a]).collect::<Vec<_>>();
        b.insert(*p, g.insert(kind, &fields).unwrap());
    }
    let event = apply_with_binding(&mut g, h, &b).unwrap();
    assert_eq!(event.produced.len(), 2);
    assert!(g.get(b[&lam]).is_none() && g.get(b[&var]).is_none());
    assert!(g.iter().any(|(_, a)| a.kind
        == Kind::Record {
            role: Role::ObjectLink,
            label: Some("arg".into())
        }
        && a.fields == [b[&body], b[&arg]]));
    assert!(g.iter().any(|(_, a)| a.kind
        == Kind::Record {
            role: Role::ObjectLink,
            label: Some("expr".into())
        }
        && a.fields == [b[&root], b[&body]]));
    assert_eq!(event.altered.len(), 0);
    // This is one beta-rule shape, not a translation theorem for the lambda calculus.
}
