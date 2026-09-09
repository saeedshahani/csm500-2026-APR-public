use engine::{apply_with_binding, Execution, ExecutionLimits, OperationalBinding, SearchLimits};
use graph_core::{fixture, AtomKey, ErrorKind, Graph, GraphLimits, Kind, LimitKind, Role};
use std::collections::{BTreeMap, BTreeSet};

fn self_lower(limits: ExecutionLimits) -> (Execution, OperationalBinding) {
    let f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let assignments = [
        ("b_self", "sl"),
        ("b_rest", "sl_r0"),
        ("b_top", "sl_r1"),
        ("b_old_rank", "sl_rank"),
        ("b_old_rev", "sl_revision"),
    ]
    .into_iter()
    .map(|(p, t)| (f.names[p], f.names[t]))
    .collect();
    let binding = OperationalBinding::new(f.names["sl"], assignments);
    (Execution::new(f.graph, limits).unwrap(), binding)
}
fn next_identity(graph: &Graph) -> AtomKey {
    graph.clone().insert(Kind::Nat, &[]).unwrap()
}

#[test]
fn prepared_ground_span_matches_hand_derived_self_lowering_and_reference() {
    let (mut execution, binding) = self_lower(ExecutionLimits::default());
    let before = execution.graph().snapshot();
    let next = next_identity(execution.graph());
    let mut reference = execution.graph().clone();
    let expected =
        apply_with_binding(&mut reference, binding.source(), binding.assignments()).unwrap();
    let prepared = execution.prepare(binding.clone()).unwrap();
    assert_eq!(prepared.binding(), &binding);
    assert_eq!(prepared.event(), &expected);
    assert_eq!(prepared.event().consumed.len(), 3);
    assert_eq!(prepared.event().preserved.len(), 2);
    assert_eq!(prepared.event().produced.len(), 2);
    let ground = prepared.ground_rule();
    assert_eq!(ground.left().len(), 5);
    assert_eq!(ground.fresh().len(), 2);
    assert_eq!(ground.right().count(), 4);
    assert_eq!(
        prepared.ground_match().image(),
        &ground.left().keys().copied().collect()
    );
    assert_eq!(ground.left()[&binding.source()].kind, Kind::RuleHead);
    let right: BTreeMap<_, _> = ground.right().collect();
    for id in ground.preserved() {
        assert_eq!(ground.left().get(id), right.get(id).copied());
    }
    for atom in right.values() {
        assert!(atom.fields.iter().all(|id| right.contains_key(id)));
    }
    for id in ground.fresh().keys() {
        assert!(execution.graph().get(*id).is_none());
    }
    assert_eq!(execution.graph().snapshot(), before);
    assert_eq!(next_identity(execution.graph()), next);
    assert_eq!(execution.commit(prepared).unwrap(), expected);
    assert_eq!(execution.graph().snapshot(), reference.snapshot());
    assert_eq!(execution.committed_steps(), 1);
}

#[test]
fn fresh_package_proposal_preserves_literal_variables_and_remains_uncommitted() {
    let f = fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let binding = OperationalBinding::new(f.names["iw"], BTreeMap::new());
    let mut execution = Execution::new(f.graph, ExecutionLimits::default()).unwrap();
    let prepared = execution.prepare(binding).unwrap();
    let rule = prepared.ground_rule();
    assert!(rule.left().is_empty());
    assert!(rule.preserved().is_empty());
    assert_eq!(rule.fresh().len(), 13);
    assert_eq!(
        rule.fresh()
            .values()
            .filter(|a| matches!(a.kind, Kind::PatternVariable { .. }))
            .count(),
        1
    );
    assert_eq!(execution.view().unwrap().configuration().rules.len(), 1);
    execution.commit(prepared).unwrap();
    assert_eq!(execution.view().unwrap().configuration().rules.len(), 2);
}

#[test]
fn competing_candidates_are_stale_after_one_commits() {
    let (mut execution, binding) = self_lower(ExecutionLimits::default());
    let first = execution.prepare(binding.clone()).unwrap();
    let second = execution.prepare(binding).unwrap();
    execution.commit(first).unwrap();
    let before = execution.graph().snapshot();
    let next = next_identity(execution.graph());
    assert_eq!(
        execution.commit(second).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert_eq!(execution.graph().snapshot(), before);
    assert_eq!(next_identity(execution.graph()), next);
    assert_eq!(execution.committed_steps(), 1);
}

#[test]
fn candidates_cannot_cross_identical_execution_instances() {
    let (first, binding) = self_lower(ExecutionLimits::default());
    let mut second = Execution::new(first.graph().clone(), first.limits()).unwrap();
    let candidate = first.prepare(binding).unwrap();
    assert_eq!(
        second.commit(candidate).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert_eq!(second.committed_steps(), 0);
}

#[test]
fn replacing_with_an_identical_graph_starts_a_new_history() {
    let (mut execution, binding) = self_lower(ExecutionLimits::default());
    let old = execution.prepare(binding.clone()).unwrap();
    execution.replace_graph(execution.graph().clone()).unwrap();
    assert_eq!(
        execution.commit(old).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    let fresh = execution.prepare(binding).unwrap();
    execution.commit(fresh).unwrap();
    assert_eq!(execution.committed_steps(), 1);
    execution.replace_graph(execution.graph().clone()).unwrap();
    assert_eq!(execution.committed_steps(), 0);
}

#[test]
fn invalid_or_oversized_replacement_preserves_state_and_candidate_validity() {
    for oversized in [false, true] {
        let limits = ExecutionLimits {
            graph: GraphLimits {
                max_atoms: 23,
                ..GraphLimits::default()
            },
            ..ExecutionLimits::default()
        };
        let (mut execution, binding) = self_lower(limits);
        let candidate = execution.prepare(binding).unwrap();
        let before = execution.graph().snapshot();
        let mut replacement = if oversized {
            execution.graph().clone()
        } else {
            Graph::new()
        };
        replacement.insert(Kind::RuleHead, &[]).unwrap();
        let error = execution.replace_graph(replacement).unwrap_err();
        assert_eq!(
            error.kind(),
            if oversized {
                ErrorKind::ResourceExhaustion
            } else {
                ErrorKind::InvalidConfiguration
            }
        );
        assert_eq!(execution.graph().snapshot(), before);
        execution.commit(candidate).unwrap();
    }
}

#[test]
fn no_op_commit_also_invalidates_older_candidates() {
    let f = fixture::load("h RuleHead\nz Nat\nr Record[Rank] s0=h s1=z\nv Record[Revision] s0=h\nm Record[ModeObject] s0=h\na Record[Active] s0=h").unwrap();
    let binding = OperationalBinding::new(f.names["h"], BTreeMap::new());
    let mut execution = Execution::new(f.graph, ExecutionLimits::default()).unwrap();
    let first = execution.prepare(binding.clone()).unwrap();
    let old = execution.prepare(binding).unwrap();
    let before = execution.graph().snapshot();
    execution.commit(first).unwrap();
    assert_eq!(execution.graph().snapshot(), before);
    assert_eq!(
        execution.commit(old).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
}

#[test]
fn invalid_binding_has_structured_context_and_does_not_poison_search() {
    let (execution, binding) = self_lower(ExecutionLimits::default());
    let mut invalid = binding.assignments().clone();
    let variable = *invalid
        .keys()
        .find(|id| {
            matches!(
                execution.graph().get(**id).unwrap().kind,
                Kind::PatternVariable {
                    binding: graph_core::Binding::AppliedRule,
                    ..
                }
            )
        })
        .unwrap();
    let fresh_nat = execution
        .graph()
        .of_sort(graph_core::Sort::Nat)
        .into_iter()
        .find(|id| !invalid.values().any(|v| v == id))
        .unwrap();
    invalid.insert(variable, fresh_nat);
    let next = next_identity(execution.graph());
    let mut search = execution.search();
    let error = search
        .prepare(OperationalBinding::new(binding.source(), invalid))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidBinding);
    assert!(error.context().atoms.contains(&variable));
    assert!(error.context().atoms.contains(&binding.source()));
    search.prepare(binding).unwrap();
    assert_eq!(search.attempted_candidates(), 2);
    search.finish().unwrap();
    assert_eq!(next_identity(execution.graph()), next);
}

#[test]
fn candidate_exhaustion_is_sticky_and_never_a_complete_empty_search() {
    let limits = ExecutionLimits {
        search: SearchLimits {
            max_candidates: 1,
            max_work: 100,
        },
        ..ExecutionLimits::default()
    };
    let (execution, binding) = self_lower(limits);
    let mut search = execution.search();
    search.prepare(binding.clone()).unwrap();
    let error = search.prepare(binding.clone()).unwrap_err();
    assert_eq!(error.context().limit, Some(LimitKind::Candidates));
    assert_eq!(
        search.prepare(binding).unwrap_err().kind(),
        ErrorKind::ResourceExhaustion
    );
    assert_eq!(search.finish().unwrap_err(), error);
}

#[test]
fn search_work_limits_allow_exact_boundary_and_fail_before_preparation() {
    let limits = ExecutionLimits {
        search: SearchLimits {
            max_candidates: 2,
            max_work: 6,
        },
        ..ExecutionLimits::default()
    };
    let (execution, binding) = self_lower(limits);
    let mut exact = execution.search();
    exact.prepare(binding.clone()).unwrap();
    assert_eq!(exact.work_used(), 6);
    exact.finish().unwrap();
    let mut search = execution.search();
    search.charge_work(1).unwrap();
    assert_eq!(
        search.prepare(binding).unwrap_err().context().limit,
        Some(LimitKind::SearchWork)
    );
    assert!(search.finish().is_err());
    let zero_limits = ExecutionLimits {
        search: SearchLimits {
            max_candidates: 0,
            ..SearchLimits::default()
        },
        ..ExecutionLimits::default()
    };
    let (zero, binding) = self_lower(zero_limits);
    let next = next_identity(zero.graph());
    assert_eq!(
        zero.prepare(binding).unwrap_err().context().limit,
        Some(LimitKind::Candidates)
    );
    assert_eq!(next_identity(zero.graph()), next);
    let mut overflow = execution.search();
    assert_eq!(
        overflow
            .charge_work(usize::MAX)
            .unwrap_err()
            .context()
            .limit,
        Some(LimitKind::SearchWork)
    );
    let limits = ExecutionLimits {
        search: SearchLimits {
            max_work: usize::MAX,
            ..SearchLimits::default()
        },
        ..ExecutionLimits::default()
    };
    let (execution, _) = self_lower(limits);
    let mut overflow = execution.search();
    overflow.charge_work(usize::MAX).unwrap();
    assert_eq!(
        overflow.charge_work(1).unwrap_err().context().limit,
        Some(LimitKind::SearchWork)
    );
}

#[test]
fn post_state_resource_failure_preserves_allocator_and_marks_search_incomplete() {
    let f = fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let binding = OperationalBinding::new(f.names["iw"], BTreeMap::new());
    let limits = ExecutionLimits {
        graph: GraphLimits {
            max_atoms: f.graph.len(),
            ..GraphLimits::default()
        },
        ..ExecutionLimits::default()
    };
    let execution = Execution::new(f.graph, limits).unwrap();
    let before = execution.graph().snapshot();
    let next = next_identity(execution.graph());
    let mut search = execution.search();
    let error = search.prepare(binding).unwrap_err();
    assert_eq!(error.context().limit, Some(LimitKind::Atoms));
    assert_eq!(search.finish().unwrap_err(), error);
    assert_eq!(execution.graph().snapshot(), before);
    assert_eq!(next_identity(execution.graph()), next);
}

#[test]
fn lifecycle_and_guardedness_errors_identify_the_target_and_role() {
    for replace_mode in [false, true] {
        let source = "p RuleHead\npz Nat\npr Record[Rank] s0=p s1=pz\npm Record[ModeMeta] s0=p\npv Record[Revision] s0=p\npa Record[Active] s0=p";
        let target = "q RuleHead\nqz Nat\nqr Record[Rank] s0=q s1=qz\nqm Record[ModeObject] s0=q\nqv Record[Revision] s0=q";
        let syntax = "h PatternVariable admits={RuleHead} binding=Free\nv Record[Revision] s0=h\nnv Record[Revision] s0=h\nlh Record[Lhs] s0=p s1=h\nlv Record[Lhs] s0=p s1=v\nrh Record[Rhs] s0=p s1=h\nrv Record[Rhs] s0=p s1=nv";
        let extra = if replace_mode {
            "\nm Record[ModeObject] s0=h\nnm Record[ModeMeta] s0=h\nlm Record[Lhs] s0=p s1=m\nrm Record[Rhs] s0=p s1=nm"
        } else {
            ""
        };
        let f = fixture::load(&format!("{source}\n{target}\n{syntax}{extra}")).unwrap();
        let mut assignments =
            BTreeMap::from([(f.names["h"], f.names["q"]), (f.names["v"], f.names["qv"])]);
        if replace_mode {
            assignments.insert(f.names["m"], f.names["qm"]);
        }
        let binding = OperationalBinding::new(f.names["p"], assignments);
        let q = f.names["q"];
        let execution = Execution::new(f.graph, ExecutionLimits::default()).unwrap();
        let next = next_identity(execution.graph());
        let error = execution.prepare(binding).unwrap_err();
        assert_eq!(
            error.kind(),
            if replace_mode {
                ErrorKind::Guardedness
            } else {
                ErrorKind::Lifecycle
            }
        );
        assert!(error.context().atoms.contains(&q));
        assert_eq!(
            error.context().role,
            Some(if replace_mode {
                Role::Rank
            } else {
                Role::Revision
            })
        );
        assert_eq!(next_identity(execution.graph()), next);
    }
}

#[test]
fn completed_context_contains_support_outside_the_raw_assignment() {
    let f = fixture::load("p RuleHead\nz Nat\nr Record[Rank] s0=p s1=z\nv Record[Revision] s0=p\nm Record[ModeObject] s0=p\na Record[Active] s0=p\nx PatternVariable admits={Record[ObjectLink]} binding=Free\nl Record[Lhs] s0=p s1=x\nu Constructor[U]\nw Constructor[W]\ne Record[ObjectLink,edge] s0=u s1=w").unwrap();
    let source = f.names["p"];
    let endpoints = BTreeSet::from([f.names["u"], f.names["w"]]);
    let assignments = BTreeMap::from([(f.names["x"], f.names["e"])]);
    let execution = Execution::new(f.graph, ExecutionLimits::default()).unwrap();
    let candidate = execution
        .prepare(OperationalBinding::new(source, assignments))
        .unwrap();
    assert_eq!(candidate.binding().assignments().len(), 1);
    assert_eq!(candidate.ground_rule().preserved(), &endpoints);
    assert_eq!(candidate.ground_match().image().len(), 3);
    assert_eq!(candidate.ground_rule().right().count(), 2);
}
