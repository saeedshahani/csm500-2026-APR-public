#[allow(dead_code)]
mod support;
use engine::observation::{ObservationLimits, Observer};
use engine::runner::{Outcome, RunLimits, Runner};
use engine::scheduling::{candidate_key, Scheduler, SchedulingStrategy};
use engine::{Execution, ExecutionLimits, OperationalBinding, PreparedStep};
use graph_core::{fixture, ErrorKind, Graph, Kind, Role};
use std::collections::{BTreeMap, BTreeSet};
use support::*;

fn loop_graph() -> Graph {
    let mut g = Graph::new();
    rule(&mut g, 0, false, true);
    g
}
fn limits(steps: u64, cycles: bool) -> RunLimits {
    RunLimits {
        max_steps: steps,
        detect_cycles: cycles,
        ..RunLimits::default()
    }
}
fn runner(g: Graph) -> Runner<Scheduler> {
    Runner::new(
        g,
        ExecutionLimits::default(),
        Scheduler::seeded(12),
        Observer::all(),
    )
    .unwrap()
}

#[test]
fn normality_zero_limit_and_requested_unavailability_are_distinct() {
    let mut empty = runner(Graph::new());
    assert_eq!(
        empty.run(limits(0, true), None, || false).unwrap().outcome,
        Outcome::NormalForm
    );
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, false);
    let mut r = runner(g);
    assert_eq!(
        r.run(limits(0, true), Some(h), || false).unwrap().outcome,
        Outcome::RequestedRuleUnavailable
    );
    let mut r = runner(loop_graph());
    let before = r.scheduler().description();
    assert_eq!(
        r.run(limits(0, false), None, || false).unwrap().outcome,
        Outcome::StepLimit
    );
    assert_eq!(r.scheduler().description(), before);
}

#[test]
fn repeat_is_diagnostic_and_precedes_step_limit_without_claiming_infinite_behaviour() {
    let mut r = runner(loop_graph());
    let report = r.run(limits(1, true), None, || false).unwrap();
    assert_eq!(report.outcome, Outcome::CycleDetected);
    let repeat = report.recurrence.unwrap();
    assert_eq!((repeat.first, repeat.repeated), (0, 1));
    assert!(repeat.complete_state_equal);
    assert_ne!(repeat.first_scheduler, repeat.repeated_scheduler);
    assert!(repeat.observer.contains("initial-carrier-sha256-debug-v1"));
    assert_eq!(r.execution().committed_steps(), 1);
}

#[test]
fn cancellation_at_each_boundary_rolls_back_selection_randomness() {
    for cancel_at in 1..=3 {
        let mut r = runner(loop_graph());
        let state = r.execution().graph().snapshot();
        let rng = r.scheduler().description();
        let mut calls = 0;
        assert_eq!(
            r.run(limits(9, true), None, || {
                calls += 1;
                calls == cancel_at
            })
            .unwrap()
            .outcome,
            Outcome::Cancelled
        );
        assert_eq!(r.execution().graph().snapshot(), state);
        assert_eq!(r.scheduler().description(), rng);
        assert_eq!(r.execution().committed_steps(), 0);
    }
}

#[test]
fn replacement_releases_history_and_resets_rng_only_on_success() {
    let g = loop_graph();
    let mut r = runner(g.clone());
    let initial = r.scheduler().description();
    r.run(limits(3, true), None, || false).unwrap();
    assert_eq!(r.retained_states(), 1);
    let mut invalid = Graph::new();
    invalid.insert(Kind::RuleHead, &[]).unwrap();
    let current = r.scheduler().description();
    assert!(r.replace_graph(invalid, Observer::all()).is_err());
    assert_eq!(r.scheduler().description(), current);
    assert_eq!(r.retained_states(), 1);
    r.replace_graph(g, Observer::all()).unwrap();
    assert_eq!(r.scheduler().description(), initial);
    assert_eq!(r.retained_states(), 0);
    assert_eq!(r.execution().committed_steps(), 0);
}

#[test]
fn observation_search_and_retention_exhaustion_are_inconclusive_and_atomic() {
    for which in 0..3 {
        let mut r = runner(loop_graph());
        let mut l = limits(9, true);
        match which {
            0 => l.observation.max_work = 0,
            1 => l.observation.max_comparison_work = 0,
            _ => l.observation.max_history_units = 0,
        };
        let report = r.run(l, None, || false).unwrap();
        assert_eq!(report.outcome, Outcome::ResourceLimit);
        assert_eq!(
            report.diagnostic.unwrap().kind(),
            ErrorKind::ResourceExhaustion
        );
        assert_eq!(
            r.execution().committed_steps(),
            if which == 1 { 1 } else { 0 }
        );
        r.execution().view().unwrap();
    }
    let mut e = ExecutionLimits::default();
    e.search.max_work = 0;
    let mut r = Runner::new(loop_graph(), e, Scheduler::Deterministic, Observer::all()).unwrap();
    assert_eq!(
        r.run(limits(0, true), None, || false).unwrap().outcome,
        Outcome::ResourceLimit
    );
}

#[derive(Clone)]
struct BadScheduler;
impl SchedulingStrategy for BadScheduler {
    fn select(&mut self, _: &[PreparedStep]) -> Option<usize> {
        Some(usize::MAX)
    }
    fn description(&self) -> String {
        "bad-test-scheduler".into()
    }
}
#[test]
fn unchecked_scheduler_cannot_forge_or_publish_a_step() {
    let g = loop_graph();
    let initial = g.snapshot();
    let mut r = Runner::new(g, ExecutionLimits::default(), BadScheduler, Observer::all()).unwrap();
    assert!(r.run(limits(1, false), None, || false).is_err());
    assert_eq!(r.execution().graph().snapshot(), initial);
    assert_eq!(r.execution().committed_steps(), 0);
}

#[test]
fn ordering_seed_repetition_and_rejected_preparations() {
    let mut g = loop_graph();
    rule(&mut g, 0, false, true);
    let e = Execution::new(g, ExecutionLimits::default()).unwrap();
    let mut left = e.enabled().unwrap().into_steps();
    let mut right = e.enabled().unwrap().into_steps();
    right.reverse();
    left.sort_by_cached_key(candidate_key);
    right.sort_by_cached_key(candidate_key);
    assert_eq!(
        left.iter().map(candidate_key).collect::<Vec<_>>(),
        right.iter().map(candidate_key).collect::<Vec<_>>()
    );
    let mut choices = BTreeSet::new();
    for seed in 0..32 {
        let mut a = Scheduler::seeded(seed);
        let mut b = Scheduler::seeded(seed);
        let before = b.description();
        assert!(e
            .prepare(OperationalBinding::new(
                e.graph().iter().nth(1).unwrap().0,
                BTreeMap::new()
            ))
            .is_err());
        assert_eq!(b.description(), before);
        for _ in 0..12 {
            let x = a.select(&left);
            assert_eq!(x, b.select(&right));
            choices.insert(x);
        }
    }
    assert_eq!(choices.len(), 2);
}

#[test]
fn hidden_metadata_repeat_does_not_imply_complete_state_repeat() {
    let f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let observer =
        Observer::parse("observer-v1\nselect data\nlink 76616c7565\n", |_| None, 100).unwrap();
    let mut r = Runner::new(
        f.graph,
        ExecutionLimits::default(),
        Scheduler::Deterministic,
        observer,
    )
    .unwrap();
    // A self-lowering rank-one rule becomes normal after one step: normality,
    // established by a complete query, outranks a selected-observer recurrence.
    assert_eq!(
        r.run(limits(1, true), None, || false).unwrap().outcome,
        Outcome::NormalForm
    );
}

#[test]
fn observer_selection_is_closed_and_input_identity_does_not_retarget() {
    let mut g = Graph::new();
    let a = ctor(&mut g, "a");
    let b = ctor(&mut g, "b");
    let link = link(&mut g, "value", a, b);
    let h = rule(&mut g, 0, false, false);
    let o = Observer::parse(
        "observer-v1\nselect carrier\ninput chosen\n",
        |n| (n == "chosen").then_some(link),
        100,
    )
    .unwrap();
    let e = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
    let p = o
        .project(&e.view().unwrap(), ObservationLimits::default())
        .unwrap();
    assert_eq!(p.atoms().len(), 3);
    assert_eq!(p.value_payload(), 2);
    g.remove_set(&BTreeSet::from([link])).unwrap();
    support::link(&mut g, "value", a, b);
    let e = Execution::new(g, ExecutionLimits::default()).unwrap();
    assert!(o
        .project(&e.view().unwrap(), ObservationLimits::default())
        .unwrap()
        .atoms()
        .is_empty());
    let o = Observer::parse("observer-v1\nselect carrier\nrole Rank\n", |_| None, 100).unwrap();
    let p = o
        .project(&e.view().unwrap(), ObservationLimits::default())
        .unwrap();
    assert!(p.atoms().contains_key(&h));
    assert_eq!(p.atoms().len(), 3);
    assert!(!p
        .atoms()
        .values()
        .any(|a| a.kind.role() == Some(Role::Revision)));
}

#[test]
fn observer_wire_format_and_exact_projection_budget() {
    for preset in [
        include_str!("../../../examples/observers/all.observer"),
        include_str!("../../../examples/observers/value.observer"),
    ] {
        assert_eq!(
            Observer::parse(preset, |_| None, 1000).unwrap().canonical(),
            preset
        );
    }
    let valid = "observer-v1\nselect data\nconstructor \nkind Nat\n";
    let o = Observer::parse(valid, |_| None, valid.len()).unwrap();
    assert_eq!(o.canonical(), valid);
    assert!(Observer::parse(valid, |_| None, valid.len() - 1).is_err());
    for invalid in [
        "observer-v1\nall",
        "observer-v1\r\nall\r\n",
        "observer-v2\nall\n",
        "observer-v1\nall\nkind Nat\n",
        "observer-v1\nselect data\nlink FF\n",
        "observer-v1\nselect data\nlink ff\n",
        "observer-v1\nselect data\nkind Nat\nkind Nat\n",
        "observer-v1\nselect data\ninput missing\n",
    ] {
        assert!(
            Observer::parse(invalid, |_| None, 1000).is_err(),
            "{invalid}"
        );
    }
    let e = Execution::new(Graph::new(), ExecutionLimits::default()).unwrap();
    let mut l = ObservationLimits {
        max_work: 0,
        ..ObservationLimits::default()
    };
    assert!(o.project(&e.view().unwrap(), l).is_err());
    l.max_work = 1;
    assert!(o.project(&e.view().unwrap(), l).is_ok());
}

#[test]
fn restricted_recurrence_can_hide_live_rank_modification() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    // A separate no-op remains enabled when SelfLower finishes, permitting an
    // observation lookup instead of global normal-form precedence.
    rule(&mut f.graph, 0, false, true);
    let o = Observer::parse("observer-v1\nselect data\nlink 76616c7565\n", |_| None, 100).unwrap();
    let mut r = Runner::new(
        f.graph,
        ExecutionLimits::default(),
        Scheduler::Deterministic,
        o,
    )
    .unwrap();
    let report = r.run(limits(1, true), None, || false).unwrap();
    assert_eq!(report.outcome, Outcome::CycleDetected);
    assert!(!report.recurrence.unwrap().complete_state_equal);
}

#[test]
fn history_exact_boundary_is_accepted_and_one_less_is_exhausted() {
    let g = loop_graph();
    let e = Execution::new(g.clone(), ExecutionLimits::default()).unwrap();
    let units = 2 * Observer::all()
        .project(&e.view().unwrap(), ObservationLimits::default())
        .unwrap()
        .retention_units()
        + Scheduler::seeded(12).description().len();
    for (budget, outcome) in [
        (units, Outcome::StepLimit),
        (units - 1, Outcome::ResourceLimit),
    ] {
        let mut r = runner(g.clone());
        let mut l = limits(0, true);
        l.observation.max_history_units = budget;
        assert_eq!(r.run(l, None, || false).unwrap().outcome, outcome);
    }
}
