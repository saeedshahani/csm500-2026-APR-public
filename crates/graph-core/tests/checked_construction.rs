use graph_core::{
    fixture, validate_with_limits, ErrorKind, Graph, GraphLimits, Kind, LimitKind, NewAtom, Role,
    Target, ValidatedGraph,
};
use std::collections::BTreeSet;

fn next_identity(graph: &Graph) -> u64 {
    graph.clone().insert(Kind::Nat, &[]).unwrap().number()
}

#[test]
fn input_limit_checks_utf8_bytes_before_ledger_parsing() {
    let limits = GraphLimits {
        max_input_bytes: 2,
        ..GraphLimits::default()
    };
    limits.check_input("é").unwrap();
    let error = fixture::load_with_limits("éx", &limits).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::ResourceExhaustion);
    assert_eq!(error.context().limit, Some(LimitKind::InputBytes));
    assert_eq!(
        fixture::load("bad").err().unwrap().kind(),
        ErrorKind::MalformedInput
    );
}

#[test]
fn atom_field_and_label_limits_reject_before_batch_allocation() {
    let cases = [
        (
            GraphLimits {
                max_atoms: 0,
                ..GraphLimits::default()
            },
            NewAtom {
                kind: Kind::Nat,
                fields: vec![],
            },
            LimitKind::Atoms,
        ),
        (
            GraphLimits {
                max_fields: 0,
                ..GraphLimits::default()
            },
            NewAtom {
                kind: Kind::Nat,
                fields: vec![Target::Fresh(0)],
            },
            LimitKind::Fields,
        ),
        (
            GraphLimits {
                max_label_bytes: 1,
                ..GraphLimits::default()
            },
            NewAtom {
                kind: Kind::Constructor("é".into()),
                fields: vec![],
            },
            LimitKind::LabelBytes,
        ),
    ];
    for (limits, item, expected) in cases {
        let mut graph = Graph::new();
        let next = next_identity(&graph);
        let error = graph
            .insert_batch_with_limits(&[item], &limits)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ResourceExhaustion);
        assert_eq!(error.context().limit, Some(expected));
        assert!(graph.is_empty());
        assert_eq!(next_identity(&graph), next);
    }
}

#[test]
fn construction_limits_count_existing_records_and_allow_exact_boundaries() {
    let limits = GraphLimits {
        max_atoms: 2,
        max_fields: 1,
        max_label_bytes: 2,
        ..GraphLimits::default()
    };
    let mut graph = Graph::new();
    graph
        .insert_batch_with_limits(
            &[NewAtom {
                kind: Kind::Constructor("é".into()),
                fields: vec![],
            }],
            &limits,
        )
        .unwrap();
    graph
        .insert_batch_with_limits(
            &[NewAtom {
                kind: Kind::Nat,
                fields: vec![],
            }],
            &limits,
        )
        .unwrap();
    validate_with_limits(&graph, &limits).unwrap();
    let before = graph.snapshot();
    let next = next_identity(&graph);
    assert_eq!(
        graph
            .insert_batch_with_limits(
                &[NewAtom {
                    kind: Kind::Nat,
                    fields: vec![]
                }],
                &limits
            )
            .unwrap_err()
            .context()
            .limit,
        Some(LimitKind::Atoms)
    );
    assert_eq!(graph.snapshot(), before);
    assert_eq!(next_identity(&graph), next);
    let label_limits = GraphLimits {
        max_atoms: 3,
        ..limits
    };
    assert_eq!(
        graph
            .insert_batch_with_limits(
                &[NewAtom {
                    kind: Kind::Constructor("X".into()),
                    fields: vec![]
                }],
                &label_limits
            )
            .unwrap_err()
            .context()
            .limit,
        Some(LimitKind::LabelBytes)
    );
}

#[test]
fn forward_references_and_object_cycles_lower_in_one_atomic_batch() {
    let graph = fixture::load("ab Record[ObjectLink,next] s0=a s1=b\nba Record[ObjectLink,next] s0=b s1=a\na Constructor[A]\nb Constructor[B]").unwrap();
    let view = ValidatedGraph::new(&graph.graph, &GraphLimits::default()).unwrap();
    assert!(view.configuration().rules.is_empty());
    assert_eq!(
        view.graph().atom(graph.names["ab"]).unwrap().fields,
        [graph.names["a"], graph.names["b"]]
    );
    assert_eq!(
        view.graph().atom(graph.names["ba"]).unwrap().fields,
        [graph.names["b"], graph.names["a"]]
    );
}

#[test]
fn failed_late_batch_item_reports_context_and_preserves_allocator() {
    let mut graph = Graph::new();
    let next = next_identity(&graph);
    let error = graph
        .insert_batch(&[
            NewAtom {
                kind: Kind::Nat,
                fields: vec![],
            },
            NewAtom {
                kind: Kind::record(Role::Rank),
                fields: vec![Target::Fresh(0), Target::Fresh(99)],
            },
        ])
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidConfiguration);
    assert_eq!(error.context().batch_index, Some(1));
    assert_eq!(error.context().role, Some(Role::Rank));
    assert_eq!(error.context().slot, Some(1));
    assert!(graph.is_empty());
    assert_eq!(next_identity(&graph), next);
}

#[test]
fn dangling_diagnostics_identify_surviving_source_target_and_slot() {
    let mut graph = Graph::new();
    let zero = graph.insert(Kind::Nat, &[]).unwrap();
    let one = graph.insert(Kind::Nat, &[zero]).unwrap();
    let before = graph.snapshot();
    let next = next_identity(&graph);
    let error = graph.remove_set(&BTreeSet::from([zero])).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Dangling);
    assert_eq!(error.context().atoms, [one, zero]);
    assert_eq!(error.context().slot, Some(0));
    assert_eq!(graph.snapshot(), before);
    assert_eq!(next_identity(&graph), next);
}

#[test]
fn field_typing_diagnostics_identify_record_role_and_endpoints() {
    let f = fixture::load("h RuleHead\nc Constructor[C]\nr Record[Rank] s0=h s1=c").unwrap();
    let error = ValidatedGraph::new(&f.graph, &GraphLimits::default()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidConfiguration);
    assert_eq!(error.context().role, Some(Role::Rank));
    assert_eq!(error.context().slot, Some(1));
    assert!(error.context().atoms.contains(&f.names["r"]));
    assert!(error.context().atoms.contains(&f.names["c"]));
}

#[test]
fn package_depth_is_a_resource_bound_not_a_schema_rejection() {
    let f = fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let limits = GraphLimits {
        max_package_depth: 0,
        ..GraphLimits::default()
    };
    let error = validate_with_limits(&f.graph, &limits).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ResourceExhaustion);
    assert_eq!(error.context().limit, Some(LimitKind::PackageDepth));
    assert!(error.context().atoms.contains(&f.names["wh"]));
    validate_with_limits(
        &f.graph,
        &GraphLimits {
            max_package_depth: 1,
            ..limits
        },
    )
    .unwrap();
    let flat = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    validate_with_limits(&flat.graph, &limits).unwrap();
}

#[test]
fn nested_packages_share_depth_and_work_limits_across_recursive_calls() {
    let a4 = include_str!("../../../examples/fixtures/a4.atoms");
    let inner = fixture::load(a4).unwrap();
    let mut text = format!("{a4}\nouter RuleHead\nouter_z Nat\nouter_one Nat s0=outer_z\nouter_two Nat s0=outer_one\nouter_rank Record[Rank] s0=outer s1=outer_two\nouter_mode Record[ModeMeta] s0=outer\nouter_rev Record[Revision] s0=outer\nouter_active Record[Active] s0=outer\n");
    for name in inner.names.keys() {
        text.push_str(&format!("outer_R_{name} Record[Rhs] s0=outer s1={name}\n"));
    }
    let nested = fixture::load(&text).unwrap();
    let limits = GraphLimits {
        max_package_depth: 1,
        ..GraphLimits::default()
    };
    assert_eq!(
        validate_with_limits(&nested.graph, &limits)
            .unwrap_err()
            .context()
            .limit,
        Some(LimitKind::PackageDepth)
    );
    let limits = GraphLimits {
        max_package_depth: 2,
        ..limits
    };
    let config = validate_with_limits(&nested.graph, &limits).unwrap();
    assert_eq!(config.rules.len(), 1);
    assert_eq!(
        config.rules[&nested.names["outer"]].package_atoms.len(),
        inner.graph.len()
    );

    // Locate the exact total work budget needed for this finite fixture. A run
    // one unit short must stop, even though each smaller package succeeds alone.
    let mut low = 0;
    let mut high = limits.max_validation_work;
    while low < high {
        let middle = low + (high - low) / 2;
        let bounded = GraphLimits {
            max_validation_work: middle,
            ..limits
        };
        match validate_with_limits(&nested.graph, &bounded) {
            Ok(_) => high = middle,
            Err(error) => {
                assert_eq!(error.context().limit, Some(LimitKind::ValidationWork));
                low = middle + 1;
            }
        }
    }
    assert!(low > 0);
    let short = GraphLimits {
        max_validation_work: low - 1,
        ..limits
    };
    assert_eq!(
        validate_with_limits(&nested.graph, &short)
            .unwrap_err()
            .context()
            .limit,
        Some(LimitKind::ValidationWork)
    );
    validate_with_limits(&inner.graph, &short).unwrap();
}

#[test]
fn validation_work_is_bounded_and_validation_never_mutates_the_buffer() {
    let f = fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let before = f.graph.snapshot();
    let next = next_identity(&f.graph);
    let limits = GraphLimits {
        max_validation_work: 1,
        ..GraphLimits::default()
    };
    let error = validate_with_limits(&f.graph, &limits).unwrap_err();
    assert_eq!(error.context().limit, Some(LimitKind::ValidationWork));
    assert_eq!(f.graph.snapshot(), before);
    assert_eq!(next_identity(&f.graph), next);
    validate_with_limits(&f.graph, &GraphLimits::default()).unwrap();
}

#[test]
fn detached_atom_and_rule_copies_cannot_modify_a_validated_graph() {
    let f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let view = ValidatedGraph::new(&f.graph, &GraphLimits::default()).unwrap();
    let head = f.names["sl"];
    let mut detached = view.rule(head).unwrap().clone();
    detached.rank = 999;
    detached.lhs.clear();
    let mut atom = view.graph().atom(head).unwrap().clone();
    atom.kind = Kind::Nat;
    assert_eq!(view.rule(head).unwrap().rank, 1);
    assert!(!view.rule(head).unwrap().lhs.is_empty());
    assert_eq!(view.graph().atom(head).unwrap().kind, Kind::RuleHead);
}
