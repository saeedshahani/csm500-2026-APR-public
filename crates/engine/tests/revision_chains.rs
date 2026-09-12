//! Correct-by-construction closed histories; no external edits occur after Execution::new.
use engine::{Execution, ExecutionLimits};
use graph_core::{fixture, validate, AtomKey, Graph, Kind, NewAtom, Role, Target};
use std::collections::{BTreeMap, BTreeSet};
#[allow(dead_code)]
mod support;

fn lowerer(g: &mut Graph, rank: usize) -> AtomKey {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let h = f.names["sl"];
    let mut top = f.names["sl_r0"];
    f.graph
        .remove_set(&BTreeSet::from([f.names["sl_rank"], f.names["sl_r1"]]))
        .unwrap();
    for _ in 0..rank {
        top = f.graph.insert(Kind::Nat, &[top]).unwrap();
    }
    f.graph.insert(Kind::record(Role::Rank), &[h, top]).unwrap();
    let atoms: Vec<_> = f.graph.iter().collect();
    let positions: BTreeMap<_, _> = atoms
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();
    let batch: Vec<_> = atoms
        .iter()
        .map(|(_, a)| NewAtom {
            kind: a.kind.clone(),
            fields: a
                .fields
                .iter()
                .map(|id| Target::Fresh(positions[id]))
                .collect(),
        })
        .collect();
    g.insert_batch(&batch).unwrap()[positions[&h]]
}

#[test]
fn generated_closed_multihead_histories_respect_initial_rank_and_revision_depth() {
    for seed in 0..24usize {
        let mut g = Graph::new();
        let ranks = [seed % 5, (seed / 2 + 1) % 5, (seed / 3 + 2) % 5];
        let heads: Vec<_> = ranks.iter().map(|r| lowerer(&mut g, *r)).collect();
        let initial = validate(&g).unwrap();
        let maximum = *ranks.iter().max().unwrap();
        let mut provenance: BTreeMap<_, _> = initial
            .rules
            .values()
            .map(|r| (r.revision, (0usize, r.rank)))
            .collect();
        let mut all_seen: BTreeSet<_> = g.iter().map(|(id, _)| id).collect();
        let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
        for i in 0..ranks.iter().sum::<usize>() {
            let before = validate(run.graph()).unwrap();
            let mut steps = run.enabled().unwrap().into_steps();
            let index = (seed + i * 7) % steps.len();
            let event = run.commit(steps.remove(index)).unwrap();
            let after = validate(run.graph()).unwrap();
            let source_rank = before.rules[&event.source].rank;
            let (depth, birth_rank) = provenance[&event.source_revision];
            assert_eq!(source_rank, birth_rank, "revision continuity");
            assert!(event.produced.is_disjoint(&all_seen));
            all_seen.extend(&event.produced);
            assert_eq!(event.altered, BTreeSet::from([event.source]));
            for h in event.entered.union(&event.altered) {
                let post = &after.rules[h];
                assert!(post.rank < source_rank && post.rank <= maximum);
                assert!(depth < maximum);
                assert!(provenance
                    .insert(post.revision, (depth + 1, post.rank))
                    .is_none());
                assert!(event.produced.contains(&post.revision));
                assert!(event.consumed.contains(&before.rules[h].revision));
            }
            for h in &heads {
                if *h != event.source {
                    assert_eq!(before.rules[h].revision, after.rules[h].revision);
                }
            }
        }
        assert!(run.enabled().unwrap().steps().is_empty());
        assert!(run.view().unwrap().live_rules().all(|r| r.rank == 0));
        assert_eq!(run.committed_steps() as usize, ranks.iter().sum::<usize>());
    }
}

fn installer(source_rank: usize, target_rank: usize, active: bool) -> (Graph, AtomKey) {
    let mut g = Graph::new();
    let source = support::rule(&mut g, source_rank, true, true);
    let old: BTreeSet<_> = g.iter().map(|(id, _)| id).collect();
    support::rule(&mut g, target_rank, false, active);
    let package: Vec<_> = g
        .iter()
        .map(|(id, _)| id)
        .filter(|id| !old.contains(id))
        .collect();
    support::sides(&mut g, source, &[], &package);
    (g, source)
}

#[test]
fn generated_active_and_inactive_packages_check_every_entry_against_source_rank() {
    for source_rank in 0..4 {
        for target_rank in 0..4 {
            for active in [false, true] {
                let (g, source) = installer(source_rank, target_rank, active);
                let before = g.snapshot();
                let next = g.clone().insert(Kind::Nat, &[]).unwrap();
                let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
                let enabled = run.enabled_for(source).unwrap();
                if target_rank < source_rank {
                    assert_eq!(enabled.steps().len(), 1);
                    let event = run.commit(enabled.into_steps().pop().unwrap()).unwrap();
                    assert_eq!(event.entered.len(), 1);
                    let post = run.view().unwrap();
                    let entered = post.rule(*event.entered.first().unwrap()).unwrap();
                    assert_eq!((entered.rank, entered.active), (target_rank, active));
                    assert!(event.produced.contains(&entered.revision));
                } else {
                    assert!(enabled.steps().is_empty());
                    assert_eq!(enabled.rejected().len(), 1);
                    assert_eq!(
                        enabled.rejected()[0].diagnostic.kind(),
                        graph_core::ErrorKind::Guardedness
                    );
                    assert_eq!(run.graph().snapshot(), before);
                    assert_eq!(run.graph().clone().insert(Kind::Nat, &[]).unwrap(), next);
                }
            }
        }
    }
}

#[test]
fn a_fixed_installer_can_create_unboundedly_many_shallow_versions_in_the_mathematical_model() {
    let (g, source) = installer(1, 0, false);
    let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
    let revision = run.view().unwrap().rule(source).unwrap().revision;
    for expected in 2..=17 {
        let step = run.enabled_for(source).unwrap().into_steps().pop().unwrap();
        let event = run.commit(step).unwrap();
        assert_eq!(event.source_revision, revision);
        assert_eq!(event.entered.len(), 1);
        assert_eq!(run.view().unwrap().live_rules().count(), expected);
        assert!(run
            .view()
            .unwrap()
            .live_rules()
            .filter(|r| r.head != source)
            .all(|r| r.rank == 0));
    }
    // Sixteen instances test only a finite prefix; the induction is in formalism §13.
}

#[test]
fn an_alternating_weak_strict_measure_model_terminates_but_restoration_breaks_it() {
    // Abstract certificate comparison only, not an alternative production guard.
    for initial in 0..17 {
        let (mut n, mut phase, mut steps) = (initial, false, 0);
        while n > 0 {
            if phase {
                n -= 1;
            }
            phase = !phase;
            steps += 1;
        }
        assert_eq!(steps, 2 * initial);
    }
    // An omitted restoring edge invalidates the claimed global descent.
    let mut n = 1;
    for _ in 0..16 {
        n -= 1;
        assert_eq!(n, 0);
        n += 1;
    }
    assert_eq!(n, 1);
}
