//! Maintained complete set for a checked, label-parametric one-shot rule class.
//! Failure to recognise the class uses generic complete admission, never rejection.
use crate::{Execution, OperationalBinding};
use graph_core::{AtomKey, Binding, Kind, Result, Role, Sort};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[derive(Clone, Debug)]
struct Counts {
    count: usize,
    left: Option<Rc<Counts>>,
    right: Option<Rc<Counts>>,
}
impl Counts {
    fn new(n: usize) -> Rc<Self> {
        Rc::new(Self {
            count: n,
            left: (n > 1).then(|| Self::new(n / 2)),
            right: (n > 1).then(|| Self::new(n - n / 2)),
        })
    }
    fn at(&self, rank: usize, lo: usize, hi: usize) -> usize {
        assert!(rank < self.count);
        if hi - lo == 1 {
            return lo;
        }
        let left = self.left.as_ref().unwrap();
        let mid = lo + (hi - lo) / 2;
        if rank < left.count {
            left.at(rank, lo, mid)
        } else {
            self.right.as_ref().unwrap().at(rank - left.count, mid, hi)
        }
    }
    fn remove(&self, index: usize, lo: usize, hi: usize) -> Rc<Self> {
        if hi - lo == 1 {
            assert_eq!(self.count, 1);
            return Counts::new(0);
        }
        let mid = lo + (hi - lo) / 2;
        Rc::new(Self {
            count: self.count - 1,
            left: Some(if index < mid {
                self.left.as_ref().unwrap().remove(index, lo, mid)
            } else {
                self.left.as_ref().unwrap().clone()
            }),
            right: Some(if index >= mid {
                self.right.as_ref().unwrap().remove(index, mid, hi)
            } else {
                self.right.as_ref().unwrap().clone()
            }),
        })
    }
}
#[derive(Clone, Debug)]
pub(crate) struct OneShot {
    pub head: AtomKey,
    bindings: Rc<Vec<OperationalBinding>>,
    counts: Rc<Counts>,
}
impl OneShot {
    pub fn recognise(execution: &Execution) -> Result<Option<Self>> {
        let cfg = execution.configuration();
        let active: Vec<_> = cfg.rules.values().filter(|r| r.active).collect();
        if active.len() != 1 {
            return Ok(None);
        }
        let r = active[0];
        let g = execution.graph();
        if r.meta || r.lhs.len() != 2 || r.rhs.len() != 2 {
            return Ok(None);
        }
        let kept: Vec<_> = r.lhs.intersection(&r.rhs).copied().collect();
        let removed: Vec<_> = r.lhs.difference(&r.rhs).copied().collect();
        let added: Vec<_> = r.rhs.difference(&r.lhs).copied().collect();
        if kept.len() != 1 || removed.len() != 1 || added.len() != 1 {
            return Ok(None);
        }
        let variable = kept[0];
        let old = g.atom(removed[0])?;
        let new = g.atom(added[0])?;
        if g.atom(variable)?.kind != Kind::variable(Sort::Constructor, Binding::Free)
            || old.fields != [variable, variable]
            || new.fields != old.fields
            || !matches!(
                old.kind,
                Kind::Record {
                    role: Role::ObjectLink,
                    ..
                }
            )
            || !matches!(
                new.kind,
                Kind::Record {
                    role: Role::ObjectLink,
                    ..
                }
            )
            || old.kind == new.kind
        {
            return Ok(None);
        }
        let hidden: BTreeSet<_> = cfg
            .rules
            .values()
            .flat_map(|r| r.representation.iter().copied())
            .collect();
        let mut search = execution.search();
        let mut bindings = Vec::new();
        for (key, atom) in g.iter() {
            search.charge_work(1 + atom.fields.len())?;
            if atom.kind != old.kind
                || hidden.contains(&key)
                || atom.fields[0] != atom.fields[1]
                || hidden.contains(&atom.fields[0])
            {
                continue;
            }
            let binding = OperationalBinding::new(
                r.head,
                BTreeMap::from([(variable, atom.fields[0]), (removed[0], key)]),
            );
            // Establish admission for EVERY initial occurrence, not a sampled one.
            // At later queries resource admission is shared because each delta has
            // the same counts/kinds, and untouched occurrences cannot interfere.
            match search.prepare(binding.clone()) {
                Ok(_) => bindings.push(binding),
                Err(e) if e.kind() == graph_core::ErrorKind::ResourceExhaustion => return Err(e),
                Err(_) => return Ok(None),
            }
        }
        bindings.sort_by(|a, b| a.assignments().cmp(b.assignments()));
        search.charge_work(4usize.saturating_add(bindings.len()))?;
        Ok(Some(Self {
            head: r.head,
            counts: Counts::new(bindings.len()),
            bindings: Rc::new(bindings),
        }))
    }
    pub fn len(&self) -> usize {
        self.counts.count
    }
    pub fn select(&self, rank: usize) -> (usize, OperationalBinding) {
        let index = self.counts.at(rank, 0, self.bindings.len());
        (index, self.bindings[index].clone())
    }
    pub fn remove(&mut self, index: usize) {
        self.counts = self.counts.remove(index, 0, self.bindings.len());
    }
    pub fn check_query(&self, execution: &Execution) -> Result<()> {
        let mut search = execution.search();
        // Versioned v3 charge: certified set cardinality plus selected preparation.
        search.charge_work(1 + self.len())?;
        if self.len() > execution.limits().search.max_candidates {
            return Err(graph_core::Diagnostic::exhausted(
                graph_core::LimitKind::Candidates,
                "certified complete set exceeds candidate bound",
            ));
        }
        if self.len() != 0 {
            search.prepare(self.select(0).1)?;
        }
        Ok(())
    }
}
