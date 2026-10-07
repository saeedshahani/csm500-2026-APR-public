//! Bounded native experimental profile. See benchmarks/PROTOCOL.md.
#[path = "../benches/support/mod.rs"]
mod support;
use engine::observation::{isomorphic, isomorphic_indexed, Observer};
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
    assert!(n <= 100_000 && events <= 100_000);
    let lim = limits();
    let setup = Instant::now();
    let report_phase = |name: &str| {
        if std::env::var_os("REFLECTIVE_SCALE_PHASES").is_some() {
            eprintln!("scale-phase {name} {}", setup.elapsed().as_millis());
        }
    };
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
    report_phase("validated");
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
    let observation = if phase.starts_with("compare") {
        Some(
            Observer::all()
                .project(
                    &run.view().unwrap(),
                    engine::observation::ObservationLimits {
                        max_work: 10_000_000,
                        ..Default::default()
                    },
                )
                .unwrap(),
        )
    } else {
        None
    };
    let permuted = if phase == "compare-permuted" {
        let count = run.graph().len() as u64;
        let identities: std::collections::BTreeMap<_, _> = run
            .graph()
            .iter()
            .enumerate()
            .map(|(i, (key, _))| (key, count - 1 - i as u64))
            .collect();
        let mut snapshot: Vec<_> = run
            .graph()
            .iter()
            .map(|(key, atom)| {
                (
                    identities[&key],
                    atom.kind.clone(),
                    atom.fields.iter().map(|k| identities[k]).collect(),
                )
            })
            .collect();
        snapshot.sort_by_key(|(id, _, _)| *id);
        let graph = Graph::restore_identity_snapshot(&snapshot, count, &lim.graph).unwrap();
        Some(
            engine::observation::all(
                &graph,
                lim.graph,
                engine::observation::ObservationLimits {
                    max_work: 10_000_000,
                    ..Default::default()
                },
            )
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
                std::hint::black_box(Observer::all().project(
                    &run.view()?,
                    engine::observation::ObservationLimits {
                        max_work: 10_000_000,
                        ..Default::default()
                    },
                )?);
            }
            "compare" | "compare-indexed" => {
                let value = observation.as_ref().unwrap();
                assert!(if phase == "compare-indexed" {
                    isomorphic_indexed(value, value, 10_000_000)?
                } else {
                    isomorphic(value, value, 10_000_000)?
                });
            }
            "compare-permuted" => {
                assert!(isomorphic_indexed(
                    observation.as_ref().unwrap(),
                    permuted.as_ref().unwrap(),
                    100_000_000
                )?);
            }
            "pipeline-reference" | "pipeline-indexed" | "pipeline-framed" => {
                use engine::runner::{ExecutionProfile, RunLimits, Runner};
                use engine::trace::{TraceDocument, TraceLimits, TraceOptions, VerifiedTrace};
                let framed = phase.ends_with("framed");
                let profile = if framed {
                    ExecutionProfile::FramedV3
                } else if phase.ends_with("indexed") {
                    ExecutionProfile::IndexedV2
                } else {
                    ExecutionProfile::ReferenceV1
                };
                let mut trace_limits = TraceLimits {
                    execution: lim,
                    max_bytes: 64 * 1024 * 1024,
                    ..Default::default()
                };
                // Declared native large-job envelope, not the hosted/API policy.
                // Keep one initial and one endpoint checkpoint; every event is
                // still replayed and all recurrence observations remain enabled.
                if framed && n > 100 {
                    trace_limits.max_bytes = 512 * 1024 * 1024;
                    trace_limits.max_nodes = 40_000_000;
                    trace_limits.max_events = 100_000;
                    trace_limits.max_atoms_total = 2_000_000;
                    trace_limits.max_fields_total = 4_000_000;
                    trace_limits.max_bindings = 1_000_000;
                    trace_limits.max_replay_work = 100_000_000;
                    trace_limits.max_checkpoint_units = 100_000_000;
                    trace_limits.recording = engine::recording::RecordingLimits {
                        max_events: 100_000,
                        max_units: 100_000_000,
                        max_work: 100_000_000,
                    };
                    trace_limits.observation = engine::observation::ObservationLimits {
                        max_work: 10_000_000,
                        max_comparison_work: 100_000_000,
                        max_history_units: 100_000_000,
                    };
                    trace_limits.analysis = engine::causal::AnalysisLimits {
                        max_work: 100_000_000,
                        max_units: 100_000_000,
                    };
                }
                let mut runner = Runner::with_profile(
                    run.graph().clone(),
                    lim,
                    Scheduler::seeded(u64::from(seed)),
                    Observer::all(),
                    profile,
                )?;
                runner.enable_recording(trace_limits.recording)?;
                let initial_items: std::collections::BTreeMap<_, _> = run
                    .graph()
                    .iter()
                    .filter(
                        |(_, a)| matches!(&a.kind, Kind::Constructor(s) if s.starts_with("item:")),
                    )
                    .map(|(id, a)| (id, a.kind.clone()))
                    .collect();
                let run_started = Instant::now();
                report_phase("run-start");
                let report = runner.run(
                    RunLimits {
                        max_steps: events,
                        detect_cycles: true,
                        observation: trace_limits.observation,
                    },
                    None,
                    || false,
                )?;
                result["run_ns"] = json!(run_started.elapsed().as_nanos());
                report_phase("run-finished");
                result["retained_states"] = json!(runner.retained_states());
                result["outcome"] = json!(format!("{:?}", report.outcome));
                result["run_diagnostic"] =
                    json!(report.diagnostic.as_ref().map(ToString::to_string));
                result["commits"] = json!(runner.execution().committed_steps());
                result["processed"] = json!(domain_count(runner.execution().graph(), "processed"));
                result["pending"] = json!(domain_count(runner.execution().graph(), "pending"));
                if ["fixed", "guarded", "symmetry"].contains(&family.as_str()) {
                    let final_graph = runner.execution().graph();
                    let mut statuses = std::collections::BTreeMap::<_, usize>::new();
                    for (_, a) in final_graph.iter() {
                        if matches!(&a.kind, Kind::Record { role: Role::ObjectLink, label: Some(s) } if s == "pending" || s == "processed")
                            && initial_items.contains_key(&a.fields[0])
                        {
                            assert_eq!(a.fields[0], a.fields[1]);
                            *statuses.entry(a.fields[0]).or_default() += 1;
                        }
                    }
                    assert_eq!(statuses.len(), initial_items.len());
                    for (id, kind) in &initial_items {
                        assert_eq!(&final_graph.get(*id).unwrap().kind, kind);
                        assert_eq!(statuses[id], 1);
                    }
                    assert_eq!(
                        result["processed"].as_u64().unwrap() + result["pending"].as_u64().unwrap(),
                        n as u64
                    );
                    assert_eq!(
                        result["processed"].as_u64().unwrap(),
                        runner
                            .execution()
                            .committed_steps()
                            .saturating_sub(u64::from(family == "guarded"))
                    );
                    result["domain_verified"] = json!(true);
                }
                let export_started = Instant::now();
                report_phase("export-start");
                let document = TraceDocument::from_runner(
                    &runner,
                    None,
                    if framed && n > 100 {
                        TraceOptions {
                            checkpoint_interval: events.max(1),
                            conflict_states: Vec::new(),
                        }
                    } else {
                        TraceOptions::default()
                    },
                    trace_limits,
                )?;
                let bytes = document.to_json(trace_limits.max_bytes)?;
                report_phase("export-finished");
                result["export_ns"] = json!(export_started.elapsed().as_nanos());
                let import_started = Instant::now();
                let verified = VerifiedTrace::import(&bytes, trace_limits)?;
                report_phase("import-finished");
                assert_eq!(
                    verified
                        .state_at(runner.execution().committed_steps() as usize)?
                        .snapshot(),
                    runner.execution().graph().snapshot()
                );
                result["import_ns"] = json!(import_started.elapsed().as_nanos());
                result["trace_bytes"] = json!(bytes.len());
                result["replay_verified"] = json!(true);
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
    if !phase.starts_with("pipeline") {
        result["commits"] = json!(run.committed_steps());
    }
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
        if phase != "baseline" && !phase.starts_with("pipeline") {
            result["processed"] = json!(processed);
        }
        if !phase.starts_with("pipeline") {
            result["pending"] = json!(pending);
        }
        result["domain_invariant"] = json!(true);
    }
    println!("{result}");
}
