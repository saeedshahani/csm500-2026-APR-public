//! Authored-source integration; no operational binding is supplied by the caller.
use engine::{Execution, ExecutionLimits};
use graph_core::{fixture, Kind};
use parser::{load, print};

#[test]
fn authored_rule_revision_changes_automatically_discovered_output() {
    let loaded = load(include_str!("../../../examples/syntax/modify-output.graph")).unwrap();
    let target = loaded.name("target").unwrap();
    let supervisor = loaded.name("supervisor").unwrap();
    let mut run = Execution::new(loaded.into_graph(), ExecutionLimits::default()).unwrap();
    for expected in ["A", "B"] {
        let set = run.enabled_for(target).unwrap();
        assert_eq!(set.steps().len(), 1);
        let event = run.commit(set.into_steps().pop().unwrap()).unwrap();
        assert_eq!(
            run.graph()
                .get(*event.produced.first().unwrap())
                .unwrap()
                .kind,
            Kind::Constructor(expected.into())
        );
        if expected == "A" {
            let edits = run.enabled_for(supervisor).unwrap();
            assert_eq!(edits.steps().len(), 1);
            let edit = run.commit(edits.into_steps().pop().unwrap()).unwrap();
            assert!(edit.altered.contains(&target));
        }
    }
    let text = print(run.graph()).unwrap();
    assert_eq!(print(load(&text).unwrap().graph()).unwrap(), text);
}

#[test]
fn authored_installer_then_installed_rule_use_automatic_matching() {
    let text = include_str!("../../../examples/syntax/install-wrap.graph").replacen(
        "program {",
        "program { atom input = Constructor(\"Input\");",
        1,
    );
    let loaded = load(&text).unwrap();
    let input = loaded.name("input").unwrap();
    let mut run = Execution::new(loaded.into_graph(), ExecutionLimits::default()).unwrap();
    let enabled = run.enabled().unwrap();
    assert_eq!(enabled.steps().len(), 1);
    let installed = run.commit(enabled.into_steps().pop().unwrap()).unwrap();
    let head = *installed.entered.first().unwrap();
    let enabled = run.enabled_for(head).unwrap();
    assert_eq!(enabled.steps().len(), 1);
    let event = run.commit(enabled.into_steps().pop().unwrap()).unwrap();
    assert_eq!(event.produced.len(), 2);
    assert!(event.preserved.contains(&input));
}

#[test]
fn printed_arithmetic_and_self_lowering_run_without_hand_written_assignments() {
    for (ledger, consumed, produced) in [
        (include_str!("../../../examples/fixtures/a2.atoms"), 5, 1),
        (include_str!("../../../examples/fixtures/b2.atoms"), 3, 2),
    ] {
        let graph = fixture::load(ledger).unwrap().graph;
        let loaded = load(&print(&graph).unwrap()).unwrap();
        let mut run = Execution::new(loaded.into_graph(), ExecutionLimits::default()).unwrap();
        let enabled = run.enabled().unwrap();
        assert_eq!(enabled.steps().len(), 1);
        let event = run.commit(enabled.into_steps().pop().unwrap()).unwrap();
        assert_eq!(
            (event.consumed.len(), event.produced.len()),
            (consumed, produced)
        );
        assert!(run.enabled().unwrap().steps().is_empty());
    }
}
