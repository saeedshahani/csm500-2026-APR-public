//! Authored C2 evidence through parser, checked admission and committed recording.
use engine::causal::{AnalysisLimits, DependencyKind, DependencyOrder};
use engine::recording::{EventId, Recording, RecordingLimits};
use engine::{Execution, ExecutionLimits};
use graph_core::Role;
#[test]
fn restoration_keeps_note_dependency_but_reattributes_the_exact_revision() {
    let input = parser::load(include_str!("../../../examples/programs/restoration.graph")).unwrap();
    let mut run = Execution::new(input.graph().clone(), ExecutionLimits::default()).unwrap();
    let mut log = Recording::new(&run, RecordingLimits::default()).unwrap();
    for _ in 0..3 {
        for (source, target) in [("p", "q"), ("q", "p")] {
            let h = input.name(target).unwrap();
            for actor in [source, "supervisor"] {
                let rank = run
                    .graph()
                    .iter()
                    .find(|(_, a)| a.kind.role() == Some(Role::Rank) && a.fields[0] == h)
                    .unwrap()
                    .0;
                let step = run
                    .enabled_for(input.name(actor).unwrap())
                    .unwrap()
                    .into_steps()
                    .into_iter()
                    .find(|s| s.event().consumed.contains(&rank))
                    .unwrap();
                log.commit(&mut run, step).unwrap();
            }
        }
    }
    let order = DependencyOrder::extract(log.events(), AnalysisLimits::default()).unwrap();
    for i in (2..log.events().len()).step_by(2) {
        let current = &log.events()[i];
        let lower = &log.events()[i - 2];
        let supervisor = &log.events()[i - 1];
        assert!(supervisor
            .footprint
            .produced
            .contains(&current.redex.revision));
        let revision_links = &order.edges()[&(supervisor.id, current.id)];
        assert!(revision_links
            .iter()
            .any(|w| w.kind == DependencyKind::ExactRevision && w.atom == current.redex.revision));
        let broader = &order.edges()[&(lower.id, current.id)];
        assert!(broader
            .iter()
            .any(|w| w.kind == DependencyKind::ProduceRead));
        assert!(broader
            .iter()
            .all(|w| w.kind != DependencyKind::ExactRevision));
    }
    assert!(order.edges().keys().all(|(a, b)| a < b));
    assert!(order
        .ancestors(EventId(10), AnalysisLimits::default())
        .unwrap()
        .contains(&EventId(0)));
}
