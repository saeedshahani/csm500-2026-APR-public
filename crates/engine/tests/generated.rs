#[path = "../../graph-core/tests/support/generated.rs"]
mod generated;
use engine::observation::{all, isomorphic, ObservationLimits, Observer};
use engine::runner::{Outcome, RunLimits, Runner};
use engine::scheduling::Scheduler;
use engine::{Execution, ExecutionLimits, OperationalBinding};
use generated::*;
use graph_core::{validate, AtomKey, ErrorKind, Graph, GraphLimits, Kind, Role, Sort};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;
use std::collections::{BTreeMap, BTreeSet};

fn observe(g: &Graph) -> engine::observation::Observation {
    all(g, GraphLimits::default(), ObservationLimits::default()).unwrap()
}
fn next(g: &Graph) -> AtomKey {
    g.clone().insert(Kind::Nat, &[]).unwrap()
}

#[test]
fn generated_ln_sequences_preserve_ground_typing_and_freshness() {
    check(
        "LN successful sequences",
        "proptest-regressions/ln.txt",
        objects(),
        |recipe| {
            let mut run = Execution::new(ln(&recipe), ExecutionLimits::default()).unwrap();
            let mut seen: BTreeSet<_> = run.graph().iter().map(|(k, _)| k).collect();
            for _ in &recipe.edges {
                let before = run.graph().clone();
                let set = run.enabled().unwrap();
                prop_assert!(!set.steps().is_empty());
                let proposal = set.into_steps().remove(0);
                prop_assert_eq!(
                    proposal
                        .binding()
                        .assignments()
                        .values()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .len(),
                    proposal.binding().assignments().len()
                );
                for (id, atom) in proposal.ground_rule().left() {
                    prop_assert_eq!(before.get(*id), Some(atom));
                    prop_assert!(atom
                        .fields
                        .iter()
                        .all(|f| proposal.ground_rule().left().contains_key(f)));
                }
                let event = run.commit(proposal).unwrap();
                validate(run.graph()).unwrap();
                prop_assert!(event.produced.is_disjoint(&seen));
                seen.extend(&event.produced);
                for (key, atom) in before.iter() {
                    if !event.consumed.contains(&key) {
                        prop_assert_eq!(run.graph().get(key), Some(atom));
                    }
                }
                prop_assert_eq!(event.consumed.len(), 1);
                prop_assert_eq!(event.produced.len(), 1);
                prop_assert!(event.altered.is_empty());
            }
            prop_assert!(run.enabled().unwrap().steps().is_empty());
            Ok(())
        },
    );
}
#[test]
fn generated_shared_schedulers_repeat_and_converge_only_in_ln() {
    check(
        "LN schedules",
        "proptest-regressions/schedules.txt",
        (objects(), any::<u64>()),
        |(recipe, seed)| {
            let g = ln(&recipe);
            let execute = |scheduler| {
                let mut r = Runner::new(
                    g.clone(),
                    ExecutionLimits::default(),
                    scheduler,
                    Observer::all(),
                )
                .unwrap();
                let report = r
                    .run(
                        RunLimits {
                            max_steps: recipe.edges.len() as u64,
                            ..RunLimits::default()
                        },
                        None,
                        || false,
                    )
                    .unwrap();
                assert_eq!(report.outcome, Outcome::NormalForm);
                (r.execution().graph().clone(), report.scheduler)
            };
            let (a, _) = execute(Scheduler::Deterministic);
            let (b, state) = execute(Scheduler::seeded(seed));
            let (c, repeated) = execute(Scheduler::seeded(seed));
            prop_assert_eq!(b.snapshot(), c.snapshot());
            prop_assert_eq!(state, repeated);
            prop_assert!(isomorphic(&observe(&a), &observe(&b), 1_000_000).unwrap());
            Ok(())
        },
    );
}
#[test]
fn generated_reflective_sequences_redecode_ranks_and_exact_revisions() {
    check(
        "reflective sequences",
        "proptest-regressions/reflective.txt",
        proptest::collection::vec(0usize..4, 1..4),
        |ranks| {
            let mut g = Graph::new();
            for rank in &ranks {
                append(&mut g, &lowerer(*rank, true, false));
            }
            let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
            let mut seen: BTreeSet<_> = run.graph().iter().map(|(k, _)| k).collect();
            for _ in 0..ranks.iter().sum::<usize>() {
                let pre = validate(run.graph()).unwrap();
                let proposal = run.enabled().unwrap().into_steps().remove(0);
                let event = run.commit(proposal).unwrap();
                let post = validate(run.graph()).unwrap();
                prop_assert_eq!(&event.altered, &BTreeSet::from([event.source]));
                prop_assert!(event.entered.is_empty() && event.exited.is_empty());
                prop_assert_eq!(
                    post.rules[&event.source].rank + 1,
                    pre.rules[&event.source].rank
                );
                prop_assert!(event.consumed.contains(&pre.rules[&event.source].revision));
                prop_assert!(event.produced.contains(&post.rules[&event.source].revision));
                prop_assert!(event.produced.is_disjoint(&seen));
                seen.extend(event.produced);
                prop_assert_eq!(
                    run.view().unwrap().rule(event.source).unwrap().rank,
                    post.rules[&event.source].rank
                );
            }
            prop_assert!(run.enabled().unwrap().steps().is_empty());
            Ok(())
        },
    );
}
#[test]
fn generated_packages_and_inactive_partial_representations_are_valid() {
    check(
        "packages and partial bodies",
        "proptest-regressions/packages.txt",
        (0usize..4, any::<bool>(), 0usize..4),
        |(depth, active, rank)| {
            validate(&lowerer(rank, false, true)).unwrap();
            let g = nested(depth, active);
            let config = validate(&g).unwrap();
            prop_assert_eq!(config.rules.len(), 1);
            if active && depth > 0 {
                let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
                let proposal = run.enabled().unwrap().into_steps().remove(0);
                let event = run.commit(proposal).unwrap();
                prop_assert_eq!(event.entered.len(), 1);
                let post = run.view().unwrap();
                let entered = post.rule(*event.entered.first().unwrap()).unwrap();
                prop_assert_eq!(entered.rank, depth - 1);
                prop_assert!(event.produced.contains(&entered.revision));
            }
            Ok(())
        },
    );
}
#[test]
fn generated_multi_target_promotions_and_negative_transactions() {
    check(
        "multi-target success/GC/lifecycle",
        "proptest-regressions/promotions.txt",
        0usize..4,
        |rank| {
            for fault in 0..3 {
                let (g, source, binding, targets) = promote(rank, fault == 1, fault == 2);
                let baseline = g.snapshot();
                let allocator = next(&g);
                let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
                let candidate = run.prepare(OperationalBinding::new(source, binding));
                if fault == 0 {
                    let event = run.commit(candidate.unwrap()).unwrap();
                    prop_assert_eq!(event.altered, targets.clone());
                    let view = run.view().unwrap();
                    for h in targets {
                        let r = view.rule(h).unwrap();
                        prop_assert!(r.meta);
                        prop_assert!(event.produced.contains(&r.revision));
                    }
                } else {
                    prop_assert_eq!(
                        candidate.unwrap_err().kind(),
                        if fault == 1 {
                            ErrorKind::Guardedness
                        } else {
                            ErrorKind::Lifecycle
                        }
                    );
                    prop_assert_eq!(run.graph().snapshot(), baseline);
                    prop_assert_eq!(next(run.graph()), allocator);
                }
            }
            Ok(())
        },
    );
}

// Each recipe retains the offending structure while unrelated object noise and
// rank shrink. Category is not inferred from arbitrary random invalid bytes.
#[test]
fn generated_targeted_invalid_categories_are_distinct_and_atomic() {
    check(
        "targeted invalid families",
        "proptest-regressions/invalid.txt",
        objects(),
        |recipe| {
            for category in 0..6 {
                let mut g = object_graph(&recipe, false);
                let source = rule(&mut g, 1, category != 4, true);
                let mut binding = BTreeMap::new();
                match category {
                    0 => {
                        let keep = variable(&mut g, Sort::Nat);
                        let delete = variable(&mut g, Sort::Nat);
                        sides(&mut g, source, &[keep, delete], &[keep]);
                        let zero = g.insert(Kind::Nat, &[]).unwrap();
                        let one = g.insert(Kind::Nat, &[zero]).unwrap();
                        binding.extend([(keep, one), (delete, zero)]);
                    }
                    1 => {
                        let other = rule(&mut g, 0, false, false);
                        let item = ctor(&mut g, "shared");
                        sides(&mut g, source, &[item], &[item]);
                        sides(&mut g, other, &[item], &[item]);
                    }
                    2 => {
                        let child = g.insert(Kind::RuleHead, &[]).unwrap();
                        sides(&mut g, source, &[], &[child]);
                    }
                    3 => {
                        let revision = owned(&g, source, Role::Revision);
                        g.remove_set(&BTreeSet::from([revision])).unwrap();
                    }
                    4 => {
                        let edge = variable(&mut g, Sort::Record(Role::ObjectLink));
                        sides(&mut g, source, &[edge], &[edge]);
                        let other = rule(&mut g, 0, false, false);
                        let hidden = ctor(&mut g, "hidden");
                        sides(&mut g, other, &[hidden], &[hidden]);
                        let visible = ctor(&mut g, "visible");
                        let target = link(&mut g, "ground", visible, hidden);
                        binding.insert(edge, target);
                    }
                    _ => {
                        let x = variable(&mut g, Sort::Nat);
                        sides(&mut g, source, &[x], &[x]);
                    }
                }
                let before = g.snapshot();
                let allocator = next(&g);
                if (1..=3).contains(&category) {
                    prop_assert!(validate(&g).is_err());
                    prop_assert_eq!(g.snapshot(), before);
                    prop_assert_eq!(next(&g), allocator);
                } else {
                    let run = Execution::new(g, ExecutionLimits::default()).unwrap();
                    let error = run
                        .prepare(OperationalBinding::new(source, binding))
                        .unwrap_err();
                    prop_assert_eq!(
                        error.kind(),
                        if category == 0 {
                            ErrorKind::Dangling
                        } else {
                            ErrorKind::InvalidBinding
                        }
                    );
                    if category == 4 {
                        prop_assert_eq!(error.0, "object grounding enters rule representation");
                    }
                    prop_assert_eq!(run.graph().snapshot(), before);
                    prop_assert_eq!(next(run.graph()), allocator);
                }
            }
            Ok(())
        },
    );
}

#[test]
fn recipe_shrinking_retains_valid_references_and_failure_preconditions() {
    let mut runner = TestRunner::deterministic();
    for _ in 0..16 {
        let mut tree = (objects(), 0usize..4, 1usize..4)
            .new_tree(&mut runner)
            .unwrap();
        loop {
            let (recipe, rank, depth) = tree.current();
            validate(&object_graph(&recipe, false)).unwrap();
            validate(&ln(&recipe)).unwrap();
            validate(&lowerer(rank, false, true)).unwrap();
            validate(&nested(depth, true)).unwrap();
            let (g, h, b, _) = promote(rank, true, false);
            let run = Execution::new(g, ExecutionLimits::default()).unwrap();
            assert_eq!(
                run.prepare(OperationalBinding::new(h, b))
                    .unwrap_err()
                    .kind(),
                ErrorKind::Guardedness
            );
            if !tree.simplify() {
                break;
            }
        }
    }
}

#[test]
fn generated_live_exit_consumes_the_complete_representation_and_revision() {
    check(
        "live exits",
        "proptest-regressions/exits.txt",
        (0usize..4, any::<bool>()),
        |(rank, active)| {
            let mut g = Graph::new();
            let target = rule(&mut g, rank, false, active);
            let representation = validate(&g).unwrap().rules[&target].representation.clone();
            let revision = owned(&g, target, Role::Revision);
            let source = rule(&mut g, 0, true, true);
            let mut lhs = Vec::new();
            let mut assignments = BTreeMap::new();
            for key in &representation {
                let sort = g.atom(*key).unwrap().kind.sort();
                let pattern = variable(&mut g, sort);
                lhs.push(pattern);
                assignments.insert(pattern, *key);
            }
            sides(&mut g, source, &lhs, &[]);
            let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
            let step = run
                .prepare(OperationalBinding::new(source, assignments))
                .unwrap();
            let event = run.commit(step).unwrap();
            prop_assert_eq!(event.exited, BTreeSet::from([target]));
            prop_assert_eq!(&event.consumed, &representation);
            prop_assert!(event.consumed.contains(&revision));
            prop_assert!(
                event.produced.is_empty() && event.entered.is_empty() && event.altered.is_empty()
            );
            prop_assert!(run.view().unwrap().rule(target).is_none());
            Ok(())
        },
    );
}
