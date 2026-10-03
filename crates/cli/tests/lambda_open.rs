//! Independent locally nameless substitution versus graph-resident open evaluation.
use parser::lambda::{self, Limits, Node, OpenTerm, Transition};

#[derive(Clone, Debug, PartialEq, Eq)]
enum E {
    B(usize),
    F(String),
    L(Box<E>),
    A(Box<E>, Box<E>),
}
fn l(e: E) -> E {
    E::L(Box::new(e))
}
fn a(f: E, x: E) -> E {
    E::A(Box::new(f), Box::new(x))
}
fn tree(t: &OpenTerm, id: usize, depth: usize) -> E {
    match t.nodes()[id] {
        Node::Var(n) if n < depth => E::B(n),
        Node::Var(n) => E::F(t.free_names()[n - depth].clone()),
        Node::Lam(b) => l(tree(t, b, depth + 1)),
        Node::App(f, x) => a(tree(t, f, depth), tree(t, x, depth)),
    }
}
fn shift(e: &E, delta: isize, depth: usize) -> E {
    match e {
        E::B(n) if *n >= depth => E::B(n.checked_add_signed(delta).unwrap()),
        E::B(_) | E::F(_) => e.clone(),
        E::L(b) => l(shift(b, delta, depth + 1)),
        E::A(f, x) => a(shift(f, delta, depth), shift(x, delta, depth)),
    }
}
fn subst(e: &E, x: &E, depth: usize) -> E {
    match e {
        E::B(n) if *n == depth => shift(x, depth as isize, 0),
        E::B(_) | E::F(_) => e.clone(),
        E::L(b) => l(subst(b, x, depth + 1)),
        E::A(f, y) => a(subst(f, x, depth), subst(y, x, depth)),
    }
}
fn beta(b: &E, x: &E) -> E {
    shift(&subst(b, &shift(x, 1, 0), 0), -1, 0)
}
fn step(e: &E) -> Option<E> {
    match e {
        E::A(f, x) => match &**f {
            E::L(b) => Some(beta(b, x)),
            _ => step(f).map(|f| a(f, (**x).clone())),
        },
        _ => None,
    }
}
fn normal(e: &E) -> Option<E> {
    match e {
        E::L(b) => normal(b).map(l),
        E::A(f, x) => match &**f {
            E::L(b) => Some(beta(b, x)),
            _ => normal(f)
                .map(|f| a(f, (**x).clone()))
                .or_else(|| normal(x).map(|x| a((**f).clone(), x))),
        },
        _ => None,
    }
}
fn cbv(e: &E) -> Option<E> {
    match e {
        E::A(f, x) => cbv(f)
            .map(|f| a(f, (**x).clone()))
            .or_else(|| cbv(x).map(|x| a((**f).clone(), x)))
            .or_else(|| match (&**f, &**x) {
                (E::L(b), E::L(_)) => Some(beta(b, x)),
                _ => None,
            }),
        _ => None,
    }
}
fn parsed(text: &str) -> OpenTerm {
    lambda::parse_open(text, vec!["x".into(), "y".into()], Limits::default()).unwrap()
}
fn run(term: &OpenTerm, bound: usize) -> serde_json::Value {
    let mut expected = tree(term, term.root(), 0);
    let encoded = lambda::encode_open(term, Limits::default()).unwrap();
    let printed = parser::print(&encoded.graph).unwrap();
    let loaded = parser::load(&printed).unwrap();
    assert_eq!(
        lambda::interpret(loaded.graph(), Limits::default()).unwrap(),
        Some(term.display(Limits::default()).unwrap())
    );
    let mut limits = engine::ExecutionLimits::default();
    limits.search.max_work = 50_000_000;
    let mut execution = engine::Execution::new(encoded.graph, limits).unwrap();
    let mut recording = engine::recording::Recording::new(&execution, Default::default()).unwrap();
    let initial_atoms = execution.graph().len();
    let mut transitions = Vec::new();
    let mut readbacks = Vec::new();
    let mut terminal = false;
    for i in 0..=bound {
        let read =
            lambda::readback_open(execution.graph(), encoded.root, Limits::default()).unwrap();
        assert_eq!(tree(&read, read.root(), 0), expected);
        readbacks.push(read.display(Limits::default()).unwrap());
        let enabled = execution.enabled().unwrap();
        if enabled.steps().is_empty() {
            assert!(step(&expected).is_none());
            terminal = true;
            break;
        }
        assert_eq!(enabled.steps().len(), 1);
        if i == bound {
            break;
        }
        let rewrite = enabled.into_steps().pop().unwrap();
        let transition = encoded.rules[&rewrite.event().source];
        if transition == Transition::Beta {
            expected = step(&expected).unwrap();
        }
        let event = recording.commit(&mut execution, rewrite).unwrap();
        assert!(event.altered.is_empty() && event.entered.is_empty() && event.exited.is_empty());
        transitions.push(format!("{transition:?}"));
    }
    serde_json::json!({"source":term.display(Limits::default()).unwrap(),"context":term.free_names(),
        "transitions":transitions,"readbacks":readbacks,"terminal":terminal,
        "initial_atoms":initial_atoms,"final_atoms":execution.graph().len()})
}

#[test]
fn strategy_counterfactuals_distinguish_open_weak_strong_and_strict() {
    let input = parsed("(\\z. ((\\w. w) z))");
    let e = tree(&input, input.root(), 0);
    assert!(step(&e).is_none());
    assert_eq!(normal(&e), Some(l(E::B(0))));
    let input = parsed("((\\z. (\\w. w)) ((\\u. (u u)) (\\u. (u u))))");
    let e = tree(&input, input.root(), 0);
    assert_eq!(step(&e), Some(l(E::B(0))));
    assert_eq!(cbv(&e), Some(e));
    let input = parsed("(x ((\\z. z) y))");
    let e = tree(&input, input.root(), 0);
    assert!(step(&e).is_none());
    assert_eq!(normal(&e), Some(a(E::F("x".into()), E::F("y".into()))));
    // A cyclic source is representation, not an accepted evaluation strategy.
    assert!(OpenTerm::new(vec![Node::App(0, 0)], 0, vec![], Limits::default()).is_err());
}

#[test]
fn capture_shadowing_neutral_spines_and_divergence() {
    for (input, output) in [
        ("((\\z. (\\x. z)) x)", "(λ.x)"),
        ("((\\z. (\\x. (z x))) x)", "(λ.(x 0))"),
        ("((\\z. (\\u. (\\v. z))) (\\u. x))", "(λ.(λ.(λ.x)))"),
        ("((\\x. x) y)", "y"),
        ("((\\z. z) (x y))", "(x y)"),
        ("((\\f. ((f x) y)) (\\u. (\\v. u)))", "x"),
        (
            "(x ((\\u. (u u)) (\\u. (u u))))",
            "(x ((λ.(0 0)) (λ.(0 0))))",
        ),
        ("((\\z. x) ((\\u. (u u)) (\\u. (u u))))", "x"),
    ] {
        let result = run(&parsed(input), 50);
        assert_eq!(result["terminal"], true, "{input}");
        assert_eq!(
            result["readbacks"].as_array().unwrap().last().unwrap(),
            output,
            "{input}"
        );
    }
    let omega = run(&parsed("((\\u. (u u)) (\\u. (u u)))"), 30);
    assert_eq!(omega["terminal"], false);
    assert!(
        omega["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| **x == "Beta")
            .count()
            > 2
    );
}

fn terms(size: usize, depth: usize) -> Vec<E> {
    if size == 1 {
        return (0..depth)
            .map(E::B)
            .chain([E::F("x".into()), E::F("y".into())])
            .collect();
    }
    let mut out = terms(size - 1, depth + 1)
        .into_iter()
        .map(l)
        .collect::<Vec<_>>();
    for left in 1..size - 1 {
        for f in terms(left, depth) {
            for x in terms(size - 1 - left, depth) {
                out.push(a(f.clone(), x));
            }
        }
    }
    out
}
fn arena(e: &E, nodes: &mut Vec<Node>, depth: usize) -> usize {
    let n = match e {
        E::B(n) => Node::Var(*n),
        E::F(n) => Node::Var(depth + usize::from(n == "y")),
        E::L(b) => Node::Lam(arena(b, nodes, depth + 1)),
        E::A(f, x) => Node::App(arena(f, nodes, depth), arena(x, nodes, depth)),
    };
    let id = nodes.len();
    nodes.push(n);
    id
}
fn evidence() -> serde_json::Value {
    let mut rows = Vec::new();
    for size in 1..=6 {
        for e in terms(size, 0) {
            let mut nodes = Vec::new();
            let root = arena(&e, &mut nodes, 0);
            let term = OpenTerm::new(nodes, root, vec!["x".into(), "y".into()], Limits::default())
                .unwrap();
            rows.push(run(&term, 24));
        }
    }
    serde_json::json!({"profile":lambda::OPEN_PROFILE,"oracle":"locally-nameless-substitution-v1","tree_sizes":[1,6],"graph_step_bound":24,"cases":rows})
}
#[test]
fn exhaustive_small_open_terms_match_independent_substitution() {
    let actual = evidence();
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/lambda/open/evidence.json");
    assert_eq!(
        actual,
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(file).unwrap()).unwrap()
    );
}
#[test]
#[ignore = "explicit evidence generation"]
fn generate_open_evidence() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/lambda/open/evidence.json");
    std::fs::write(
        file,
        serde_json::to_string_pretty(&evidence()).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
fn context_bounds_profiles_aliasing_and_readback_rejections() {
    let limits = Limits::default();
    for names in [vec!["x".into(), "y".into()], vec!["y".into(), "x".into()]] {
        let term = lambda::parse_open("((\\z. (\\u. (z y))) x)", names, limits).unwrap();
        let result = run(&term, 20);
        assert_eq!(
            result["readbacks"].as_array().unwrap().last().unwrap(),
            "(λ.(x y))"
        );
    }
    let empty = lambda::parse_open("((\\x. x) (\\y. y))", vec![], limits).unwrap();
    assert_eq!(run(&empty, 10)["terminal"], true);
    for names in [
        vec!["x".into(), "x".into()],
        vec!["1x".into()],
        vec!["x,y".into()],
    ] {
        assert!(lambda::parse_open("x", names, limits).is_err());
    }
    assert!(lambda::parse_open("z", vec!["x".into()], limits).is_err());
    assert!(lambda::compile_profile("x", "unknown", vec!["x".into()], limits).is_err());
    assert!(
        lambda::compile_profile("x", lambda::CLOSED_PROFILE, vec!["x".into()], limits).is_err()
    );
    assert!(lambda::parse_open(
        "x",
        vec!["x".into()],
        Limits {
            max_nodes: 0,
            ..limits
        }
    )
    .is_err());
    let shared = OpenTerm::new(
        vec![Node::Var(0), Node::App(0, 0)],
        1,
        vec!["x".into()],
        limits,
    )
    .unwrap();
    assert_eq!(run(&shared, 10)["terminal"], true);
    // Same arena index is free at depth zero and locally bound beneath a lambda.
    let shared = OpenTerm::new(
        vec![Node::Var(0), Node::Lam(0), Node::App(1, 0)],
        2,
        vec!["x".into()],
        limits,
    )
    .unwrap();
    let result = run(&shared, 10);
    assert_eq!(result["readbacks"].as_array().unwrap().last().unwrap(), "x");
    let encoded = lambda::encode_open(&parsed("x"), limits).unwrap();
    assert!(lambda::readback(&encoded.graph, encoded.root, limits).is_err());
    assert_eq!(
        lambda::readback_open(
            &encoded.graph,
            encoded.root,
            Limits {
                max_work: 0,
                ..limits
            }
        )
        .unwrap_err(),
        lambda::Error::ResourceLimit
    );
    let closed = lambda::encode(&lambda::parse("(\\x. x)", limits).unwrap(), limits).unwrap();
    assert!(lambda::readback_open(&closed.graph, closed.root, limits).is_err());
    let mut snapshot = engine::trace::GraphData::from_graph(&encoded.graph);
    for atom in snapshot.atoms.values_mut() {
        if let graph_core::Kind::Constructor(s) = &mut atom.kind {
            if s.starts_with("lc.Profile:") {
                *s = "lc.Profile:open-cbn-v999:x,y".into();
            }
        }
    }
    let bad = snapshot.restore(&Default::default()).unwrap();
    assert!(lambda::interpret(&bad, limits).is_err());
}
