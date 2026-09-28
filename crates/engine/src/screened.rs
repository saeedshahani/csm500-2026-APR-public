//! Versioned v3 recurrence: immutable snapshots, necessary-invariant buckets,
//! and exact comparison on every bucket hit. Hash equality proves NOTHING.
use crate::{
    observation::{self, Budget, Observation, ObservationLimits, Observer},
    runner::Recurrence,
    Execution,
};
use graph_core::{Diagnostic, LimitKind, Result, SharedCarrier};
use std::{collections::BTreeMap, rc::Rc};
type Signature = (usize, usize, u64);
type RuleShape = Vec<(usize, bool, bool, usize, usize)>;
#[derive(Clone, Debug)]
struct Entry {
    step: u64,
    full: SharedCarrier,
    selected: Option<Rc<Observation>>,
    scheduler: String,
    rule_shape: Option<RuleShape>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::{Graph, Kind, NewAtom, Role, Target};
    #[test]
    fn rule_invariant_screens_rank_changes_but_not_projected_recurrence() {
        let source = include_str!("../../../examples/practical/month-normalisation.graph").replace(
            "rank rank0 = zero",
            "atom one = Nat(zero); rank rank0 = zero",
        );
        let a = Execution::new(
            parser::load(&source).unwrap().into_graph(),
            Default::default(),
        )
        .unwrap();
        let b = Execution::new(
            parser::load(&source.replace("rank rank0 = zero", "rank rank0 = one"))
                .unwrap()
                .into_graph(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(a.graph().shape_signature(), b.graph().shape_signature());
        let limits = ObservationLimits {
            max_comparison_work: 20,
            ..Default::default()
        };
        let mut h = History::default();
        h.observe(&a, &Observer::all(), "a".into(), limits, 0, true)
            .unwrap();
        assert!(h
            .observe(&b, &Observer::all(), "b".into(), limits, 0, true)
            .unwrap()
            .1
            .is_none());
        let mut old = History::default();
        old.observe(&a, &Observer::all(), "a".into(), limits, 0, false)
            .unwrap();
        assert!(old
            .observe(&b, &Observer::all(), "b".into(), limits, 0, false)
            .is_err());
        let observer = Observer::parse(
            "observer-v1\nselect data\nkind Constructor\n",
            |_| None,
            100,
        )
        .unwrap();
        let mut projected = History::default();
        projected
            .observe(&a, &observer, "a".into(), Default::default(), 0, true)
            .unwrap();
        let recurrence = projected
            .observe(&b, &observer, "b".into(), Default::default(), 0, true)
            .unwrap()
            .1
            .unwrap();
        assert!(!recurrence.complete_state_equal);
    }
    fn ring(targets: &[usize], offset: usize) -> Execution {
        let mut g = Graph::new();
        let old: std::collections::BTreeSet<_> = (0..offset)
            .map(|_| g.insert(Kind::Nat, &[]).unwrap())
            .collect();
        g.remove_set(&old).unwrap();
        let mut atoms: Vec<_> = targets
            .iter()
            .map(|_| NewAtom {
                kind: Kind::Constructor("v".into()),
                fields: vec![],
            })
            .collect();
        atoms.extend(targets.iter().enumerate().map(|(i, j)| NewAtom {
            kind: Kind::Record {
                role: Role::ObjectLink,
                label: Some("edge".into()),
            },
            fields: vec![Target::Fresh(i), Target::Fresh(*j)],
        }));
        g.insert_batch(&atoms).unwrap();
        Execution::new(g, Default::default()).unwrap()
    }
    #[test]
    fn matching_signatures_still_require_exact_incidence_and_allow_renamed_recurrence() {
        let a = ring(&[1, 2, 3, 4, 5, 0], 0);
        let b = ring(&[1, 2, 0, 4, 5, 3], 0);
        let c = ring(&[1, 2, 3, 4, 5, 0], 9);
        assert_eq!(a.graph().shape_signature(), b.graph().shape_signature());
        assert_eq!(a.graph().shape_signature(), c.graph().shape_signature());
        let mut h = History::default();
        assert!(h
            .observe(
                &a,
                &Observer::all(),
                "first".into(),
                Default::default(),
                0,
                true
            )
            .unwrap()
            .1
            .is_none());
        assert!(h
            .observe(
                &b,
                &Observer::all(),
                "different".into(),
                Default::default(),
                0,
                true
            )
            .unwrap()
            .1
            .is_none());
        let recurrence = h
            .observe(
                &c,
                &Observer::all(),
                "renamed".into(),
                Default::default(),
                0,
                true,
            )
            .unwrap()
            .1
            .unwrap();
        assert!(recurrence.complete_state_equal);
        assert_eq!(recurrence.first_scheduler, "first");
    }
}
impl Entry {
    fn signature(&self) -> Signature {
        self.selected
            .as_ref()
            .map_or_else(|| self.full.signature(), |o| o.signature())
    }
    fn observation(&self, execution: &Execution, limits: ObservationLimits) -> Result<Observation> {
        match &self.selected {
            Some(o) => Ok((**o).clone()),
            None => observation::all(
                &self.full.restore(&execution.limits().graph)?,
                execution.limits().graph,
                limits,
            ),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub(crate) struct History {
    entries: Vec<Entry>,
    buckets: BTreeMap<Signature, Vec<usize>>,
    units: usize,
}
impl History {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn units(&self) -> usize {
        self.units
    }
    pub fn identity(&self) -> Vec<(u64, Signature, &str)> {
        self.entries
            .iter()
            .map(|e| (e.step, e.signature(), e.scheduler.as_str()))
            .collect()
    }
    pub fn truncate(&mut self, len: usize, units: usize) {
        while self.entries.len() > len {
            let e = self.entries.pop().unwrap();
            let key = e.signature();
            let ids = self.buckets.get_mut(&key).unwrap();
            ids.pop();
            if ids.is_empty() {
                self.buckets.remove(&key);
            }
        }
        self.units = units;
    }
    pub fn observe(
        &mut self,
        execution: &Execution,
        observer: &Observer,
        scheduler: String,
        limits: ObservationLimits,
        produced_units: usize,
        rule_screen: bool,
    ) -> Result<(usize, Option<Recurrence>)> {
        let mut projection = Budget::new(limits.max_work, LimitKind::ObservationWork);
        projection.charge(1)?;
        // A full-carrier isomorphism preserves the multiset of decoded rule
        // ranks, modes, activity and membership cardinalities. This is only a
        // necessary invariant: equal shapes still require exact comparison.
        // Never use hidden rule metadata to reject a projected recurrence.
        let rule_shape =
            if rule_screen && observer.is_all() {
                let rules = &execution.configuration().rules;
                projection.charge(rules.len().saturating_mul(
                    6 + usize::BITS as usize - rules.len().leading_zeros() as usize,
                ))?;
                let mut shape: RuleShape = rules
                    .values()
                    .map(|r| (r.rank, r.meta, r.active, r.lhs.len(), r.rhs.len()))
                    .collect();
                shape.sort_unstable();
                Some(shape)
            } else {
                None
            };
        let selected = if observer.is_all() {
            None
        } else {
            Some(Rc::new(observer.project(&execution.view()?, limits)?))
        };
        let full = execution.graph().shared_carrier();
        let payload = selected
            .as_ref()
            .map_or_else(|| full.payload(), |o| o.value_payload());
        let current = Entry {
            step: execution.committed_steps(),
            full,
            selected,
            scheduler,
            rule_shape,
        };
        let signature = current.signature();
        let mut budget = Budget::new(limits.max_comparison_work, LimitKind::ComparisonWork);
        budget.charge(1)?;
        for &index in self.buckets.get(&signature).into_iter().flatten() {
            let old = &self.entries[index];
            budget.charge(old.rule_shape.as_ref().map_or(0, |s| 1 + 5 * s.len()))?;
            if old.rule_shape != current.rule_shape {
                continue;
            }
            // Charge materialisation before restoring a full historical carrier.
            budget.charge(old.full.units().saturating_add(current.full.units()))?;
            if observation::isomorphic_indexed_budget(
                &old.observation(execution, limits)?,
                &current.observation(execution, limits)?,
                &mut budget,
            )? {
                let full_equal = if old.full.signature() != current.full.signature() {
                    false
                } else if observer.is_all() {
                    true
                } else {
                    budget.charge(old.full.units().saturating_add(current.full.units()))?;
                    let a = observation::all(
                        &old.full.restore(&execution.limits().graph)?,
                        execution.limits().graph,
                        limits,
                    )?;
                    let b = observation::all(execution.graph(), execution.limits().graph, limits)?;
                    observation::isomorphic_indexed_budget(&a, &b, &mut budget)?
                };
                return Ok((
                    payload,
                    Some(Recurrence {
                        first: old.step,
                        repeated: current.step,
                        observer: observer.identity(),
                        first_scheduler: old.scheduler.clone(),
                        repeated_scheduler: current.scheduler,
                        complete_state_equal: full_equal,
                    }),
                ));
            }
        }
        // Versioned logical units: first full payload plus fresh immutable atoms,
        // projected copies and per-entry metadata. Tree overhead is not heap bytes.
        let extra = if self.entries.is_empty() {
            current.full.units()
        } else {
            produced_units
        };
        let units = self
            .units
            .saturating_add(extra)
            .saturating_add(current.scheduler.len() + 8)
            .saturating_add(current.rule_shape.as_ref().map_or(0, |s| 5 * s.len()))
            .saturating_add(current.selected.as_ref().map_or(0, |o| o.retention_units()));
        if units > limits.max_history_units {
            return Err(Diagnostic::exhausted(
                LimitKind::ObservationHistory,
                "retained observation history limit exhausted",
            ));
        }
        self.buckets
            .entry(signature)
            .or_default()
            .push(self.entries.len());
        self.entries.push(current);
        self.units = units;
        Ok((payload, None))
    }
}
