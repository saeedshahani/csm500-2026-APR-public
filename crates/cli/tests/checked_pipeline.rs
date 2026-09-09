//! Cross-crate integration belongs at the CLI boundary: parser and engine remain
//! production siblings. S2.E will add authored parse/print/isomorphism cases here
//! once those APIs exist. This test uses the existing ledger reader, not a parser.
use engine::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::fixture;
use std::collections::BTreeMap;

#[test]
fn ledger_to_checked_execution_is_a_cross_crate_integration_baseline() {
    let f = fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let installer = f.names["iw"];
    let mut execution = Execution::new(f.graph, ExecutionLimits::default()).unwrap();
    let prepared = execution
        .prepare(OperationalBinding::new(installer, BTreeMap::new()))
        .unwrap();
    assert_eq!(prepared.event().produced.len(), 13);
    let event = execution.commit(prepared).unwrap();
    let view = execution.view().unwrap();
    assert_eq!(view.configuration().rules.len(), 2);
    assert_eq!(event.entered.len(), 1);
}
