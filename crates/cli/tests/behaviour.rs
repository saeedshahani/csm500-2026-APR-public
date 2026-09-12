#[path = "../../parser/tests/support/mod.rs"]
#[allow(dead_code)]
mod oracle;
use engine::observation::{isomorphic, ObservationLimits, Observer};
use engine::{Execution, ExecutionLimits};
use graph_core::{Graph, GraphLimits, ValidatedGraph};
use std::path::PathBuf;
use std::process::Command;

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/behaviour")
        .join(format!("{name}.graph"))
}
fn load(name: &str) -> Graph {
    parser::load(&std::fs::read_to_string(path(name)).unwrap())
        .unwrap()
        .into_graph()
}
fn view(g: &Graph, observer: &Observer) -> engine::observation::Observation {
    observer
        .project(
            &ValidatedGraph::new(g, &GraphLimits::default()).unwrap(),
            ObservationLimits::default(),
        )
        .unwrap()
}
fn value() -> Observer {
    Observer::parse(
        include_str!("../../../examples/observers/value.observer"),
        |_| None,
        1000,
    )
    .unwrap()
}
fn expected(name: &str, step: usize) -> String {
    match name {
        "oscillating" => format!(
            "oscillating.{}.expected",
            if step % 2 == 0 { "even" } else { "odd" }
        ),
        _ => format!("{name}.expected"),
    }
}

#[test]
fn three_cases_agree_with_hand_derived_carriers_and_the_same_payload_measure() {
    for (name, horizon, final_measure) in
        [("stable", 1, 2), ("oscillating", 8, 2), ("collapse", 1, 0)]
    {
        let initial = load(name);
        let sizes = match name {
            "stable" => (20, 25),
            "collapse" => (17, 19),
            _ => (36, 46),
        };
        assert_eq!((initial.len(), initial.field_count()), sizes);
        let initial_value = view(&initial, &value());
        assert_eq!(initial_value.value_payload(), 2);
        let mut run = Execution::new(initial.clone(), ExecutionLimits::default()).unwrap();
        for step in 1..=horizon {
            let enabled = run.enabled().unwrap();
            assert_eq!(enabled.steps().len(), 1);
            assert!(enabled.rejected().is_empty());
            let event = run.commit(enabled.into_steps().remove(0)).unwrap();
            assert!(
                event.altered.is_empty() && event.entered.is_empty() && event.exited.is_empty()
            );
            assert_eq!(event.consumed.len(), 1);
            assert_eq!(event.produced.len(), usize::from(name != "collapse"));
            let wanted = load(&expected(name, step));
            assert!(oracle::isomorphic(run.graph(), &wanted, 1_000_000).unwrap());
            assert!(isomorphic(
                &view(run.graph(), &Observer::all()),
                &view(&wanted, &Observer::all()),
                1_000_000
            )
            .unwrap());
            let payload = view(run.graph(), &value());
            assert_eq!(payload.value_payload(), final_measure);
            assert_eq!(
                isomorphic(&payload, &initial_value, 1_000_000).unwrap(),
                name != "collapse"
            );
            assert_eq!(
                isomorphic(
                    &view(run.graph(), &Observer::all()),
                    &view(&initial, &Observer::all()),
                    1_000_000
                )
                .unwrap(),
                name == "oscillating" && step % 2 == 0
            );
        }
        assert_eq!(
            run.enabled().unwrap().steps().is_empty(),
            name != "oscillating"
        );
    }
}

#[test]
fn compiled_commands_retain_recurrence_observer_seed_and_threshold_evidence() {
    for name in ["stable", "oscillating", "collapse"] {
        for observer in ["all", "value"] {
            let observer_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/observers")
                .join(format!("{observer}.observer"));
            let output = Command::new(env!("CARGO_BIN_EXE_cli"))
                .args([
                    "run",
                    path(name).to_str().unwrap(),
                    "--max-steps",
                    "6",
                    "--strategy",
                    "seeded",
                    "--seed",
                    "42",
                    "--observer",
                    observer_path.to_str().unwrap(),
                    "--detect-cycles",
                    "true",
                ])
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(if name == "oscillating" { 6 } else { 0 }),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            for field in [
                "seed=42",
                "ChaCha8Rng",
                "nontriviality-threshold: 2",
                "initial-carrier-sha256-debug-v1",
                "observer-v1",
                "observation-limits:",
            ] {
                assert!(text.contains(field), "{text}");
            }
            let step = if name == "oscillating" && observer == "all" {
                2
            } else {
                1
            };
            assert!(text.contains(&format!("committed-steps: {step}")));
            if name == "oscillating" {
                assert!(text.contains("not a proof"));
                assert!(text.contains(&format!("complete-state-equal={}", observer == "all")));
            }
            let final_graph = parser::load(&text).unwrap().into_graph();
            assert!(
                oracle::isomorphic(&final_graph, &load(&expected(name, step)), 1_000_000).unwrap()
            );
            assert_eq!(
                view(&final_graph, &value()).value_payload(),
                if name == "collapse" { 0 } else { 2 }
            );
        }
    }
}

#[test]
fn finite_prefixes_and_exact_step_bound_do_not_change_the_mathematical_classification() {
    for name in ["stable", "oscillating", "collapse"] {
        for steps in [0, 1, 6] {
            let output = Command::new(env!("CARGO_BIN_EXE_cli"))
                .args([
                    "run",
                    path(name).to_str().unwrap(),
                    "--max-steps",
                    &steps.to_string(),
                ])
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(if name != "oscillating" && steps > 0 {
                    0
                } else {
                    3
                })
            );
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains(if name != "oscillating" && steps > 0 {
                "outcome: NormalForm"
            } else {
                "outcome: StepLimit"
            }));
            parser::load(&text).unwrap();
        }
    }
}
