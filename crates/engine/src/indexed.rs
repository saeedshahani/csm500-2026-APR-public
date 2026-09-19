//! Experimental query-local indexed matching and compact enabled-set retention.
//! Not used by Runner or portable trace replay: work accounting is a separate
//! profile. Admission still uses the complete reference validator/transaction.
use crate::scheduling::Scheduler;
use crate::{AdmissionFailure, Execution, OperationalBinding, PreparedStep, RejectedBinding};
use graph_core::{AtomKey, Binding, Graph, Kind, Result, Role, RuleView, Sort};
use std::collections::{BTreeMap, BTreeSet};

pub const PROFILE: &str = "indexed-compact-v1";
type KindKey = (Sort, Option<Role>, Option<String>, usize);
#[derive(Default)]
struct Index {
    sorts: BTreeMap<Sort, Vec<AtomKey>>,
    kinds: BTreeMap<KindKey, Vec<AtomKey>>,
    incoming: BTreeMap<(AtomKey, usize), Vec<AtomKey>>,
}
fn kind_key(kind: &Kind, arity: usize) -> KindKey {
    let label = match kind {
        Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => Some(s.clone()),
        _ => None,
    };
    (kind.sort(), kind.role(), label, arity)
}
impl Index {
    fn build(graph: &Graph, search: &mut crate::CandidateSearch<'_>) -> Result<Self> {
        let mut index = Self::default();
        for (id, atom) in graph.iter() {
            search.charge_work(1 + atom.fields.len())?;
            index.sorts.entry(atom.kind.sort()).or_default().push(id);
            index
                .kinds
                .entry(kind_key(&atom.kind, atom.fields.len()))
                .or_default()
                .push(id);
            for (slot, target) in atom.fields.iter().enumerate() {
                index.incoming.entry((*target, slot)).or_default().push(id);
            }
        }
        Ok(index)
    }
}

/// A complete admission result borrowing its immutable baseline. No graph copies
/// survive enumeration. The borrow disallows mutation/reload while it exists.
#[derive(Debug)]
pub struct CompactEnabled<'a> {
    execution: &'a Execution,
    bindings: Vec<OperationalBinding>,
    rejected: Vec<RejectedBinding>,
    work: usize,
}
impl CompactEnabled<'_> {
    pub fn bindings(&self) -> &[OperationalBinding] {
        &self.bindings
    }
    pub fn rejected(&self) -> &[RejectedBinding] {
        &self.rejected
    }
    pub fn work_used(&self) -> usize {
        self.work
    }
    /// Reprepare the selected binding on the unchanged baseline. The caller
    /// publishes the returned scheduler only after a successful checked commit.
    /// Custom schedulers inspecting full candidates are intentionally unsupported.
    pub fn select(self, scheduler: &Scheduler) -> Result<Option<(PreparedStep, Scheduler)>> {
        let mut next = scheduler.clone();
        match next.select_count(self.bindings.len()) {
            None => Ok(None),
            Some(i) => Ok(Some((
                self.execution.prepare(self.bindings[i].clone())?,
                next,
            ))),
        }
    }
}

impl Execution {
    /// Complete raw matching under the indexed-v1 work accounting profile.
    pub fn bindings_indexed(&self, source: AtomKey) -> Result<Vec<OperationalBinding>> {
        let mut search = self.search();
        let view = self.view()?;
        let rule = crate::matching::source_rule(view.configuration(), source)?;
        let hidden = crate::matching::hidden(view.configuration(), &mut search)?;
        let index = Index::build(self.graph(), &mut search)?;
        let mut out = Vec::new();
        enumerate(self.graph(), rule, &hidden, &index, &mut search, |s, b| {
            s.reserve_candidate(b.assignments().len())?;
            out.push(b);
            Ok(())
        })?;
        Ok(out)
    }
    /// Full enabled set, not first-match selection. Each candidate is fully
    /// prepared then discarded; only its binding and rejection evidence remain.
    pub fn enabled_compact(&self) -> Result<CompactEnabled<'_>> {
        let mut search = self.search();
        let view = self.view()?;
        let hidden = crate::matching::hidden(view.configuration(), &mut search)?;
        let index = Index::build(self.graph(), &mut search)?;
        let mut bindings = Vec::new();
        let mut rejected = Vec::new();
        for rule in view.live_rules() {
            search.charge_work(1)?;
            enumerate(self.graph(), rule, &hidden, &index, &mut search, |s, b| {
                match s.prepare_classified(b.clone()) {
                    Ok(step) => {
                        s.charge_work(step.ground_rule().fresh().len())?;
                        bindings.push(b);
                    }
                    Err(AdmissionFailure::Rejected(diagnostic)) => rejected.push(RejectedBinding {
                        binding: b,
                        diagnostic,
                    }),
                    Err(AdmissionFailure::Failed(error)) => return Err(error),
                }
                Ok(())
            })?;
        }
        bindings.sort_by(|a, b| (a.source(), a.assignments()).cmp(&(b.source(), b.assignments())));
        Ok(CompactEnabled {
            execution: self,
            bindings,
            rejected,
            work: search.work_used(),
        })
    }
}

fn enumerate(
    graph: &Graph,
    rule: &RuleView,
    hidden: &BTreeSet<AtomKey>,
    index: &Index,
    search: &mut crate::CandidateSearch<'_>,
    mut visit: impl FnMut(&mut crate::CandidateSearch<'_>, OperationalBinding) -> Result<()>,
) -> Result<()> {
    if !rule.active {
        return Ok(());
    }
    let mut assignment = BTreeMap::new();
    let mut used = BTreeSet::new();
    let mut domains = Vec::new();
    for id in &rule.lhs {
        search.charge_work(1)?;
        let pattern = graph.atom(*id)?;
        if matches!(
            pattern.kind,
            Kind::PatternVariable {
                binding: Binding::AppliedRule,
                ..
            }
        ) {
            if !used.insert(rule.head) {
                return Ok(());
            }
            assignment.insert(*id, rule.head);
            continue;
        }
        let candidates: Vec<_> = match &pattern.kind {
            Kind::PatternVariable { admits, .. } => admits
                .iter()
                .flat_map(|s| index.sorts.get(s).into_iter().flatten())
                .copied()
                .collect(),
            kind => index
                .kinds
                .get(&kind_key(kind, pattern.fields.len()))
                .cloned()
                .unwrap_or_default(),
        };
        search.charge_work(candidates.len())?;
        let mut candidates: Vec<_> = candidates
            .into_iter()
            .filter(|id| rule.meta || !hidden.contains(id))
            .collect();
        candidates.sort();
        if candidates.is_empty() {
            return Ok(());
        }
        domains.push((*id, candidates));
    }
    domains.sort_by_key(|(id, candidates)| (candidates.len(), *id));
    // Iterative DFS; each frame's restricted domain is rebuilt when its parent
    // changes. Incidence is a necessary condition, never an admission shortcut.
    let mut frames: Vec<(Vec<AtomKey>, usize)> = Vec::new();
    let mut depth = 0;
    loop {
        if depth == domains.len() {
            if crate::matching::feasible(graph, &assignment, search)? {
                visit(
                    search,
                    OperationalBinding::new(rule.head, assignment.clone()),
                )?;
            }
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
            continue;
        }
        let (pattern_id, base) = &domains[depth];
        if let Some(previous) = assignment.remove(pattern_id) {
            used.remove(&previous);
        }
        if frames.len() == depth {
            let mut restricted: Option<Vec<AtomKey>> = None;
            let pattern = graph.atom(*pattern_id)?;
            for (slot, field) in pattern.fields.iter().enumerate() {
                if let Some(target) = assignment.get(field) {
                    let values = index
                        .incoming
                        .get(&(*target, slot))
                        .cloned()
                        .unwrap_or_default();
                    search.charge_work(values.len() + 1)?;
                    if restricted
                        .as_ref()
                        .is_none_or(|old| values.len() < old.len())
                    {
                        restricted = Some(values);
                    }
                }
            }
            for (other, target) in &assignment {
                for (slot, field) in graph.atom(*other)?.fields.iter().enumerate() {
                    search.charge_work(1)?;
                    if field == pattern_id {
                        restricted = Some(vec![graph.atom(*target)?.fields[slot]]);
                    }
                }
            }
            let values = match restricted {
                Some(values) => values
                    .into_iter()
                    .filter(|id| base.binary_search(id).is_ok())
                    .collect(),
                None => {
                    search.charge_work(base.len())?;
                    base.clone()
                }
            };
            frames.push((values, 0));
        }
        let (values, next) = &mut frames[depth];
        let mut advanced = false;
        while *next < values.len() {
            search.charge_work(1)?;
            let target = values[*next];
            *next += 1;
            if !used.insert(target) {
                continue;
            }
            assignment.insert(*pattern_id, target);
            if crate::matching::feasible(graph, &assignment, search)? {
                depth += 1;
                advanced = true;
                break;
            }
            assignment.remove(pattern_id);
            used.remove(&target);
        }
        if !advanced {
            frames.pop();
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
        }
    }
}
