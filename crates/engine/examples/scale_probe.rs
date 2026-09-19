//! Bounded native experimental profile. See docs/scalability-study.md.
#[path = "../benches/support/mod.rs"]
mod support;
use engine::observation::{isomorphic, Observer};
use engine::scheduling::{candidate_key, Scheduler, SchedulingStrategy};
use engine::{Execution, ExecutionLimits, SearchLimits};
use graph_core::{Graph, GraphLimits, Kind, NewAtom, Role, Target};
use serde_json::json;
use std::time::Instant;
use support::dataset;

fn limits() -> ExecutionLimits {
    ExecutionLimits {
        graph: GraphLimits {
            max_input_bytes: 64 * 1024 * 1024,
            max_atoms: 300_000,
            max_fields: 600_000,
            max_label_bytes: 32 * 1024 * 1024,
            max_validation_work: 100_000_000,
            ..Default::default()
        },
        search: SearchLimits {
            max_candidates: 100_000,
            max_work: 10_000_000,
        },
    }
}
fn fixture(family: &str, n: usize, seed: u32) -> Graph {
    let lim = limits();
    let mut graph = if family == "oscillating" {
        support::fixture("trace", 0)
    } else if family == "restoration" {
        parser::load(include_str!("../../../examples/programs/restoration.graph"))
            .unwrap()
            .into_graph()
    } else if family == "rules" {
        support::fixture("rules", 32)
    } else {
        parser::load(
            &dataset::source(
                &dataset::generate(
                    0,
                    seed,
                    if family == "guarded" {
                        dataset::Policy::GuardedEnable
                    } else {
                        dataset::Policy::Fixed
                    },
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .into_graph()
    };
    let mut batch = Vec::new();
    let mut rng = seed;
    for i in 0..n {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let label = if family == "symmetry" {
            "item:shared".into()
        } else {
            format!("item:{i}:{rng}")
        };
        let id = batch.len();
        batch.push(NewAtom {
            kind: Kind::Constructor(label),
            fields: vec![],
        });
        if !["oscillating", "restoration", "rules"].contains(&family) {
            batch.push(NewAtom {
                kind: Kind::Record {
                    role: Role::ObjectLink,
                    label: Some("pending".into()),
                },
                fields: vec![Target::Fresh(id), Target::Fresh(id)],
            });
        }
    }
    if family == "star" || family == "degree" {
        for i in 0..n {
            for slot in 0..if family == "degree" { 4 } else { 1 } {
                let target = if family == "star" {
                    0
                } else {
                    ((i + slot + 1) % n) * 2
                };
                batch.push(NewAtom {
                    kind: Kind::Record {
                        role: Role::ObjectLink,
                        label: Some("context".into()),
                    },
                    fields: vec![Target::Fresh(i * 2), Target::Fresh(target)],
                });
            }
        }
    }
    graph.insert_batch_with_limits(&batch, &lim.graph).unwrap();
    graph
}
fn domain_count(graph: &Graph, label: &str) -> usize {
    graph.iter().filter(|(_,a)| matches!(&a.kind,Kind::Record {role:Role::ObjectLink,label:Some(s)} if s==label)
        && a.fields.first().is_some_and(|id| matches!(&graph.get(*id).unwrap().kind,Kind::Constructor(s) if s.starts_with("item:")))).count()
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 5, "scale_probe FAMILY SIZE PHASE EVENTS SEED");
    let family = &args[0];
    let n: usize = args[1].parse().unwrap();
    let phase = &args[2];
    let events: u64 = args[3].parse().unwrap();
    let seed: u32 = args[4].parse().unwrap();
    assert!(n <= 100_000 && events <= 1000);
    let lim = limits();
    let setup = Instant::now();
    let graph = if family.starts_with("import:") {
        let source = std::fs::read_to_string(family.trim_start_matches("import:")).unwrap();
        parser::load_with_limits(
            &source,
            &parser::ParseLimits {
                graph: lim.graph,
                max_tokens: 4_000_000,
                ..Default::default()
            },
        )
        .unwrap()
        .into_graph()
    } else {
        fixture(family, n, seed)
    };
    let setup_ns = setup.elapsed().as_nanos();
    let mut run = Execution::new(graph, lim).unwrap();
    let mut result = json!({"schema":"scale-probe-v1","family":family,"size":n,"phase":phase,"seed":seed,
        "event_bound":events,"atoms":run.graph().len(),"fields":run.graph().field_count(),"setup_ns":setup_ns,
        "implementation":engine::trace::IMPLEMENTATION,"outcome":"Completed","commits":0});
    let prepared_binding = if phase == "prepare" {
        let head = *run
            .view()
            .unwrap()
            .configuration()
            .rules
            .keys()
            .next()
            .unwrap();
        Some(run.bindings_indexed(head).unwrap().remove(0))
    } else {
        None
    };
    let observation = if phase == "compare" {
        Some(
            Observer::all()
                .project(&run.view().unwrap(), Default::default())
                .unwrap(),
        )
    } else {
        None
    };
    let started = Instant::now();
    let operation = (|| -> graph_core::Result<()> {
        match phase.as_str() {
            "import" | "validate" => {
                std::hint::black_box(run.view()?);
            }
            "clone" => {
                std::hint::black_box(run.graph().clone());
            }
            "observe" => {
                std::hint::black_box(Observer::all().project(&run.view()?, Default::default())?);
            }
            "compare" => {
                let value = observation.as_ref().unwrap();
                assert!(isomorphic(value, value, 10_000_000)?);
            }
            "inspect" => {
                let bounded:Vec<_>=run.graph().iter().take(250).map(|(k,a)|json!({"id":k.number(),"kind":a.kind,"fields":a.fields.iter().map(|f|f.number()).collect::<Vec<_>>()})).collect();
                result["inspection_bytes"] = json!(serde_json::to_vec(&bounded).unwrap().len());
                result["inspection_atoms"] = json!(bounded.len());
            }
            "prepare" => {
                std::hint::black_box(run.prepare(prepared_binding.unwrap())?);
            }
            "baseline" => {
                let mut values = vec![false; n];
                for value in values.iter_mut().take(events as usize) {
                    *value = true;
                }
                result["processed"] = json!(values.iter().filter(|v| **v).count());
                std::hint::black_box(values);
            }
            "bindings-reference" | "bindings-indexed" => {
                let heads: Vec<_> = run.view()?.live_rules().map(|r| r.head).collect();
                let mut total = 0;
                for head in heads {
                    total += if phase.ends_with("indexed") {
                        run.bindings_indexed(head)?.len()
                    } else {
                        run.bindings(head)?.len()
                    };
                }
                result["complete_count"] = json!(total);
            }
            "enabled-reference" => {
                let set = run.enabled()?;
                result["complete_count"] = json!(set.steps().len());
                result["work"] = json!(set.work_used());
            }
            "enabled-compact" => {
                let set = run.enabled_compact()?;
                result["complete_count"] = json!(set.bindings().len());
                result["work"] = json!(set.work_used());
            }
            "run-reference" | "run-compact" | "record-reference" | "record-compact" => {
                let mut scheduler = Scheduler::seeded(u64::from(seed));
                let mut history = if phase.starts_with("record") {
                    Some(engine::recording::Recording::new(
                        &run,
                        engine::recording::RecordingLimits {
                            max_units: 10_000_000,
                            max_work: 10_000_000,
                            ..Default::default()
                        },
                    )?)
                } else {
                    None
                };
                loop {
                    // Complete query precedes terminal classification, matching
                    // the reference runner's normal-form versus bound priority.
                    let selected = if phase.ends_with("compact") {
                        run.enabled_compact()?.select(&scheduler)?
                    } else {
                        let mut steps = run.enabled()?.into_steps();
                        steps.sort_by_key(candidate_key);
                        let mut next = scheduler.clone();
                        next.select(&steps).map(|i| (steps.remove(i), next))
                    };
                    let Some((step, next)) = selected else {
                        result["outcome"] = json!("NormalForm");
                        break;
                    };
                    if run.committed_steps() >= events {
                        result["outcome"] = json!("StepLimit");
                        break;
                    }
                    if let Some(history) = &mut history {
                        history.commit(&mut run, step)?;
                    } else {
                        run.commit(step)?;
                    }
                    scheduler = next;
                }
                if let Some(history) = history {
                    result["recorded_events"] = json!(history.events().len());
                }
            }
            _ => panic!("unknown phase"),
        }
        Ok(())
    })();
    result["phase_ns"] = json!(started.elapsed().as_nanos());
    result["commits"] = json!(run.committed_steps());
    if let Err(error) = operation {
        result["outcome"] = json!(format!("{:?}", error.kind()));
        result["diagnostic"] = json!(error.to_string());
    }
    if ["fixed", "guarded", "symmetry", "star", "degree"].contains(&family.as_str()) {
        let processed = domain_count(run.graph(), "processed");
        let pending = domain_count(run.graph(), "pending");
        assert_eq!(processed + pending, n);
        if phase.starts_with("run") || phase.starts_with("record") {
            assert_eq!(
                processed as u64,
                run.committed_steps()
                    .saturating_sub(u64::from(family == "guarded"))
            );
        }
        if phase != "baseline" {
            result["processed"] = json!(processed);
        }
        result["pending"] = json!(pending);
        result["domain_invariant"] = json!(true);
    }
    println!("{result}");
}
