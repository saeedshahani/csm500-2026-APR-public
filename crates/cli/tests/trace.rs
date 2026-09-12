//! CLI-produced histories checked against independently authored expected carriers.
#[path = "../../parser/tests/support/mod.rs"]
#[allow(dead_code)]
mod oracle;
use engine::observation::{ObservationLimits, Observer};
use engine::trace::{GraphData, TraceLimits, VerifiedTrace};
use graph_core::{GraphLimits, ValidatedGraph};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}
fn export(
    name: &str,
    steps: &str,
    observer: &str,
    cycle: bool,
    conflict: bool,
) -> (Vec<u8>, VerifiedTrace) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cli"));
    command
        .args(["export"])
        .arg(path(name))
        .args([
            "--max-steps",
            steps,
            "--strategy",
            "seeded",
            "--seed",
            "42",
            "--checkpoint-interval",
            "1",
            "--detect-cycles",
            if cycle { "true" } else { "false" },
            "--observer",
        ])
        .arg(path(&format!("observers/{observer}.observer")));
    if conflict {
        command.args(["--conflict-state", "0"]);
    }
    let out = command.output().unwrap();
    assert!(
        matches!(out.status.code(), Some(0 | 3 | 6)),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let trace = VerifiedTrace::import(&out.stdout, TraceLimits::default()).unwrap();
    let code = match trace.document().report.as_ref().unwrap().outcome.as_str() {
        "NormalForm" => 0,
        "StepLimit" | "ResourceLimit" => 3,
        "CycleDetected" => 6,
        other => panic!("unexpected outcome {other}"),
    };
    assert_eq!(out.status.code(), Some(code));
    assert_eq!(
        trace
            .document()
            .settings
            .plan
            .as_ref()
            .unwrap()
            .seed
            .unwrap()
            .0,
        42
    );
    assert_eq!(
        trace.document().settings.plan.as_ref().unwrap().max_steps.0,
        steps.parse::<u64>().unwrap()
    );
    assert_eq!(
        trace.document().source_sha256.as_ref().unwrap(),
        &engine::trace::sha256(&std::fs::read(path(name)).unwrap())
    );
    (out.stdout, trace)
}
fn expected(trace: &VerifiedTrace, index: usize, name: &str) {
    let g = trace.state_at(index).unwrap();
    let expected = parser::load(&std::fs::read_to_string(path(name)).unwrap()).unwrap();
    assert_eq!(
        oracle::isomorphic(&g, expected.graph(), 1_000_000),
        Ok(true),
        "{name}"
    );
    assert_eq!(
        GraphData::from_graph(&g),
        GraphData::from_graph(&trace.state_from_initial(index).unwrap())
    );
}
#[test]
fn behaviour_goldens_cover_both_observers_and_independent_payloads() {
    for name in ["stable", "collapse", "oscillating"] {
        for observer in ["all", "value"] {
            for cycle in [false, true] {
                let (_, trace) = export(
                    &format!("behaviour/{name}.graph"),
                    "6",
                    observer,
                    cycle,
                    false,
                );
                let count = trace.document().events.len();
                assert_eq!(
                    count,
                    if name != "oscillating" {
                        1
                    } else if !cycle {
                        6
                    } else if observer == "all" {
                        2
                    } else {
                        1
                    }
                );
                for i in 0..=count {
                    let target = if i == 0 {
                        format!("behaviour/{name}.graph")
                    } else if name == "oscillating" {
                        format!(
                            "behaviour/oscillating.{}.expected.graph",
                            if i % 2 == 0 { "even" } else { "odd" }
                        )
                    } else {
                        format!("behaviour/{name}.expected.graph")
                    };
                    expected(&trace, i, &target);
                }
                let graph = trace.state_at(count).unwrap();
                let value = Observer::parse(
                    include_str!("../../../examples/observers/value.observer"),
                    |_| None,
                    1000,
                )
                .unwrap();
                let measure = value
                    .project(
                        &ValidatedGraph::new(&graph, &GraphLimits::default()).unwrap(),
                        ObservationLimits::default(),
                    )
                    .unwrap()
                    .value_payload();
                assert_eq!(measure, if name == "collapse" { 0 } else { 2 });
                if name != "oscillating" {
                    assert_eq!(trace.document().report.as_ref().unwrap().payload, None);
                }
            }
        }
    }
}
#[test]
fn s2_cli_golden_traces_preserve_operational_provenance() {
    for (name, steps, state, golden) in [
        ("arithmetic", "1", 1, "arithmetic.expected"),
        ("mutual-regress", "1", 0, "mutual-regress"),
        ("staging", "1", 1, "staging.installed.expected"),
        ("self-modify", "1", 1, "self-modify.first.expected"),
    ] {
        let (bytes, t) = export(
            &format!("programs/{name}.graph"),
            steps,
            "all",
            false,
            false,
        );
        expected(&t, state, &format!("programs/{golden}.graph"));
        let (again, _) = export(
            &format!("programs/{name}.graph"),
            steps,
            "all",
            false,
            false,
        );
        assert_eq!(bytes, again, "no provenance-erasing normalization");
    }
}
#[test]
fn independent_examples_have_no_edges_and_commute_in_both_orders() {
    for name in ["disjoint", "shared-read"] {
        let (_, trace) = export(
            &format!("independent/{name}.graph"),
            "2",
            "all",
            false,
            true,
        );
        expected(&trace, 2, &format!("independent/{name}.expected.graph"));
        assert!(trace.document().dependencies.as_ref().unwrap().is_empty());
        let snapshot = trace.document().conflicts[0].analysis.as_ref().unwrap();
        assert_eq!(snapshot.alternatives.len(), 2);
        assert_eq!(snapshot.pairs.len(), 1);
        let pair = &snapshot.pairs[0];
        assert!(pair.first_after_second.is_none() && pair.second_after_first.is_none());
        assert_eq!(pair.commutes, Some(true));
        assert_eq!(pair.corresponding_events, Some(true));
        let graph = parser::load(
            &std::fs::read_to_string(path(&format!("independent/{name}.graph"))).unwrap(),
        )
        .unwrap()
        .into_graph();
        for first in [0, 1] {
            let mut e = engine::Execution::new(graph.clone(), Default::default()).unwrap();
            let p = e.enabled().unwrap().into_steps().remove(first);
            e.commit(p).unwrap();
            let p = e.enabled().unwrap().into_steps().remove(0);
            e.commit(p).unwrap();
            assert_eq!(
                oracle::isomorphic(e.graph(), &trace.state_at(2).unwrap(), 1_000_000),
                Ok(true)
            );
        }
        assert!(engine::causal::DependencyOrder::extract(
            &trace.document().events,
            Default::default()
        )
        .unwrap()
        .closure(Default::default())
        .unwrap()
        .is_empty());
    }
}
fn pipe(command: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn cli_replay_validation_and_no_overwrite_are_atomic() {
    for (input, flags, code, outcome) in [
        (
            "behaviour/stable.graph",
            ["--max-work", "0"],
            3,
            "ResourceLimit",
        ),
        (
            "programs/mutual-regress.graph",
            ["--rule", "p"],
            4,
            "RequestedRuleUnavailable",
        ),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_cli"))
            .arg("export")
            .arg(path(input))
            .args(["--max-steps", "1"])
            .args(flags)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(code));
        let t = VerifiedTrace::import(&out.stdout, Default::default()).unwrap();
        assert_eq!(t.document().report.as_ref().unwrap().outcome, outcome);
        assert!(t.document().events.is_empty());
    }
    let (bytes, _) = export("behaviour/stable.graph", "1", "all", false, false);
    assert!(pipe(&["trace-check", "-"], &bytes).status.success());
    let out = pipe(&["replay", "-", "--at", "1"], &bytes);
    assert!(out.status.success());
    assert!(parser::load(std::str::from_utf8(&out.stdout).unwrap()).is_ok());
    let out = pipe(&["trace-check", "-"], b"{broken");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let out = pipe(&["replay", "-", "--at", "999"], &bytes);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let target = std::env::temp_dir().join(format!("trace-no-overwrite-{}", std::process::id()));
    let guard = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .unwrap();
    drop(guard);
    let out = pipe(
        &["replay", "-", "--output", target.to_str().unwrap()],
        &bytes,
    );
    assert!(!out.status.success());
    assert_eq!(std::fs::read(&target).unwrap(), b"");
    std::fs::remove_file(target).unwrap();
}

#[test]
fn explicit_s2_histories_replay_through_cli_across_installation_and_self_modification() {
    for name in ["staging", "self-modify", "restoration"] {
        let input = parser::load(
            &std::fs::read_to_string(path(&format!("programs/{name}.graph"))).unwrap(),
        )
        .unwrap();
        let mut e = engine::Execution::new(input.graph().clone(), Default::default()).unwrap();
        let mut log = engine::recording::Recording::new(&e, Default::default()).unwrap();
        let mut goldens = vec![(0, format!("programs/{name}.graph"))];
        if name == "staging" {
            let p = e
                .enabled_for(input.name("iw").unwrap())
                .unwrap()
                .into_steps()
                .remove(0);
            let child = *p.event().entered.first().unwrap();
            log.commit(&mut e, p).unwrap();
            goldens.push((1, "programs/staging.installed.expected.graph".into()));
            let p = e.enabled_for(child).unwrap().into_steps().remove(0);
            log.commit(&mut e, p).unwrap();
            goldens.push((2, "programs/staging.expected.graph".into()));
        } else if name == "self-modify" {
            let head = input.name("sl").unwrap();
            let mut member = input.name("sl_R_output_a").unwrap();
            for round in 0..2 {
                let p = e
                    .enabled_for(head)
                    .unwrap()
                    .into_steps()
                    .into_iter()
                    .find(|p| p.event().consumed.contains(&member))
                    .unwrap();
                let event = log.commit(&mut e, p).unwrap();
                goldens.push((
                    log.events().len(),
                    format!(
                        "programs/self-modify.{}expected.graph",
                        if round == 0 { "first." } else { "" }
                    ),
                ));
                member = *event
                    .produced
                    .iter()
                    .find(|k| {
                        e.graph().get(**k).unwrap().kind.role() == Some(graph_core::Role::Rhs)
                    })
                    .unwrap();
                if round == 0 {
                    let rank = e
                        .graph()
                        .iter()
                        .find(|(_, a)| {
                            a.kind.role() == Some(graph_core::Role::Rank) && a.fields[0] == head
                        })
                        .unwrap()
                        .0;
                    let p = e
                        .enabled_for(input.name("supervisor").unwrap())
                        .unwrap()
                        .into_steps()
                        .into_iter()
                        .find(|p| p.event().consumed.contains(&rank))
                        .unwrap();
                    log.commit(&mut e, p).unwrap();
                    goldens.push((2, "programs/self-modify.promoted.expected.graph".into()));
                }
            }
        } else {
            for (actor, target) in [
                ("p", "q"),
                ("supervisor", "q"),
                ("q", "p"),
                ("supervisor", "p"),
            ] {
                let head = input.name(target).unwrap();
                let rank = e
                    .graph()
                    .iter()
                    .find(|(_, a)| {
                        a.kind.role() == Some(graph_core::Role::Rank) && a.fields[0] == head
                    })
                    .unwrap()
                    .0;
                let p = e
                    .enabled_for(input.name(actor).unwrap())
                    .unwrap()
                    .into_steps()
                    .into_iter()
                    .find(|p| p.event().consumed.contains(&rank))
                    .unwrap();
                log.commit(&mut e, p).unwrap();
            }
            goldens.push((4, "programs/restoration.expected.graph".into()));
        }
        let doc = engine::trace::TraceDocument::from_recording(
            &log,
            &Observer::all(),
            None,
            engine::trace::TraceOptions {
                checkpoint_interval: 2,
                conflict_states: vec![],
            },
            Default::default(),
        )
        .unwrap();
        let bytes = doc.to_json(4_000_000).unwrap();
        let trace = VerifiedTrace::import(&bytes, Default::default()).unwrap();
        assert!(pipe(&["trace-check", "-"], &bytes).status.success());
        for (i, golden) in goldens {
            expected(&trace, i, &golden);
            let out = pipe(&["replay", "-", "--at", &i.to_string()], &bytes);
            assert!(out.status.success());
            let result = parser::load(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
            assert_eq!(
                oracle::isomorphic(result.graph(), &trace.state_at(i).unwrap(), 1_000_000),
                Ok(true)
            );
        }
    }
}

#[test]
fn published_trace_is_a_checked_one_event_prefix() {
    let bytes = include_bytes!("../../../examples/traces/stable.trace.json");
    let trace = VerifiedTrace::import(bytes, Default::default()).unwrap();
    assert_eq!(trace.document().status, "recorded_prefix");
    assert_eq!(trace.document().events.len(), 1);
    expected(&trace, 1, "behaviour/stable.expected.graph");
    assert!(pipe(&["trace-check", "-"], bytes).status.success());
}
