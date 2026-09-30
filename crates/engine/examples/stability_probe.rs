//! Finite-state research diagnostic; elapsed time is not a comparative benchmark.
#[path = "support/body_revisions.rs"]
mod bodies;
use engine::stability::{analyse, Limits};
use graph_core::{ErrorKind, Graph};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3, "MODE N MAX_STATES");
    let n: usize = args[1].parse().unwrap();
    let (g, c) = match args[0].as_str() {
        "guard" => bodies::guard(n),
        "output" => bodies::output(),
        "gated" => bodies::modulating(true),
        "ungated" => bodies::modulating(false),
        "frame" => {
            let (mut g, c) = bodies::guard(n);
            g.insert(graph_core::Kind::Constructor("unused frame".into()), &[])
                .unwrap();
            (g, c)
        }
        "compose" => {
            let (a, ca) = bodies::output();
            let (b, cb) = bodies::guard(n);
            let mut g = Graph::new();
            bodies::append(&mut g, &a, "A:");
            bodies::append(&mut g, &b, "B:");
            let mut c = bodies::prefixed(&ca, "A:");
            let cb = bodies::prefixed(&cb, "B:");
            c.requirements.extend(cb.requirements);
            c.preserve.extend(cb.preserve);
            (g, c)
        }
        _ => panic!("unknown mode"),
    };
    let limits = Limits {
        max_states: args[2].parse().unwrap(),
        max_edges: 10000,
        max_retained_units: 5_000_000,
        max_comparison_work: 50_000_000,
        ..Default::default()
    };
    let result = match analyse(g, &c, limits) {
        Ok(report) => serde_json::to_value(report).unwrap(),
        Err(e) if e.kind() == ErrorKind::ResourceExhaustion => {
            serde_json::json!({"schema":"recovery-analysis-v2","verdict":"inconclusive","reason":e.to_string()})
        }
        Err(e) => panic!("{e}"),
    };
    println!("{}", serde_json::to_string(&result).unwrap());
}
