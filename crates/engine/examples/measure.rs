//! Native controlled-region probe. stdout is one JSON result, never a timing claim
//! about a failed workload's throughput. See docs/performance-study.md.
#[path = "../benches/support/mod.rs"]
mod support;
use engine::{
    causal::DependencyOrder,
    runner::RunLimits,
    trace::{TraceDocument, TraceOptions, VerifiedTrace},
    Execution,
};
use serde_json::json;
use std::{hint::black_box, time::Instant};
fn timed<T>(f: impl FnOnce() -> T) -> (u128, T) {
    let start = Instant::now();
    let result = black_box(f());
    (start.elapsed().as_nanos(), result)
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3, "measure FAMILY SIZE PHASE");
    let family = &args[0];
    let n: usize = args[1].parse().unwrap();
    let phase = &args[2];
    if family == "calibration" {
        let (ns, data) = timed(|| {
            let mut bytes = vec![0u8; n * 1024 * 1024];
            for i in (0..bytes.len()).step_by(4096) {
                bytes[i] = 1;
            }
            black_box(bytes)
        });
        println!(
            "{}",
            json!({"family":family,"size":n,"phase":phase,"wall_ns":ns,"touched_bytes":data.len(),"outcome":"Completed"})
        );
        return;
    }
    let graph = support::fixture(family, n);
    support::check(&graph, family, n);
    let config = graph_core::validate(&graph).unwrap();
    let source = parser::print(&graph).unwrap();
    let mut execution = Execution::new(graph.clone(), Default::default()).unwrap();
    let mut result = json!({"family":family,"size":n,"phase":phase,"atoms":graph.len(),"fields":graph.field_count(),"rules":config.rules.len(),"max_lhs":config.rules.values().map(|r|r.lhs.len()).max().unwrap_or(0),"source_bytes":source.len(),"outcome":"Completed"});
    let ns = match phase.as_str() {
        "parse" => timed(|| parser::parse(&source).unwrap()).0,
        "load" => timed(|| parser::load(&source).unwrap()).0,
        "validate_decode" => timed(|| graph_core::validate(&graph).unwrap()).0,
        "scan" => timed(|| graph.of_sort(graph_core::Sort::Constructor)).0,
        "bindings" => {
            if let Some(rule) = config.rules.values().find(|r| r.active) {
                let (ns, found) = timed(|| execution.bindings(rule.head));
                match found {
                    Ok(b) => result["bindings"] = json!(b.len()),
                    Err(e) => result["outcome"] = json!(format!("{:?}", e.kind())),
                };
                ns
            } else {
                result["outcome"] = json!("Inapplicable");
                0
            }
        }
        "enabled" => {
            let (ns, found) = timed(|| execution.enabled());
            match found {
                Ok(set) => {
                    result["admitted"] = json!(set.steps().len());
                    result["rejected"] = json!(set.rejected().len());
                    result["work_units"] = json!(set.work_used());
                }
                Err(e) => result["outcome"] = json!(format!("{:?}", e.kind())),
            };
            ns
        }
        "prepare" | "commit" => {
            let binding = config
                .rules
                .values()
                .find(|r| r.active)
                .and_then(|r| execution.bindings(r.head).ok())
                .and_then(|b| b.into_iter().next());
            if let Some(binding) = binding {
                if phase == "prepare" {
                    let (ns, prepared) = timed(|| execution.prepare(binding));
                    if let Err(e) = prepared {
                        result["outcome"] = json!(format!("{:?}", e.kind()));
                    }
                    ns
                } else if let Ok(step) = execution.prepare(binding) {
                    let (ns, event) = timed(|| execution.commit(step).unwrap());
                    result["commits"] = json!(1);
                    result["produced"] = json!(event.produced.len());
                    ns
                } else {
                    result["outcome"] = json!("Inapplicable");
                    0
                }
            } else {
                result["outcome"] = json!("Inapplicable");
                0
            }
        }
        "run" => {
            let mut runner = support::runner(graph.clone());
            let bound = if family == "practical" || family == "guarded" {
                n + 1
            } else {
                20
            };
            let (ns, report) = timed(|| {
                runner
                    .run(
                        RunLimits {
                            max_steps: bound as u64,
                            ..Default::default()
                        },
                        None,
                        || false,
                    )
                    .unwrap()
            });
            result["outcome"] = json!(format!("{:?}", report.outcome));
            result["commits"] = json!(runner.execution().committed_steps());
            if family == "practical" || family == "guarded" {
                if n == 1000 {
                    assert_eq!(result["outcome"], "ResourceLimit");
                    assert_eq!(runner.execution().committed_steps(), 0);
                } else {
                    assert_eq!(result["outcome"], "NormalForm");
                    assert_eq!(
                        runner.execution().committed_steps(),
                        (n + usize::from(family == "guarded")) as u64
                    );
                }
            }
            if family == "lambda" {
                let readback =
                    parser::lambda::interpret(runner.execution().graph(), Default::default())
                        .unwrap();
                result["readback"] = json!(readback);
                assert_eq!(
                    result["outcome"],
                    if n == 3 { "StepLimit" } else { "NormalForm" }
                );
            }
            ns
        }
        "order" | "closure" | "publish-1" | "publish-8" | "publish-32" | "replay-1"
        | "replay-8" | "replay-32" => {
            assert_eq!(family, "trace");
            let runner = support::history(n);
            let events = runner.recording().unwrap().events();
            result["events"] = json!(events.len());
            result["recording_units"] = json!(runner.recording().unwrap().units());
            if phase == "order" {
                let (ns, order) =
                    timed(|| DependencyOrder::extract(events, Default::default()).unwrap());
                result["edges"] = json!(order.edges().len());
                ns
            } else if phase == "closure" {
                let order = DependencyOrder::extract(events, Default::default()).unwrap();
                let (ns, closure) = timed(|| order.closure(Default::default()).unwrap());
                result["pairs"] = json!(closure.len());
                ns
            } else {
                let interval = phase.split('-').nth(1).unwrap().parse().unwrap();
                let build = || {
                    TraceDocument::from_runner(
                        &runner,
                        None,
                        TraceOptions {
                            checkpoint_interval: interval,
                            ..Default::default()
                        },
                        Default::default(),
                    )
                    .unwrap()
                };
                if phase.starts_with("publish") {
                    let (ns, bytes) = timed(|| build().to_json(16 * 1024 * 1024).unwrap());
                    result["json_bytes"] = json!(bytes.len());
                    ns
                } else {
                    let doc = build();
                    result["json_bytes"] = json!(doc.to_json(16 * 1024 * 1024).unwrap().len());
                    let verified = VerifiedTrace::verify(doc, Default::default()).unwrap();
                    let expected = verified.state_from_initial(n - 1).unwrap();
                    result["inspected_state"] = json!(n - 1);
                    let (ns, state) = timed(|| verified.state_at(n - 1).unwrap());
                    assert_eq!(state.snapshot(), expected.snapshot());
                    ns
                }
            }
        }
        _ => panic!("unknown phase"),
    };
    result["wall_ns"] = if result["outcome"] == "Inapplicable" {
        serde_json::Value::Null
    } else {
        json!(ns)
    };
    println!("{result}");
}
