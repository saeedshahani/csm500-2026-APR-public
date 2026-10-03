use engine::recording::RecordingLimits;
use engine::{
    workflow::{self, Session},
    Execution, ExecutionLimits,
};
use graph_core::{Kind, Role};

#[test]
fn finite_permits_and_persistent_modulation_have_distinct_guarantees() {
    for persistent in [false, true] {
        let (graph, contract) = workflow::example(false, 4, persistent).unwrap();
        let report = workflow::check(&graph, &contract, ExecutionLimits::default()).unwrap();
        assert_eq!(report.all_orders_transaction_bound.is_none(), persistent);
        let mut run = Session::new(
            graph,
            contract,
            ExecutionLimits::default(),
            RecordingLimits::default(),
            1,
        )
        .unwrap();
        let bound: usize = run.completion_bound().parse().unwrap();
        for _ in 0..bound {
            if run.report().unfinished == 0 {
                break;
            }
            run.step().unwrap().unwrap();
        }
        assert_eq!(run.report().unfinished, 0);
        let before = serde_json::to_value(&run.report().records).unwrap();
        if persistent {
            for _ in 0..8 {
                assert!(!run.step().unwrap().unwrap().altered.is_empty());
            }
        }
        assert_eq!(serde_json::to_value(&run.report().records).unwrap(), before);
    }
}

#[test]
fn branching_tasks_complete_with_fifo_and_preserved_parent_payloads() {
    for count in [1, 4, 12] {
        let (graph, contract) = workflow::example(true, count, true).unwrap();
        let mut run = Session::new(
            graph,
            contract,
            ExecutionLimits::default(),
            RecordingLimits::default(),
            2,
        )
        .unwrap();
        let parents = run.report().records.clone();
        let bound: usize = run.completion_bound().parse().unwrap();
        for _ in 0..bound {
            if run.report().unfinished == 0 {
                break;
            }
            run.step().unwrap().unwrap();
        }
        assert_eq!(run.report().unfinished, 0);
        assert_eq!(run.report().records.len(), count * 3);
        for old in parents {
            let new = run
                .report()
                .records
                .iter()
                .find(|r| r.ticket == old.ticket)
                .unwrap();
            assert_eq!(old.payload, new.payload);
            assert_eq!(old.payload_identity, new.payload_identity);
        }
    }
}

#[test]
fn every_reference_successor_preserves_the_recognised_profile() {
    for branching in [false, true] {
        for persistent in [false, true] {
            let (mut graph, contract) = workflow::example(branching, 2, persistent).unwrap();
            for _ in 0..4 {
                let before =
                    workflow::check(&graph, &contract, ExecutionLimits::default()).unwrap();
                let execution = Execution::new(graph.clone(), ExecutionLimits::default()).unwrap();
                let steps = execution.enabled().unwrap().into_steps();
                for step in &steps {
                    let mut e = Execution::new(graph.clone(), ExecutionLimits::default()).unwrap();
                    let p = e.prepare(step.binding().clone()).unwrap();
                    let event = e.commit(p).unwrap();
                    let after =
                        workflow::check(e.graph(), &contract, ExecutionLimits::default()).unwrap();
                    let b: u128 = before.work_potential.parse().unwrap();
                    let a: u128 = after.work_potential.parse().unwrap();
                    if event.altered.is_empty() {
                        assert!(a < b);
                    } else {
                        assert_eq!(a, b);
                    }
                }
                let Some(step) = steps.into_iter().find(|s| s.event().altered.is_empty()) else {
                    break;
                };
                let mut e = Execution::new(graph, ExecutionLimits::default()).unwrap();
                let p = e.prepare(step.binding().clone()).unwrap();
                e.commit(p).unwrap();
                graph = e.graph().clone();
            }
        }
    }
}

#[test]
fn starvation_is_possible_without_the_opt_in_scheduler() {
    let (graph, contract) = workflow::example(false, 1, true).unwrap();
    let mut e = Execution::new(graph, ExecutionLimits::default()).unwrap();
    for _ in 0..8 {
        let s = e
            .enabled()
            .unwrap()
            .into_steps()
            .into_iter()
            .find(|s| !s.event().altered.is_empty())
            .unwrap();
        e.commit(s).unwrap();
    }
    assert_eq!(
        workflow::check(e.graph(), &contract, ExecutionLimits::default())
            .unwrap()
            .unfinished,
        1
    );
}

#[test]
fn unsafe_contracts_and_native_valid_extra_structure_are_not_certified() {
    let (graph, c) = workflow::example(true, 1, true).unwrap();
    let mut bad = c.clone();
    bad.weights.insert("split".into(), "4".into());
    assert!(workflow::check(&graph, &bad, ExecutionLimits::default()).is_err());
    let mut bad = c.clone();
    bad.routes
        .retain(|r| !(r.from == "received" && r.to == "archive"));
    assert!(workflow::check(&graph, &bad, ExecutionLimits::default()).is_err());
    let mut extra = graph.clone();
    extra
        .insert(Kind::Constructor("harmless".into()), &[])
        .unwrap();
    graph_core::validate(&extra).unwrap();
    assert!(workflow::check(&extra, &c, ExecutionLimits::default()).is_err());
    let mut inactive = graph.clone();
    let active = inactive
        .iter()
        .find(|(_, a)| a.kind == Kind::record(Role::Active))
        .unwrap()
        .0;
    inactive.remove_set(&[active].into()).unwrap();
    graph_core::validate(&inactive).unwrap();
    assert!(workflow::check(&inactive, &c, ExecutionLimits::default()).is_err());
}

#[test]
fn reset_starts_a_new_history_and_preserves_identity() {
    let (graph, c) = workflow::example(false, 1, true).unwrap();
    let mut run = Session::new(
        graph,
        c,
        ExecutionLimits::default(),
        RecordingLimits::default(),
        0,
    )
    .unwrap();
    while run.report().unfinished > 0 {
        run.step().unwrap();
    }
    let task = run.report().records[0].clone();
    let mut reset = run
        .reset_status(&task.ticket, "received", RecordingLimits::default())
        .unwrap();
    assert!(reset.recording().events().is_empty());
    assert!(!run.recording().events().is_empty());
    while reset.report().unfinished > 0 {
        reset.step().unwrap();
    }
    assert_eq!(
        reset.report().records[0].payload_identity,
        task.payload_identity
    );
    assert!(reset
        .reset_status(&task.ticket, "archive", RecordingLimits::default())
        .is_err());
}

#[test]
fn native_valid_guard_and_field_mutants_are_unsupported() {
    let (graph, contract) = workflow::example(false, 2, true).unwrap();
    let config = graph_core::validate(&graph).unwrap();
    let supervisor = config.rules.values().find(|r| r.meta).unwrap();
    let guard = *supervisor
        .lhs
        .iter()
        .find(|k| graph.get(**k).unwrap().kind == Kind::record(Role::ModeObject))
        .unwrap();
    let mut deleted = graph.references_to(guard);
    deleted.insert(guard);
    let mut mutant = graph.clone();
    mutant.remove_set(&deleted).unwrap();
    graph_core::validate(&mutant).unwrap();
    assert!(workflow::check(&mutant, &contract, ExecutionLimits::default()).is_err());

    let report = workflow::check(&graph, &contract, ExecutionLimits::default()).unwrap();
    let identity: u64 = report.records[0].identity_link.parse().unwrap();
    let atoms: Vec<_> = graph
        .iter()
        .map(|(k, a)| {
            let mut fields: Vec<_> = a.fields.iter().map(|f| f.number()).collect();
            if k.number() == identity {
                fields.reverse();
            }
            (k.number(), a.kind.clone(), fields)
        })
        .collect();
    let mutant = graph_core::Graph::restore_identity_snapshot(
        &atoms,
        graph.next_identity(),
        &ExecutionLimits::default().graph,
    )
    .unwrap();
    assert!(workflow::check(&mutant, &contract, ExecutionLimits::default()).is_err());
}

#[test]
fn persistent_fifo_serves_each_initial_task_before_repeating_one() {
    let (graph, contract) = workflow::example(false, 4, true).unwrap();
    let mut run = Session::new(
        graph,
        contract,
        ExecutionLimits::default(),
        RecordingLimits::default(),
        2,
    )
    .unwrap();
    let order: Vec<_> = run
        .report()
        .records
        .iter()
        .map(|r| r.ticket.clone())
        .collect();
    let mut serviced = Vec::new();
    while serviced.len() < order.len() {
        let before = run.report().records.clone();
        let event = run.step().unwrap().unwrap();
        if event.altered.is_empty() {
            let changed: Vec<_> = before
                .iter()
                .filter(|r| {
                    run.report()
                        .records
                        .iter()
                        .find(|n| n.ticket == r.ticket)
                        .unwrap()
                        .stage
                        != r.stage
                })
                .map(|r| r.ticket.clone())
                .collect();
            assert_eq!(changed.len(), 1);
            serviced.extend(changed);
        }
    }
    assert_eq!(serviced, order);
}

#[test]
fn resource_exhaustion_and_recording_failure_do_not_commit() {
    let (graph, c) = workflow::example(false, 1, true).unwrap();
    let mut limits = ExecutionLimits::default();
    limits.search.max_work = 0;
    assert_eq!(
        workflow::check(&graph, &c, limits).unwrap_err().kind(),
        graph_core::ErrorKind::ResourceExhaustion
    );
    let mut run = Session::new(
        graph,
        c,
        ExecutionLimits::default(),
        RecordingLimits {
            max_events: 0,
            ..RecordingLimits::default()
        },
        1,
    )
    .unwrap();
    let before = run.graph().snapshot();
    assert!(run.step().is_err());
    assert_eq!(run.graph().snapshot(), before);
    assert!(run.recording().events().is_empty());
}
