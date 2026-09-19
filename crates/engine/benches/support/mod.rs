#![allow(dead_code)]
use engine::{
    observation::Observer,
    runner::{RunLimits, Runner},
    scheduling::Scheduler,
    Execution,
};
use graph_core::{Binding, Graph, Kind, Role, Sort};
#[path = "../../../wasm-bridge/src/dataset.rs"]
pub mod dataset;

pub fn fixture(family: &str, n: usize) -> Graph {
    if family == "practical" || family == "guarded" {
        let d = dataset::generate(
            n,
            42,
            if family == "guarded" {
                dataset::Policy::GuardedEnable
            } else {
                dataset::Policy::Fixed
            },
        )
        .unwrap();
        return parser::load(&dataset::source(&d).unwrap())
            .unwrap()
            .into_graph();
    }
    if family == "lambda" {
        let text = match n {
            1 => include_str!("../../../../examples/lambda/identity.lambda"),
            2 => include_str!("../../../../examples/lambda/repetition.lambda"),
            _ => include_str!("../../../../examples/lambda/omega.lambda"),
        };
        return parser::lambda::encode(
            &parser::lambda::parse(text, Default::default()).unwrap(),
            Default::default(),
        )
        .unwrap()
        .graph;
    }
    if family == "reflective" {
        return parser::load(include_str!(
            "../../../../examples/programs/self-modify.graph"
        ))
        .unwrap()
        .into_graph();
    }
    if family == "trace" {
        return parser::load(include_str!(
            "../../../../examples/behaviour/oscillating.graph"
        ))
        .unwrap()
        .into_graph();
    }
    let mut g = Graph::new();
    if family == "atoms" || family == "fields" {
        let anchor = g.insert(Kind::Constructor("anchor".into()), &[]).unwrap();
        let endpoints = [anchor, anchor];
        for _ in 0..n {
            g.insert(
                if family == "atoms" {
                    Kind::Constructor("data".into())
                } else {
                    Kind::Record {
                        role: Role::ObjectLink,
                        label: Some("link".into()),
                    }
                },
                if family == "atoms" { &[] } else { &endpoints },
            )
            .unwrap();
        }
        return g;
    }
    for _ in 0..if family == "rules" { n } else { 1 } {
        let head = g.insert(Kind::RuleHead, &[]).unwrap();
        let zero = g.insert(Kind::Nat, &[]).unwrap();
        for (role, fields) in [
            (Role::Rank, vec![head, zero]),
            (Role::Revision, vec![head]),
            (Role::ModeObject, vec![head]),
            (Role::Active, vec![head]),
        ] {
            g.insert(Kind::record(role), &fields).unwrap();
        }
        for i in 0..if family == "pattern" { n } else { 1 } {
            let kind = if family == "branch" || family == "reject" {
                Kind::variable(Sort::Constructor, Binding::Free)
            } else {
                Kind::Constructor(format!("p{i}"))
            };
            let template = g.insert(kind, &[]).unwrap();
            g.insert(Kind::record(Role::Lhs), &[head, template])
                .unwrap();
            if family != "reject" {
                g.insert(Kind::record(Role::Rhs), &[head, template])
                    .unwrap();
            }
        }
    }
    for i in 0..if family == "branch" || family == "reject" || family == "pattern" {
        n
    } else {
        1
    } {
        let id = g.insert(Kind::Constructor(format!("p{i}")), &[]).unwrap();
        if family == "reject" {
            g.insert(
                Kind::Record {
                    role: Role::ObjectLink,
                    label: Some("incoming".into()),
                },
                &[id, id],
            )
            .unwrap();
        }
    }
    graph_core::validate(&g).unwrap();
    g
}
pub fn runner(g: Graph) -> Runner<Scheduler> {
    let mut r = Runner::new(
        g,
        Default::default(),
        Scheduler::Deterministic,
        Observer::all(),
    )
    .unwrap();
    r.enable_recording(Default::default()).unwrap();
    r
}
pub fn history(n: usize) -> Runner<Scheduler> {
    let mut r = runner(fixture("trace", n));
    r.run(
        RunLimits {
            max_steps: n as u64,
            ..Default::default()
        },
        None,
        || false,
    )
    .unwrap();
    assert_eq!(r.execution().committed_steps(), n as u64);
    r
}
pub fn check(g: &Graph, family: &str, n: usize) {
    graph_core::validate(g).unwrap();
    let e = Execution::new(g.clone(), Default::default()).unwrap();
    match e.enabled() {
        Ok(set) => match family {
            "atoms" | "fields" => assert!(set.steps().is_empty()),
            "reject" => {
                assert!(set.steps().is_empty());
                assert_eq!(set.rejected().len(), n);
            }
            "branch" | "rules" | "practical" => assert_eq!(set.steps().len(), n),
            _ => assert_eq!(set.steps().len(), 1),
        },
        Err(err) => {
            assert_eq!(err.kind(), graph_core::ErrorKind::ResourceExhaustion);
            assert_eq!(family, "practical");
            assert_eq!(n, 1000);
        }
    }
}
