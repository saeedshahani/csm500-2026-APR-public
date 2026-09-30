//! Serial correctness/recovery-cost diagnostic, not a throughput benchmark.
#[allow(dead_code)]
#[path = "support/stabilisation.rs"]
mod model;
use engine::stability::{analyse, reconstruct_statuses, verify_reconstruction, Limits};
use std::time::Instant;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3, "MODE N CODE");
    let n: u32 = args[1].parse().unwrap();
    let mut code: usize = args[2].parse().unwrap();
    assert!((1..=3).contains(&n));
    assert!(code < 6usize.pow(n));
    let states: Vec<_> = (0..n)
        .map(|_| {
            let s = code % 6;
            code /= 6;
            s
        })
        .collect();
    let limits = Limits {
        max_states: 1024,
        max_edges: 10000,
        max_retained_units: 10_000_000,
        max_comparison_work: 100_000_000,
        ..Default::default()
    };
    let (mut graph, contract) = model::family(&states);
    let result = match args[0].as_str() {
        "analyse" => {
            let start = Instant::now();
            match analyse(graph, &contract, limits) {
                Ok(r) => {
                    serde_json::json!({"verdict":r.verdict,"agreement":r.agreement,"states":r.states,"edges":r.edges.len(),"bound":r.recovery_bound,"potential":model::potential(&states),"outputs":r.legitimate_outputs,"elapsed_ns":start.elapsed().as_nanos()})
                }
                Err(e) if e.kind() == graph_core::ErrorKind::ResourceExhaustion => {
                    serde_json::json!({"verdict":"inconclusive","reason":e.to_string()})
                }
                Err(e) => panic!("{e}"),
            }
        }
        "missing" | "duplicate" => {
            // One-component reconstruction needs a single shared retry label.
            assert_eq!(n, 1);
            let labels = if args[0] == "missing" {
                vec![]
            } else {
                vec!["0:done", "0:pending"]
            };
            model::set_status(&mut graph, "0:Ticket", &labels);
            let start = Instant::now();
            let (fixed, receipt) =
                reconstruct_statuses(&graph, &contract, "0:pending", Default::default()).unwrap();
            let reconstruction_ns = start.elapsed().as_nanos();
            let start = Instant::now();
            verify_reconstruction(
                &graph,
                &fixed,
                &contract,
                &receipt,
                "0:pending",
                Default::default(),
            )
            .unwrap();
            let verification_ns = start.elapsed().as_nanos();
            let r = analyse(fixed, &contract, limits).unwrap();
            serde_json::json!({"verdict":r.verdict,"agreement":r.agreement,"bound":r.recovery_bound,"reconstruction_ns":reconstruction_ns,"verification_ns":verification_ns,"receipt":receipt})
        }
        _ => panic!("unknown mode"),
    };
    println!("{result}");
}
