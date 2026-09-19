//! AV-02/C01 phase probe. Failed complete queries never expose a partial set.
#[path = "../benches/support/mod.rs"]
mod support;
use engine::{observation::Observer, runner::RunLimits, Execution};
use serde_json::json;
use std::time::Instant;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2, "profile_work_items SIZE PHASE");
    let n: usize = args[0].parse().unwrap();
    assert!([10, 100, 1000].contains(&n));
    let phase = &args[1];
    let graph = support::fixture("practical", n);
    let source = parser::print(&graph).unwrap();
    let execution = Execution::new(graph.clone(), Default::default()).unwrap();
    let head = *execution
        .view()
        .unwrap()
        .configuration()
        .rules
        .keys()
        .next()
        .unwrap();
    let mut result = json!({"schema":"work-item-profile-v1", "items":n,"phase":phase,
        "atoms":graph.len(),"source_sha256":engine::trace::sha256(source.as_bytes()),
        "implementation":engine::trace::IMPLEMENTATION,"outcome":"Completed",
        "max_work":1000000,"max_candidates":1000});
    let started = Instant::now();
    match phase.as_str() {
        "load" => {
            std::hint::black_box(parser::load(&source).unwrap());
        }
        "validate" => {
            std::hint::black_box(graph_core::validate(&graph).unwrap());
        }
        "clone" => {
            std::hint::black_box(graph.clone());
        }
        "observe" => {
            std::hint::black_box(
                Observer::all()
                    .project(&execution.view().unwrap(), Default::default())
                    .unwrap(),
            );
        }
        "bindings" | "enabled" => {
            let mut search = execution.search();
            let count = if phase == "bindings" {
                search.bindings(head).map(|v| v.len())
            } else {
                search.enabled().map(|v| v.steps().len())
            };
            match count {
                Ok(count) => {
                    result["complete_count"] = json!(count);
                }
                Err(error) => {
                    result["outcome"] = json!(format!("{:?}", error.kind()));
                    result["diagnostic"] = json!(error.to_string());
                }
            }
            result["work_used"] = json!(search.work_used());
            result["attempted_candidates"] = json!(search.attempted_candidates());
        }
        "run" | "recorded_run" => {
            let mut runner = engine::runner::Runner::new(
                graph.clone(),
                Default::default(),
                engine::scheduling::Scheduler::Deterministic,
                Observer::all(),
            )
            .unwrap();
            if phase == "recorded_run" {
                runner.enable_recording(Default::default()).unwrap();
            }
            let report = runner
                .run(
                    RunLimits {
                        max_steps: n as u64 + 1,
                        ..Default::default()
                    },
                    None,
                    || false,
                )
                .unwrap();
            result["outcome"] = json!(format!("{:?}", report.outcome));
            result["commits"] = json!(runner.execution().committed_steps());
            result["diagnostic"] = json!(report.diagnostic.map(|e| e.to_string()));
        }
        _ => panic!("unknown phase"),
    }
    result["wall_ns"] = json!(started.elapsed().as_nanos());
    println!("{result}");
}
