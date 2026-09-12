// Cross-crate boundary: the engine production implementation does not import
// this separately maintained parser test oracle or depend on parser.
#[path = "../../parser/tests/support/mod.rs"]
mod oracle;
use engine::observation::{all, isomorphic, ObservationLimits, Observer};
use graph_core::{fixture, Graph, GraphLimits, Kind, Role, ValidatedGraph};

fn compare(a: &Graph, b: &Graph) -> bool {
    let observe = |g| all(g, GraphLimits::default(), ObservationLimits::default()).unwrap();
    let actual = isomorphic(&observe(a), &observe(b), 10_000_000).unwrap();
    assert_eq!(actual, oracle::isomorphic(a, b, 10_000_000).unwrap());
    actual
}
#[test]
fn generated_equivalence_laws_and_independent_oracle() {
    for n in 0..12 {
        let mut g = Graph::new();
        let mut nodes = Vec::new();
        for i in 0..n {
            nodes.push(
                g.insert(Kind::Constructor(format!("c{}", i % 3)), &[])
                    .unwrap(),
            );
        }
        for i in 0..n {
            g.insert(
                Kind::Record {
                    role: Role::ObjectLink,
                    label: Some("value".into()),
                },
                &[nodes[i], nodes[(i + 1) % n]],
            )
            .unwrap();
        }
        let h = oracle::reversed(&g);
        let k = oracle::reversed(&h);
        assert!(compare(&g, &g));
        assert!(compare(&g, &h));
        assert!(compare(&h, &g));
        assert!(compare(&h, &k));
        assert!(compare(&g, &k));
        let mut changed = h.clone();
        changed
            .insert(Kind::Constructor("litter".into()), &[])
            .unwrap();
        assert!(!compare(&g, &changed));
    }
}
#[test]
fn complete_quotation_metadata_and_fresh_revision_identity() {
    for source in [
        include_str!("../../../examples/fixtures/b2.atoms"),
        include_str!("../../../examples/fixtures/a4.atoms"),
    ] {
        let f = fixture::load(source).unwrap();
        assert!(compare(&f.graph, &oracle::reversed(&f.graph)));
    }
    let mut g = fixture::load(include_str!("../../../examples/fixtures/b2.atoms"))
        .unwrap()
        .graph;
    let before = g.clone();
    let (revision, head) = g
        .iter()
        .find(|(_, a)| a.kind.role() == Some(Role::Revision))
        .map(|(k, a)| (k, a.fields[0]))
        .unwrap();
    g.remove_set(&[revision].into()).unwrap();
    let fresh = g.insert(Kind::record(Role::Revision), &[head]).unwrap();
    assert_ne!(revision, fresh);
    assert!(compare(&before, &g));
    let active = g
        .iter()
        .find(|(_, a)| a.kind.role() == Some(Role::Active))
        .unwrap()
        .0;
    g.remove_set(&[active].into()).unwrap();
    assert!(!compare(&before, &g));
    let o = Observer::parse(
        "observer-v1\nselect data\nkind Constructor\n",
        |_| None,
        100,
    )
    .unwrap();
    let p = |g| {
        o.project(
            &ValidatedGraph::new(g, &GraphLimits::default()).unwrap(),
            ObservationLimits::default(),
        )
        .unwrap()
    };
    assert!(isomorphic(&p(&before), &p(&g), 10000).unwrap());
}
#[test]
fn labels_positions_parallel_edges_and_self_links_are_preserved() {
    let graph = |reverse: bool, parallel: usize, label: &str| {
        let mut g = Graph::new();
        let a = g.insert(Kind::Constructor("a".into()), &[]).unwrap();
        let b = g.insert(Kind::Constructor("b".into()), &[]).unwrap();
        let fields = if reverse { [b, a] } else { [a, b] };
        for _ in 0..parallel {
            g.insert(
                Kind::Record {
                    role: Role::ObjectLink,
                    label: Some(label.into()),
                },
                &fields,
            )
            .unwrap();
        }
        g
    };
    assert!(!compare(&graph(false, 1, "x"), &graph(true, 1, "x")));
    assert!(!compare(&graph(false, 1, "x"), &graph(false, 2, "x")));
    assert!(!compare(&graph(false, 1, "x"), &graph(false, 1, "y")));
}
#[test]
fn comparison_zero_and_exact_empty_budget() {
    let empty = all(
        &Graph::new(),
        GraphLimits::default(),
        ObservationLimits::default(),
    )
    .unwrap();
    assert!(isomorphic(&empty, &empty, 0).is_err());
    assert!(isomorphic(&empty, &empty, 2).is_err());
    assert!(isomorphic(&empty, &empty, 3).unwrap());
}

#[test]
fn identity_ordered_scheduler_is_not_invariant_under_graph_renaming() {
    use engine::runner::{RunLimits, Runner};
    use engine::scheduling::Scheduler;
    let source="program { rule a { atom az=Nat(); rank ar=az; mode am=ModeObject; revision av; active aa; atom x=Constructor(\"A\"); rhs { ax:x; } } rule b { atom bz=Nat(); rank br=bz; mode bm=ModeObject; revision bv; active ba; atom y=Constructor(\"B\"); rhs { by:y; } } }";
    let g = parser::load(source).unwrap().into_graph();
    let h = oracle::reversed(&g);
    assert!(compare(&g, &h));
    let execute = |g| {
        let mut r = Runner::new(
            g,
            engine::ExecutionLimits::default(),
            Scheduler::Deterministic,
            Observer::all(),
        )
        .unwrap();
        r.run(
            RunLimits {
                max_steps: 1,
                ..RunLimits::default()
            },
            None,
            || false,
        )
        .unwrap();
        r.execution().graph().clone()
    };
    assert!(!compare(&execute(g), &execute(h)));
}

#[test]
fn mode_rank_and_side_attributes_change_complete_equivalence() {
    let source = include_str!("../../../examples/fixtures/b2.atoms");
    let before = fixture::load(source).unwrap().graph;
    let changed = source.replace(
        "sl_r1         Nat                  s0=sl_r0",
        "sl_r1         Nat",
    );
    let after = fixture::load(&changed).unwrap().graph;
    assert!(!compare(&before, &after));
    let source="program { rule h { atom z=Nat(); rank r=z; mode m=ModeObject; revision v; active a; atom x=Constructor(\"x\"); lhs { l:x; } } }";
    let before = parser::load(source).unwrap().into_graph();
    let after = parser::load(&source.replace("ModeObject", "ModeMeta"))
        .unwrap()
        .into_graph();
    assert!(!compare(&before, &after));
    let changed_side = parser::load(&source.replace("lhs", "rhs"))
        .unwrap()
        .into_graph();
    assert!(!compare(&before, &changed_side));
}
