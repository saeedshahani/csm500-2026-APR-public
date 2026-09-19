#[allow(dead_code)]
mod support;
use engine::causal::{AnalysisLimits, DependencyOrder};
use engine::conditions;
use engine::observation::Observer;
use engine::recording::{AtomId, EventId, Recording};
use engine::trace::{TraceDocument, VerifiedTrace};
use engine::{Execution, OperationalBinding};
use graph_core::{ErrorKind, Graph, Role, Sort};
use std::collections::BTreeMap;
use support::*;

fn verified(log: &Recording) -> VerifiedTrace {
    VerifiedTrace::verify(
        TraceDocument::from_recording(
            log,
            &Observer::all(),
            None,
            Default::default(),
            Default::default(),
        )
        .unwrap(),
        Default::default(),
    )
    .unwrap()
}

#[test]
fn exhaustive_two_side_oracle_and_both_removal_orders() {
    // Independent truth table: visibility = !lhs && !rhs. All four initial
    // valuations, both orders, and each one-blocker-restored intervention.
    for mask in 0..4u8 {
        for reverse in [false, true] {
            let mut g = Graph::new();
            let hidden = ctor(&mut g, "Hidden");
            let owner = rule(&mut g, 0, false, false);
            let one = [hidden];
            sides(
                &mut g,
                owner,
                if mask & 1 != 0 { &one } else { &[] },
                if mask & 2 != 0 { &one } else { &[] },
            );
            let mut removers = Vec::new();
            for (bit, role) in [(1, Role::Lhs), (2, Role::Rhs)] {
                if mask & bit == 0 {
                    continue;
                }
                let source = rule(&mut g, 1, true, true);
                let h = variable(&mut g, Sort::RuleHead);
                let c = ctor(&mut g, "Hidden");
                let m = record(&mut g, role, &[h, c]);
                let rev = record(&mut g, Role::Revision, &[h]);
                let next = record(&mut g, Role::Revision, &[h]);
                sides(&mut g, source, &[h, c, m, rev], &[h, c, next]);
                removers.push((bit, role, source, h, c, m, rev));
            }
            if reverse {
                removers.reverse();
            }
            let reader = rule(&mut g, 0, false, true);
            let p = ctor(&mut g, "Hidden");
            let seen = ctor(&mut g, "Seen");
            sides(&mut g, reader, &[p], &[p, seen]);
            let mut e = Execution::new(g, Default::default()).unwrap();
            let mut log = Recording::new(&e, Default::default()).unwrap();
            let mut remaining = mask;
            let mut expected = Vec::new();
            for (i, (bit, role, source, h, c, m, rev)) in removers.into_iter().enumerate() {
                assert_eq!(
                    e.enabled_for(reader).unwrap().steps().len(),
                    usize::from(remaining == 0)
                );
                let membership = owned(e.graph(), owner, role);
                let binding = OperationalBinding::new(
                    source,
                    BTreeMap::from([
                        (h, owner),
                        (c, hidden),
                        (m, membership),
                        (rev, owned(e.graph(), owner, Role::Revision)),
                    ]),
                );
                let step = e.prepare(binding).unwrap();
                log.commit(&mut e, step).unwrap();
                remaining &= !bit;
                assert_eq!(
                    e.enabled_for(reader).unwrap().steps().len(),
                    usize::from(remaining == 0)
                );
                expected.push((AtomId::from(membership), EventId(i as u64)));
            }
            for (i, _) in &expected {
                // Restoring each removed membership falsifies the absence predicate.
                let original = log.atom(*i).unwrap();
                assert_eq!(original.fields[1], AtomId::from(hidden));
                let mut counterfactual = e.graph().clone();
                counterfactual
                    .insert(original.kind.clone(), &[owner, hidden])
                    .unwrap();
                assert!(e.bindings_indexed(reader).unwrap().len() == 1);
                let blocked = Execution::new(counterfactual, Default::default()).unwrap();
                assert!(blocked.bindings_indexed(reader).unwrap().is_empty());
            }
            let step = e.enabled_for(reader).unwrap().into_steps().pop().unwrap();
            log.commit(&mut e, step).unwrap();
            let trace = verified(&log);
            let report = conditions::extract(&trace, Default::default()).unwrap();
            let consumer = EventId(expected.len() as u64);
            assert_eq!(report.witnesses.len(), expected.len());
            for (w, (membership, removed_by)) in report.witnesses.iter().zip(expected) {
                assert_eq!(
                    (w.membership, w.removed_by, w.used_by, w.atom),
                    (membership, removed_by, consumer, hidden.into())
                );
            }
            let positive = DependencyOrder::extract(log.events(), Default::default()).unwrap();
            assert!(!positive.edges().keys().any(|(_, to)| *to == consumer));
            assert_eq!(
                conditions::extract(
                    &trace,
                    AnalysisLimits {
                        max_work: 0,
                        max_units: 100000
                    }
                )
                .unwrap_err()
                .kind(),
                ErrorKind::ResourceExhaustion
            );
            assert_eq!(
                conditions::extract(
                    &trace,
                    AnalysisLimits {
                        max_work: 100000,
                        max_units: 0
                    }
                )
                .unwrap_err()
                .kind(),
                ErrorKind::ResourceExhaustion
            );
        }
    }
}

#[test]
fn newly_created_constructor_quotation_is_explicitly_outside_fragment() {
    let mut g = Graph::new();
    let hidden = ctor(&mut g, "Hidden");
    let owner = rule(&mut g, 0, false, false);
    let source = rule(&mut g, 1, true, true);
    let h = variable(&mut g, Sort::RuleHead);
    let c = ctor(&mut g, "Hidden");
    let m = record(&mut g, Role::Rhs, &[h, c]);
    let rev = record(&mut g, Role::Revision, &[h]);
    let next = record(&mut g, Role::Revision, &[h]);
    sides(&mut g, source, &[h, c, rev], &[h, c, next, m]);
    let mut e = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&e, Default::default()).unwrap();
    let binding = OperationalBinding::new(
        source,
        BTreeMap::from([
            (h, owner),
            (c, hidden),
            (rev, owned(e.graph(), owner, Role::Revision)),
        ]),
    );
    let step = e.prepare(binding).unwrap();
    log.commit(&mut e, step).unwrap();
    let error = conditions::extract(&verified(&log), Default::default()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidConfiguration);
    assert!(error.to_string().contains("newly operative"));
}
