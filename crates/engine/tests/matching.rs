mod support;
use engine::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{
    validate, AtomKey, Binding, ErrorKind, Graph, Kind, LimitKind, NewAtom, Role, Sort, Target,
};
use std::collections::{BTreeMap, BTreeSet};
use support::*;

fn execution(graph: Graph) -> Execution {
    Execution::new(graph, ExecutionLimits::default()).unwrap()
}
fn maps(bindings: Vec<OperationalBinding>) -> BTreeSet<Vec<(AtomKey, AtomKey)>> {
    bindings
        .into_iter()
        .map(|b| b.assignments().iter().map(|(a, b)| (*a, *b)).collect())
        .collect()
}

// Deliberately naive exhaustive injective assignments, with semantic checks only
// at complete leaves. No production candidate domains, ordering or pruning reused.
fn exhaustive(graph: &Graph, source: AtomKey) -> BTreeSet<Vec<(AtomKey, AtomKey)>> {
    let state = validate(graph).unwrap();
    let rule = &state.rules[&source];
    if !rule.active {
        return BTreeSet::new();
    }
    let hidden: BTreeSet<_> = state
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    let host: Vec<_> = graph.iter().map(|(k, _)| k).collect();
    let lhs: Vec<_> = rule.lhs.iter().copied().collect();
    assert!(host.len() <= 32 && lhs.len() <= 4);
    let mut result = BTreeSet::new();
    let mut indices = vec![0; lhs.len()];
    loop {
        let targets: Vec<_> = indices.iter().map(|i| host[*i]).collect();
        if targets.iter().copied().collect::<BTreeSet<_>>().len() == targets.len() {
            let b: BTreeMap<_, _> = lhs.iter().copied().zip(targets).collect();
            let valid = b.iter().all(|(p, t)| {
                let pattern = graph.get(*p).unwrap();
                let target = graph.get(*t).unwrap();
                (rule.meta || !hidden.contains(t))
                    && match &pattern.kind {
                        Kind::PatternVariable { admits, binding } => {
                            admits.contains(&target.kind.sort())
                                && (*binding == Binding::Free || *t == source)
                        }
                        _ => {
                            pattern.kind == target.kind
                                && pattern.fields.len() == target.fields.len()
                                && pattern
                                    .fields
                                    .iter()
                                    .zip(&target.fields)
                                    .all(|(a, b_target)| b.get(a) == Some(b_target))
                        }
                    }
            });
            if valid {
                result.insert(b.into_iter().collect());
            }
        }
        let mut position = 0;
        while position < indices.len() {
            indices[position] += 1;
            if indices[position] < host.len() {
                break;
            }
            indices[position] = 0;
            position += 1;
        }
        if position == indices.len() {
            break;
        }
    }
    result
}
fn differential(graph: &Graph, source: AtomKey) {
    let run = execution(graph.clone());
    let found = run.bindings(source).unwrap();
    assert_eq!(
        found.len(),
        maps(found.clone()).len(),
        "duplicate assignments"
    );
    assert_eq!(maps(found), exhaustive(graph, source));
}

#[test]
fn all_parallel_resources_and_shared_occurrences_have_exact_expected_bindings() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    let edge = link(&mut g, "loop", x, x);
    sides(&mut g, h, &[x, edge], &[x, edge]);
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    let aa = link(&mut g, "loop", a, a);
    let aa2 = link(&mut g, "loop", a, a);
    let bb = link(&mut g, "loop", b, b);
    link(&mut g, "loop", a, b);
    link(&mut g, "wrong", a, a);
    let found = execution(g.clone()).bindings(h).unwrap();
    let expected = [(a, aa), (a, aa2), (b, bb)]
        .into_iter()
        .map(|(v, e)| vec![(x, v), (edge, e)])
        .collect();
    assert_eq!(maps(found), expected);
    differential(&g, h);
}

#[test]
fn backtracking_restores_failed_branches_and_checks_field_direction() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = ctor(&mut g, "X");
    let y = variable(&mut g, Sort::Constructor);
    let e = link(&mut g, "next", x, y);
    sides(&mut g, h, &[x, y, e], &[x, y, e]);
    let dead = ctor(&mut g, "X");
    let good = ctor(&mut g, "X");
    let target = ctor(&mut g, "Y");
    link(&mut g, "next", target, dead);
    let expected = link(&mut g, "next", good, target);
    assert_eq!(
        maps(execution(g.clone()).bindings(h).unwrap()),
        BTreeSet::from([vec![(x, good), (y, target), (e, expected)]])
    );
    differential(&g, h);
}

#[test]
fn disconnected_patterns_enumerate_injective_permutations_not_aliases() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    let y = variable(&mut g, Sort::Constructor);
    sides(&mut g, h, &[x, y], &[x, y]);
    for label in ["A", "A", "B"] {
        ctor(&mut g, label);
    }
    assert_eq!(execution(g.clone()).bindings(h).unwrap().len(), 6);
    differential(&g, h);
}

#[test]
fn empty_lhs_inactive_and_quoted_sources_are_distinguished() {
    let f = graph_core::fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let run = execution(f.graph);
    let source = f.names["iw"];
    assert_eq!(
        run.bindings(source).unwrap(),
        vec![OperationalBinding::new(source, BTreeMap::new())]
    );
    assert_eq!(run.enabled_for(source).unwrap().steps().len(), 1);
    assert_eq!(
        run.bindings(f.names["wh"]).unwrap_err().kind(),
        ErrorKind::InvalidBinding
    );
    let mut g = Graph::new();
    let inactive = rule(&mut g, 0, false, false);
    assert!(execution(g).bindings(inactive).unwrap().is_empty());
    assert!(execution(Graph::new())
        .enabled()
        .unwrap()
        .steps()
        .is_empty());
}

#[test]
fn applied_rule_is_prebound_and_multiple_self_variables_cannot_alias() {
    for count in [1, 2] {
        let mut g = Graph::new();
        let h = rule(&mut g, 0, true, true);
        rule(&mut g, 0, true, true);
        let vars: Vec<_> = (0..count)
            .map(|_| {
                g.insert(Kind::variable(Sort::RuleHead, Binding::AppliedRule), &[])
                    .unwrap()
            })
            .collect();
        sides(&mut g, h, &vars, &vars);
        let matches = execution(g.clone()).bindings(h).unwrap();
        if count == 1 {
            assert_eq!(matches[0].assignments(), &BTreeMap::from([(vars[0], h)]));
        } else {
            assert!(matches.is_empty());
        }
        differential(&g, h);
    }
}

#[test]
fn cyclic_nat_patterns_are_finite_and_arity_is_exact() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, true, true);
    let cycle = |g: &mut Graph| {
        g.insert_batch(&[
            NewAtom {
                kind: Kind::Nat,
                fields: vec![Target::Fresh(1)],
            },
            NewAtom {
                kind: Kind::Nat,
                fields: vec![Target::Fresh(0)],
            },
        ])
        .unwrap()
    };
    let pattern = cycle(&mut g);
    sides(&mut g, h, &pattern, &pattern);
    cycle(&mut g);
    g.insert(Kind::Nat, &[]).unwrap();
    assert_eq!(execution(g.clone()).bindings(h).unwrap().len(), 4);
    differential(&g, h);
}

#[test]
fn meta_sees_quoted_and_operative_records_object_hides_inactive_representations() {
    for meta in [false, true] {
        let mut g = Graph::new();
        let h = rule(&mut g, 0, meta, true);
        let x = variable(&mut g, Sort::Constructor);
        sides(&mut g, h, &[x], &[x]);
        let hidden_head = rule(&mut g, 0, false, false);
        let hidden = ctor(&mut g, "secret");
        sides(&mut g, hidden_head, &[hidden], &[hidden]);
        let public = ctor(&mut g, "public");
        let found = execution(g.clone()).bindings(h).unwrap();
        assert_eq!(found.len(), if meta { 2 } else { 1 });
        assert!(found.iter().any(|b| b.assignments()[&x] == public));
        differential(&g, h);
    }
    let mut g = Graph::new();
    let h = rule(&mut g, 0, true, true);
    let v = variable(&mut g, Sort::Record(Role::Lhs));
    sides(&mut g, h, &[v], &[v]);
    assert_eq!(execution(g.clone()).bindings(h).unwrap().len(), 1);
    differential(&g, h);
}

#[test]
fn raw_visible_link_can_fail_hidden_support_grounding() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let v = variable(&mut g, Sort::Record(Role::ObjectLink));
    sides(&mut g, h, &[v], &[v]);
    let inactive = rule(&mut g, 0, false, false);
    let secret = ctor(&mut g, "S");
    sides(&mut g, inactive, &[secret], &[secret]);
    let public = ctor(&mut g, "P");
    link(&mut g, "edge", public, secret);
    let run = execution(g);
    assert_eq!(run.bindings(h).unwrap().len(), 1);
    let enabled = run.enabled().unwrap();
    assert!(enabled.steps().is_empty());
    assert_eq!(
        enabled.rejected()[0].diagnostic.kind(),
        ErrorKind::InvalidBinding
    );
}

#[test]
fn variable_sort_unions_and_wrong_constructor_labels_are_respected() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, true, true);
    let v = g
        .insert(
            Kind::PatternVariable {
                admits: BTreeSet::from([Sort::Constructor, Sort::Nat]),
                binding: Binding::Free,
            },
            &[],
        )
        .unwrap();
    sides(&mut g, h, &[v], &[v]);
    ctor(&mut g, "X");
    let found = execution(g.clone()).bindings(h).unwrap();
    assert_eq!(found.len(), 2);
    differential(&g, h);
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = ctor(&mut g, "Wanted");
    sides(&mut g, h, &[x], &[x]);
    ctor(&mut g, "Wrong");
    assert!(execution(g.clone()).bindings(h).unwrap().is_empty());
    differential(&g, h);
}

#[test]
fn exhaustive_fixed_seed_differential_matching_and_nonmatching_inputs() {
    let mut seed = 0x52_67_u64;
    for case in 0..48 {
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed
        };
        let mut g = Graph::new();
        let h = rule(&mut g, 0, case % 2 == 0, true);
        let x = variable(&mut g, Sort::Constructor);
        let y = if case % 3 == 0 {
            ctor(&mut g, "fixed")
        } else {
            variable(&mut g, Sort::Constructor)
        };
        let e = link(&mut g, "edge", x, y);
        sides(&mut g, h, &[x, y, e], &[x, y, e]);
        let nodes: Vec<_> = (0..3)
            .map(|_| ctor(&mut g, if next() % 2 == 0 { "fixed" } else { "other" }))
            .collect();
        for _ in 0..3 {
            let a = (next() % 3) as usize;
            let b = (next() % 3) as usize;
            link(
                &mut g,
                if next() % 2 == 0 { "edge" } else { "wrong" },
                nodes[a],
                nodes[b],
            );
        }
        differential(&g, h);
    }
}

#[test]
fn budgets_are_cumulative_sticky_and_never_return_partial_enabled_sets() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    sides(&mut g, h, &[x], &[x]);
    ctor(&mut g, "A");
    ctor(&mut g, "B");
    let mut limits = ExecutionLimits::default();
    limits.search.max_candidates = 1;
    let run = Execution::new(g.clone(), limits).unwrap();
    let before = run.graph().snapshot();
    let mut search = run.search();
    assert_eq!(
        search.enabled().unwrap_err().context().limit,
        Some(LimitKind::Candidates)
    );
    assert!(search.bindings(h).is_err());
    assert!(search.finish().is_err());
    assert_eq!(before, run.graph().snapshot());
    limits.search.max_candidates = 2;
    let run = Execution::new(g.clone(), limits).unwrap();
    let mut search = run.search();
    assert_eq!(search.bindings(h).unwrap().len(), 2);
    assert!(search.bindings(h).is_err());
    assert!(search.finish().is_err());
    limits.search.max_work = 0;
    let run = Execution::new(g, limits).unwrap();
    assert_eq!(
        run.enabled().unwrap_err().context().limit,
        Some(LimitKind::SearchWork)
    );
}

#[test]
fn candidate_poststate_resource_failure_aborts_instead_of_rejecting_a_match() {
    let f = graph_core::fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let mut limits = ExecutionLimits::default();
    limits.graph.max_atoms = f.graph.len();
    let run = Execution::new(f.graph, limits).unwrap();
    let mut search = run.search();
    assert_eq!(
        search.enabled().unwrap_err().context().limit,
        Some(LimitKind::Atoms)
    );
    assert!(search.finish().is_err());
}

#[test]
fn enabled_set_combines_sources_once_and_candidate_order_is_repeatable() {
    let mut g = Graph::new();
    let a = rule(&mut g, 0, false, true);
    let b = rule(&mut g, 0, false, true);
    let old = owned(&g, a, Role::Revision);
    let mut run = execution(g);
    let first = run.enabled().unwrap();
    let second = run.enabled().unwrap();
    let keys = |set: &engine::EnabledSet| {
        set.steps()
            .iter()
            .map(|p| p.binding().source())
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&first), vec![a, b]);
    assert_eq!(keys(&first), keys(&second));
    let mut steps = first.into_steps().into_iter();
    run.commit(steps.next().unwrap()).unwrap();
    assert_eq!(
        run.commit(steps.next().unwrap()).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert_eq!(owned(run.graph(), a, Role::Revision), old);
    assert_eq!(run.committed_steps(), 1);
}
