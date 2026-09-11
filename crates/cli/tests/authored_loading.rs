//! Authored loading and engine integration stay at their existing shared consumer.
//! Bindings are explicit; these are not automatic-matcher or printer tests.
use engine::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{Binding, ErrorKind, Kind, Sort};
use parser::{load, load_with_limits, ParseLimits};
use std::collections::BTreeMap;

const INSTALLER: &str = include_str!("../../../examples/syntax/install-wrap.graph");

#[test]
fn authored_package_installs_and_its_literal_variable_then_matches_object_data() {
    let loaded = load(INSTALLER).unwrap();
    let install = loaded.name("install").unwrap();
    let mut graph = loaded.into_graph();
    let input = graph
        .insert(Kind::Constructor("Input".to_owned()), &[])
        .unwrap();
    let mut execution = Execution::new(graph, ExecutionLimits::default()).unwrap();
    let proposal = execution
        .prepare(OperationalBinding::new(install, BTreeMap::new()))
        .unwrap();
    let event = execution.commit(proposal).unwrap();
    assert_eq!(event.produced.len(), 13);
    assert_eq!(event.entered.len(), 1);
    let head = *event.entered.iter().next().unwrap();
    let view = execution.view().unwrap();
    let inner = view.rule(head).unwrap();
    assert_eq!(inner.lhs.len(), 1);
    let variable = *inner.lhs.iter().next().unwrap();
    assert_eq!(
        execution.graph().get(variable).unwrap().kind,
        Kind::variable(Sort::Constructor, Binding::Free)
    );
    let proposal = execution
        .prepare(OperationalBinding::new(
            head,
            BTreeMap::from([(variable, input)]),
        ))
        .unwrap();
    assert_eq!(execution.commit(proposal).unwrap().produced.len(), 2);
}

#[test]
fn failed_authored_load_leaves_execution_and_prepared_proposal_unchanged() {
    let loaded = load(INSTALLER).unwrap();
    let install = loaded.name("install").unwrap();
    let mut execution = Execution::new(loaded.into_graph(), ExecutionLimits::default()).unwrap();
    let before = execution.graph().snapshot();
    let proposal = execution
        .prepare(OperationalBinding::new(install, BTreeMap::new()))
        .unwrap();
    for source in [
        "program { atom x = Nat(no_such_name); }",
        "program { rule missing_metadata {} }",
        "program { atom x = ; }",
    ] {
        let result = load(source);
        assert!(result.is_err());
        // A consumer calls replace_graph only after successful loading.
        if let Ok(loaded) = result {
            execution.replace_graph(loaded.into_graph()).unwrap();
        }
        assert_eq!(execution.graph().snapshot(), before);
    }
    assert!(execution.commit(proposal).is_ok());
}

#[test]
fn successful_authored_replacement_starts_a_new_history_even_for_same_source() {
    let loaded = load(INSTALLER).unwrap();
    let install = loaded.name("install").unwrap();
    let mut execution = Execution::new(loaded.into_graph(), ExecutionLimits::default()).unwrap();
    let proposal = execution
        .prepare(OperationalBinding::new(install, BTreeMap::new()))
        .unwrap();
    execution
        .replace_graph(load(INSTALLER).unwrap().into_graph())
        .unwrap();
    assert_eq!(
        execution.commit(proposal).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
}

#[test]
fn resource_limited_loading_does_not_replace_a_valid_baseline() {
    let loaded = load(INSTALLER).unwrap();
    let execution = Execution::new(loaded.into_graph(), ExecutionLimits::default()).unwrap();
    let before = execution.graph().snapshot();
    let limits = ParseLimits {
        graph: graph_core::GraphLimits {
            max_atoms: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        load_with_limits(INSTALLER, &limits).unwrap_err()[0]
            .diagnostic
            .kind(),
        ErrorKind::ResourceExhaustion
    );
    assert_eq!(execution.graph().snapshot(), before);
}
