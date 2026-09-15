//! Cross-crate evidence: an independent substitution oracle, the reusable adapter,
//! and the unmodified general matcher/transaction. Finite tests are not a proof.
use engine::scheduling::{Scheduler, SchedulingStrategy};
use parser::lambda::{self, Limits, Node, Term, Transition};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Expr {
    V(usize),
    L(Box<Expr>),
    A(Box<Expr>, Box<Expr>),
}
fn v(n: usize) -> Expr {
    Expr::V(n)
}
fn l(b: Expr) -> Expr {
    Expr::L(Box::new(b))
}
fn a(f: Expr, x: Expr) -> Expr {
    Expr::A(Box::new(f), Box::new(x))
}
fn identity() -> Expr {
    l(v(0))
}
fn source(e: &Expr) -> Term {
    fn build(e: &Expr, nodes: &mut Vec<Node>) -> usize {
        let n = match e {
            Expr::V(i) => Node::Var(*i),
            Expr::L(b) => Node::Lam(build(b, nodes)),
            Expr::A(f, x) => Node::App(build(f, nodes), build(x, nodes)),
        };
        let id = nodes.len();
        nodes.push(n);
        id
    }
    let mut nodes = Vec::new();
    let root = build(e, &mut nodes);
    Term::new(nodes, root, Limits::default()).unwrap()
}
fn expression(t: &Term, id: usize) -> Expr {
    match t.nodes()[id] {
        Node::Var(i) => v(i),
        Node::Lam(b) => l(expression(t, b)),
        Node::App(f, x) => a(expression(t, f), expression(t, x)),
    }
}
// Independent, conventional de Bruijn shifting/substitution on trees. No graph
// pointer, environment, matcher or adapter readback operation is used here.
fn shift(e: &Expr, delta: isize, cut: usize) -> Expr {
    match e {
        Expr::V(i) => v(if *i >= cut {
            i.checked_add_signed(delta).unwrap()
        } else {
            *i
        }),
        Expr::L(b) => l(shift(b, delta, cut + 1)),
        Expr::A(f, x) => a(shift(f, delta, cut), shift(x, delta, cut)),
    }
}
fn subst(e: &Expr, x: &Expr, depth: usize) -> Expr {
    match e {
        Expr::V(i) if *i == depth => shift(x, depth as isize, 0),
        Expr::V(i) => v(*i),
        Expr::L(b) => l(subst(b, x, depth + 1)),
        Expr::A(f, y) => a(subst(f, x, depth), subst(y, x, depth)),
    }
}
fn reduce(e: &Expr) -> Option<Expr> {
    match e {
        Expr::A(f, x) => match &**f {
            Expr::L(b) => Some(shift(&subst(b, &shift(x, 1, 0), 0), -1, 0)),
            _ => reduce(f).map(|f| a(f, (**x).clone())),
        },
        _ => None,
    }
}
fn run(term: Term, bound: usize) -> (Expr, usize, Vec<Transition>, bool) {
    let mut expected = expression(&term, term.root());
    let encoded = lambda::encode(&term, Limits::default()).unwrap();
    assert_eq!(encoded.rules.len(), 11);
    let original_rules: std::collections::BTreeMap<_, _> = graph_core::validate(&encoded.graph)
        .unwrap()
        .rules
        .values()
        .flat_map(|r| r.representation.iter())
        .map(|id| (*id, encoded.graph.get(*id).unwrap().clone()))
        .collect();
    let mut limits = engine::ExecutionLimits::default();
    limits.search.max_work = 50_000_000;
    let mut execution = engine::Execution::new(encoded.graph, limits).unwrap();
    let mut transitions = Vec::new();
    let mut betas = 0;
    let mut deterministic = Scheduler::Deterministic;
    let mut seeded = Scheduler::seeded(42);
    for _ in 0..bound {
        let decoded = lambda::readback(execution.graph(), encoded.root, Limits::default()).unwrap();
        assert_eq!(expression(&decoded, decoded.root()), expected);
        let enabled = execution.enabled().unwrap();
        if enabled.steps().is_empty() {
            assert_eq!(deterministic.select(enabled.steps()), None);
            assert_eq!(seeded.select(enabled.steps()), None);
            assert!(reduce(&expected).is_none());
            return (expected, betas, transitions, true);
        }
        assert_eq!(
            enabled.steps().len(),
            1,
            "alias specialisations must partition the reachable matches"
        );
        assert_eq!(deterministic.select(enabled.steps()), Some(0));
        assert_eq!(seeded.select(enabled.steps()), Some(0));
        let step = enabled.into_steps().pop().unwrap();
        let transition = encoded.rules[&step.event().source];
        if transition == Transition::Beta {
            expected = reduce(&expected).expect("beta must reflect a source step");
            betas += 1;
        }
        let event = execution.commit(step).unwrap();
        let counts = match transition {
            Transition::Push => (2, 6),
            Transition::Beta => (3, 7),
            _ => (2, 2),
        };
        assert_eq!((event.consumed.len(), event.produced.len()), counts);
        assert!(event.entered.is_empty() && event.exited.is_empty() && event.altered.is_empty());
        for (id, atom) in &original_rules {
            assert_eq!(execution.graph().get(*id), Some(atom));
        }
        transitions.push(transition);
    }
    let decoded = lambda::readback(execution.graph(), encoded.root, Limits::default()).unwrap();
    assert_eq!(expression(&decoded, decoded.root()), expected);
    (expected, betas, transitions, false)
}

#[test]
fn reference_substitution_has_hand_checked_binding_and_strategy_cases() {
    assert_eq!(reduce(&a(identity(), identity())), Some(identity()));
    assert_eq!(reduce(&a(l(l(v(1))), identity())), Some(l(identity())));
    assert_eq!(reduce(&a(l(l(v(0))), identity())), Some(identity()));
    let omega_body = l(a(v(0), v(0)));
    let omega = a(omega_body.clone(), omega_body);
    assert_eq!(reduce(&omega), Some(omega.clone()));
    assert_eq!(reduce(&a(l(identity()), omega)), Some(identity()));
    assert_eq!(reduce(&l(a(identity(), v(0)))), None);
}

#[test]
fn named_syntax_resolves_alpha_renaming_shadowing_and_rejects_bad_inputs() {
    for (left, right) in [
        ("(\\x. x)", "(\\y. y)"),
        ("(\\x. (\\x. x))", "(\\a. (\\b. b))"),
        ("(\\x. (\\y. x))", "(\\a. (\\b. a))"),
    ] {
        let a = lambda::parse(left, Limits::default()).unwrap();
        let b = lambda::parse(right, Limits::default()).unwrap();
        assert_eq!(expression(&a, a.root()), expression(&b, b.root()));
        assert_eq!(run(a, 40), run(b, 40));
    }
    for text in [
        "x",
        "(\\x. y)",
        "(\\x. x) garbage",
        "(\\x. x",
        "(\\. x)",
        "(a b c)",
        "(\\λ. λ)",
    ] {
        assert!(lambda::parse(text, Limits::default()).is_err(), "{text}");
    }
    assert_eq!(
        lambda::parse(
            "(\\x. x)",
            Limits {
                max_work: 0,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        lambda::Error::ResourceLimit
    );
}

#[test]
fn seeded_generated_closed_terms_have_shrinkable_source_reproductions() {
    generated_evidence(false);
}
fn generated_evidence(regenerate: bool) {
    use proptest::{
        prelude::*,
        test_runner::{Config, RngAlgorithm, TestRng, TestRunner},
    };
    // Trees are built from closed combinators, so shrinking never introduces free names.
    // Failure output is the fully parenthesised source, usable directly by lambda-compile.
    let leaf = prop_oneof![
        Just("(\\x. x)".to_string()),
        Just("(\\x. (\\y. x))".to_string()),
        Just("(\\x. (x x))".to_string())
    ];
    let strategy = leaf.prop_recursive(3, 12, 2, |inner| {
        (inner.clone(), inner).prop_map(|(f, a)| format!("({f} {a})"))
    });
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: 32,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &[53; 32]),
    );
    let evidence = std::cell::RefCell::new(Vec::new());
    runner
        .run(&strategy, |text| {
            let term = lambda::parse(&text, Limits::default()).unwrap();
            let renamed = text.replace('x', "a").replace('y', "b");
            let alpha = lambda::parse(&renamed, Limits::default()).unwrap();
            assert_eq!(term.nodes(), alpha.nodes());
            let (_,betas,steps,done) = run(term, 24);
            evidence.borrow_mut().push(serde_json::json!({"source":text,"alpha_renamed":renamed,"beta_steps":betas,"administrative_steps":steps.len()-betas,"graph_steps":steps.len(),"terminal":done}));
            Ok(())
        })
        .unwrap();
    let value = serde_json::json!({"algorithm":"proptest 1.6.0 ChaCha","seed_bytes":vec![53;32],"cases":evidence.into_inner()});
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/lambda/generated.json");
    if regenerate {
        std::fs::write(file, serde_json::to_string_pretty(&value).unwrap() + "\n").unwrap();
    } else {
        assert_eq!(
            value,
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(file).unwrap()).unwrap()
        );
    }
}

fn example_evidence(regenerate: bool) {
    use engine::{
        observation::Observer,
        runner::{RunLimits, Runner},
        trace::{TraceDocument, VerifiedTrace},
    };
    use serde_json::json;
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lambda");
    let mut golden = serde_json::Map::new();
    for (name, beta_count) in [
        ("identity", 1),
        ("erasure", 2),
        ("repetition", 2),
        ("shadowing", 1),
        ("context", 2),
        ("omega", 0),
    ] {
        let text = std::fs::read_to_string(folder.join(format!("{name}.lambda"))).unwrap();
        let term = lambda::parse(&text, Limits::default()).unwrap();
        let (_, betas, steps, done) = run(term.clone(), 20);
        if name != "omega" {
            assert!(done);
            assert_eq!(betas, beta_count);
        } else {
            assert!(!done);
        }
        let encoded = lambda::encode(&term, Limits::default()).unwrap();
        let graph_text = format!(
            "// Lambda source: {}\n{}",
            text.trim().replace(['\n', '\r'], " "),
            parser::print(&encoded.graph).unwrap()
        );
        // Parse the published source before execution: printer/lowerer identity renaming is exercised.
        let graph = parser::load(&graph_text).unwrap().into_graph();
        let mut runner = Runner::new(
            graph,
            Default::default(),
            Scheduler::Deterministic,
            Observer::all(),
        )
        .unwrap();
        runner.enable_recording(Default::default()).unwrap();
        let report = runner
            .run(
                RunLimits {
                    max_steps: 20,
                    ..Default::default()
                },
                None,
                || false,
            )
            .unwrap();
        assert_eq!(
            format!("{:?}", report.outcome),
            if name == "omega" {
                "StepLimit"
            } else {
                "NormalForm"
            }
        );
        let doc = TraceDocument::from_runner(
            &runner,
            Some(engine::trace::sha256(graph_text.as_bytes())),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let verified = VerifiedTrace::verify(doc.clone(), Default::default()).unwrap();
        let observations: Vec<_> = (0..=doc.events.len())
            .map(|i| {
                lambda::interpret(&verified.state_at(i).unwrap(), Limits::default())
                    .unwrap()
                    .unwrap()
            })
            .collect();
        let mut reference = expression(&term, term.root());
        assert_eq!(
            observations[0],
            source(&reference).display(Limits::default()).unwrap()
        );
        for (i, step) in steps.iter().enumerate() {
            if *step == Transition::Beta {
                reference = reduce(&reference).unwrap();
            }
            assert_eq!(
                observations[i + 1],
                source(&reference).display(Limits::default()).unwrap()
            );
        }
        golden.insert(name.into(), json!({"source":text.trim(),"outcome":format!("{:?}",report.outcome),"beta_steps":betas,"administrative_steps":steps.len()-betas,"transitions":steps.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>(),"readbacks":observations}));
        // Durable fixtures certify checked bindings, not a scheduler implementation
        // hash which changes with the lockfile. The fresh run above checks outcomes.
        let portable = TraceDocument::from_recording(
            runner.recording().unwrap(),
            runner.observer(),
            Some(engine::trace::sha256(graph_text.as_bytes())),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let trace = portable.to_json(16 * 1024 * 1024).unwrap();
        if regenerate {
            std::fs::write(folder.join(format!("{name}.graph")), graph_text).unwrap();
            std::fs::write(folder.join(format!("{name}.trace.json")), trace).unwrap();
        } else {
            assert_eq!(
                std::fs::read_to_string(folder.join(format!("{name}.graph"))).unwrap(),
                graph_text
            );
            let stored = VerifiedTrace::import(
                &std::fs::read(folder.join(format!("{name}.trace.json"))).unwrap(),
                Default::default(),
            )
            .unwrap();
            // Implementation hashes may change; event content, observations and outcomes may not.
            assert_eq!(
                serde_json::to_value(&stored.document().events).unwrap(),
                serde_json::to_value(&doc.events).unwrap()
            );
            assert_eq!(
                lambda::interpret(
                    &stored.state_at(doc.events.len()).unwrap(),
                    Limits::default()
                )
                .unwrap(),
                lambda::interpret(
                    &verified.state_at(doc.events.len()).unwrap(),
                    Limits::default()
                )
                .unwrap()
            );
        }
    }
    let value = serde_json::Value::Object(golden);
    if regenerate {
        std::fs::write(
            folder.join("golden.json"),
            serde_json::to_string_pretty(&value).unwrap() + "\n",
        )
        .unwrap();
    } else {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(folder.join("golden.json")).unwrap()
            )
            .unwrap(),
            value
        );
    }
}
#[test]
fn published_lambda_examples_match_manual_oracle_and_verified_histories() {
    example_evidence(false);
}
#[test]
#[ignore = "explicit deterministic fixture regeneration; overwrites only examples/lambda generated files"]
fn regenerate_lambda_fixtures() {
    example_evidence(true);
    generated_evidence(true);
}

#[test]
fn graph_machine_simulates_nested_duplication_erasure_shadowing_and_lookup() {
    let omega_body = l(a(v(0), v(0)));
    let omega = a(omega_body.clone(), omega_body);
    let cases = [
        (identity(), identity(), 0),
        (a(identity(), identity()), identity(), 1),
        (a(a(l(l(v(1))), identity()), l(l(v(0)))), identity(), 2),
        (a(l(a(v(0), v(0))), identity()), identity(), 2),
        (a(l(identity()), omega), identity(), 1),
        (a(l(l(v(0))), l(l(v(1)))), identity(), 1),
        (a(l(l(v(1))), identity()), l(identity()), 1),
        (a(a(identity(), identity()), identity()), identity(), 2),
    ];
    let mut all = Vec::new();
    for (input, output, count) in cases {
        let (result, betas, steps, done) = run(source(&input), 60);
        assert!(done);
        assert_eq!(result, output);
        assert_eq!(betas, count);
        all.extend(steps);
    }
    for t in [
        Transition::Push,
        Transition::Beta,
        Transition::LookupZero,
        Transition::LookupSucc,
    ] {
        assert!(all.contains(&t));
    }
}

#[test]
fn shared_source_nodes_and_divergent_prefix_do_not_require_noninjective_matches() {
    let t = Term::new(
        vec![Node::Var(0), Node::App(0, 0), Node::Lam(1), Node::App(2, 2)],
        3,
        Limits::default(),
    )
    .unwrap();
    let initial = expression(&t, t.root());
    let (result, betas, _, done) = run(t, 20);
    assert!(!done);
    assert!(betas >= 3);
    assert_eq!(result, initial);
}

#[test]
fn source_validation_and_readback_limits_are_explicit() {
    assert!(matches!(
        Term::new(vec![Node::Var(0)], 0, Limits::default()),
        Err(lambda::Error::InvalidSource("free variable"))
    ));
    assert!(Term::new(vec![Node::Lam(0)], 0, Limits::default()).is_err());
    assert!(Term::new(
        vec![Node::Var(0), Node::Lam(0), Node::Var(9)],
        1,
        Limits::default()
    )
    .is_err());
    let encoded = lambda::encode(&source(&identity()), Limits::default()).unwrap();
    assert_eq!(
        lambda::readback(
            &encoded.graph,
            encoded.root,
            Limits {
                max_work: 0,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        lambda::Error::ResourceLimit
    );
    assert!(lambda::encode(
        &source(&identity()),
        Limits {
            max_nodes: 1,
            ..Limits::default()
        }
    )
    .is_err());
    let mut graph = encoded.graph.clone();
    graph
        .insert(
            graph_core::Kind::Record {
                role: graph_core::Role::ObjectLink,
                label: Some("lc.term".into()),
            },
            &[encoded.root, encoded.root],
        )
        .unwrap();
    assert_eq!(
        lambda::readback(&graph, encoded.root, Limits::default()).unwrap_err(),
        lambda::Error::InvalidGraph("ambiguous field")
    );
}

#[test]
fn all_eleven_alias_specialisations_have_an_admitted_graph_witness() {
    use graph_core::{AtomKey, Graph, Kind, Role};
    fn c(g: &mut Graph, s: &str) -> AtomKey {
        g.insert(Kind::Constructor(format!("lc.{s}")), &[]).unwrap()
    }
    fn e(g: &mut Graph, s: &str, a: AtomKey, b: AtomKey) {
        g.insert(
            Kind::Record {
                role: Role::ObjectLink,
                label: Some(format!("lc.{s}")),
            },
            &[a, b],
        )
        .unwrap();
    }
    fn env(g: &mut Graph, t: AtomKey, f: AtomKey, tail: AtomKey) -> AtomKey {
        let x = c(g, "Env");
        e(g, "term", x, t);
        e(g, "env", x, f);
        e(g, "tail", x, tail);
        x
    }
    for variant in 0..11 {
        let mut encoded = lambda::encode(&source(&identity()), Limits::default()).unwrap();
        let g = &mut encoded.graph;
        let old = g
            .iter()
            .filter(|(_, a)| a.kind.role() == Some(Role::ObjectLink) && a.fields[0] == encoded.root)
            .map(|(id, _)| id)
            .collect();
        g.remove_set(&old).unwrap();
        let nil = c(g, "NilEnv");
        let empty = c(g, "NilStack");
        let z = c(g, "Z");
        let inner = c(g, "Lam");
        e(g, "body", inner, z);
        let lam = c(g, "Lam");
        e(g, "body", lam, inner);
        let (term, environment, stack) = if variant < 2 {
            let app = c(g, "App");
            e(g, "fun", app, inner);
            e(g, "arg", app, if variant == 1 { inner } else { lam });
            (app, nil, empty)
        } else if variant < 8 {
            let alias = (variant - 2) / 2;
            let same = (variant - 2) % 2 == 1;
            let arg = match alias {
                1 => lam,
                2 => inner,
                _ => {
                    let x = c(g, "Lam");
                    e(g, "body", x, z);
                    x
                }
            };
            let captured = if same { nil } else { env(g, inner, nil, nil) };
            let frame = c(g, "Stack");
            e(g, "term", frame, arg);
            e(g, "env", frame, captured);
            e(g, "tail", frame, empty);
            (lam, nil, frame)
        } else if variant < 10 {
            let captured = if variant == 9 {
                env(g, inner, nil, nil)
            } else {
                nil
            };
            let current = env(g, if variant == 9 { z } else { inner }, captured, nil);
            (z, current, empty)
        } else {
            let succ = c(g, "S");
            e(g, "pred", succ, z);
            let tail = env(g, inner, nil, nil);
            let current = env(g, inner, nil, tail);
            (succ, current, empty)
        };
        e(g, "term", encoded.root, term);
        e(g, "env", encoded.root, environment);
        e(g, "stack", encoded.root, stack);
        let before = lambda::readback(g, encoded.root, Limits::default()).unwrap();
        let expected = expression(&before, before.root());
        let selected = *encoded.rules.keys().nth(variant).unwrap();
        let mut limits = engine::ExecutionLimits::default();
        limits.search.max_work = 50_000_000;
        let mut execution = engine::Execution::new(encoded.graph, limits).unwrap();
        let enabled = execution.enabled().unwrap();
        assert_eq!(enabled.steps().len(), 1, "variant {variant}");
        let step = enabled.into_steps().pop().unwrap();
        assert_eq!(step.event().source, selected, "variant {variant}");
        execution.commit(step).unwrap();
        let after = lambda::readback(execution.graph(), encoded.root, Limits::default()).unwrap();
        let result = expression(&after, after.root());
        assert_eq!(
            result,
            if (2..8).contains(&variant) {
                reduce(&expected).unwrap()
            } else {
                expected
            }
        );
    }
}

#[test]
fn bounded_generated_closed_terms_agree_at_every_observed_step() {
    // Exhaust the small grammar through size seven, not rejection-sampled raw graphs.
    fn terms(size: usize, depth: usize) -> Vec<Expr> {
        if size == 1 {
            return (0..depth).map(v).collect();
        }
        let mut out: Vec<_> = terms(size - 1, depth + 1).into_iter().map(l).collect();
        for left in 1..size - 1 {
            for f in terms(left, depth) {
                for x in terms(size - 1 - left, depth) {
                    out.push(a(f.clone(), x));
                }
            }
        }
        out
    }
    let mut count = 0;
    for size in 2..=7 {
        for term in terms(size, 0) {
            let (_, _, _, done) = run(source(&term), 40);
            assert!(done);
            count += 1;
        }
    }
    assert_eq!(count, 201);
}
