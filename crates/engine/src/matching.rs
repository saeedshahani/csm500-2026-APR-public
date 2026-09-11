use crate::{AdmissionFailure, CandidateSearch, Execution, OperationalBinding, PreparedStep};
use graph_core::{
    AtomKey, Binding, Configuration, Diagnostic, ErrorKind, Graph, Kind, Result, RuleView, Sort,
};
use std::collections::{BTreeMap, BTreeSet};

/// A raw match rejected by a named applicability predicate, not a search failure.
#[derive(Debug)]
pub struct RejectedBinding {
    pub binding: OperationalBinding,
    pub diagnostic: Diagnostic,
}

/// Fully examined enabled candidates and expected rejections for one immutable
/// state. On search/resource failure no partial set is returned. Each candidate
/// is independently stamped; committing one invalidates the others.
#[derive(Debug)]
pub struct EnabledSet {
    steps: Vec<PreparedStep>,
    rejected: Vec<RejectedBinding>,
    candidates: usize,
    work: usize,
}
impl EnabledSet {
    pub fn steps(&self) -> &[PreparedStep] {
        &self.steps
    }
    pub fn rejected(&self) -> &[RejectedBinding] {
        &self.rejected
    }
    pub fn into_steps(self) -> Vec<PreparedStep> {
        self.steps
    }
    /// Cumulative complete candidates in the containing CandidateSearch.
    pub fn attempted_candidates(&self) -> usize {
        self.candidates
    }
    pub fn work_used(&self) -> usize {
        self.work
    }
}

impl Execution {
    /// All raw injective pattern matches for one live source. Inactive sources
    /// yield no bindings. Raw matching does not check grounding or applicability.
    pub fn bindings(&self, source: AtomKey) -> Result<Vec<OperationalBinding>> {
        self.search().bindings(source)
    }
    /// Exhaustively examine every active source before returning the enabled set.
    /// Only successful empty all-source results establish absence of an enabled step.
    pub fn enabled(&self) -> Result<EnabledSet> {
        self.search().enabled()
    }
    /// As enabled, restricted to one live source; an empty result is not a global normal form.
    pub fn enabled_for(&self, source: AtomKey) -> Result<EnabledSet> {
        self.search().enabled_for(source)
    }
}

impl CandidateSearch<'_> {
    pub fn bindings(&mut self, source: AtomKey) -> Result<Vec<OperationalBinding>> {
        let result = (|| {
            self.check()?;
            let execution = self.execution;
            let view = execution.view()?;
            let rule = source_rule(view.configuration(), source)?;
            let hidden = hidden(view.configuration(), self)?;
            let mut bindings = Vec::new();
            enumerate(execution.graph(), rule, &hidden, self, |search, binding| {
                search.reserve_candidate(binding.assignments().len())?;
                bindings.push(binding);
                Ok(())
            })?;
            Ok(bindings)
        })();
        self.remember_failure(result)
    }
    pub fn enabled(&mut self) -> Result<EnabledSet> {
        self.enabled_query(None)
    }
    pub fn enabled_for(&mut self, source: AtomKey) -> Result<EnabledSet> {
        self.enabled_query(Some(source))
    }
    fn enabled_query(&mut self, source: Option<AtomKey>) -> Result<EnabledSet> {
        let result = (|| {
            self.check()?;
            let execution = self.execution;
            let view = execution.view()?;
            if let Some(source) = source {
                source_rule(view.configuration(), source)?;
            }
            let hidden = hidden(view.configuration(), self)?;
            let mut steps = Vec::new();
            let mut rejected = Vec::new();
            for rule in view
                .live_rules()
                .filter(|r| source.is_none_or(|s| s == r.head))
            {
                self.charge_work(1)?;
                enumerate(execution.graph(), rule, &hidden, self, |search, binding| {
                    // Retention as well as exploration is bounded by cumulative work.
                    search.charge_work(execution.graph().len())?;
                    match search.prepare_classified(binding.clone()) {
                        Ok(step) => {
                            search.charge_work(step.ground_rule().fresh().len())?;
                            steps.push(step);
                        }
                        Err(AdmissionFailure::Rejected(diagnostic)) => {
                            rejected.push(RejectedBinding {
                                binding,
                                diagnostic,
                            })
                        }
                        Err(AdmissionFailure::Failed(error)) => return Err(error),
                    }
                    Ok(())
                })?;
            }
            Ok(EnabledSet {
                steps,
                rejected,
                candidates: self.attempted_candidates(),
                work: self.work_used(),
            })
        })();
        self.remember_failure(result)
    }
}

fn source_rule(configuration: &Configuration, source: AtomKey) -> Result<&RuleView> {
    configuration.rules.get(&source).ok_or_else(|| {
        Diagnostic::new(ErrorKind::InvalidBinding, "enumeration source is not live").at_atom(source)
    })
}
fn hidden(
    configuration: &Configuration,
    search: &mut CandidateSearch<'_>,
) -> Result<BTreeSet<AtomKey>> {
    let mut hidden = BTreeSet::new();
    for rule in configuration.rules.values() {
        search.charge_work(rule.representation.len())?;
        hidden.extend(&rule.representation);
    }
    Ok(hidden)
}

/// Fixed candidate domains, reversible assignments and iterative backtracking.
/// Each complete assignment has exactly one search path; parallel keys stay distinct.
fn enumerate(
    graph: &Graph,
    rule: &RuleView,
    hidden: &BTreeSet<AtomKey>,
    search: &mut CandidateSearch<'_>,
    mut visit: impl FnMut(&mut CandidateSearch<'_>, OperationalBinding) -> Result<()>,
) -> Result<()> {
    if !rule.active {
        return Ok(());
    }
    let mut index: BTreeMap<Sort, Vec<AtomKey>> = BTreeMap::new();
    for (id, atom) in graph.iter() {
        search.charge_work(1)?;
        if rule.meta || !hidden.contains(&id) {
            index.entry(atom.kind.sort()).or_default().push(id);
        }
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
        let sorts = match &pattern.kind {
            Kind::PatternVariable { admits, .. } => admits.clone(),
            kind => BTreeSet::from([kind.sort()]),
        };
        let mut candidates = Vec::new();
        for sort in sorts {
            for target in index.get(&sort).into_iter().flatten() {
                search.charge_work(1)?;
                let atom = graph.atom(*target)?;
                if matches!(pattern.kind, Kind::PatternVariable { .. })
                    || (pattern.kind == atom.kind && pattern.fields.len() == atom.fields.len())
                {
                    candidates.push(*target);
                }
            }
        }
        candidates.sort();
        if candidates.is_empty() {
            return Ok(());
        }
        domains.push((*id, candidates));
    }
    domains.sort_by_key(|(id, candidates)| (candidates.len(), *id));
    let mut next = vec![0; domains.len()];
    let mut depth = 0;
    loop {
        if depth == domains.len() {
            visit(
                search,
                OperationalBinding::new(rule.head, assignment.clone()),
            )?;
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
            continue;
        }
        let (pattern, candidates) = &domains[depth];
        if let Some(previous) = assignment.remove(pattern) {
            used.remove(&previous);
        }
        let mut advance = false;
        while next[depth] < candidates.len() {
            search.charge_work(1)?;
            let target = candidates[next[depth]];
            next[depth] += 1;
            if !used.insert(target) {
                continue;
            }
            assignment.insert(*pattern, target);
            if feasible(graph, &assignment, search)? {
                advance = true;
                break;
            }
            assignment.remove(pattern);
            used.remove(&target);
        }
        if advance {
            depth += 1;
        } else {
            next[depth] = 0;
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
        }
    }
}
fn feasible(
    graph: &Graph,
    assignment: &BTreeMap<AtomKey, AtomKey>,
    search: &mut CandidateSearch<'_>,
) -> Result<bool> {
    for (pattern, target) in assignment {
        search.charge_work(1)?;
        let pattern = graph.atom(*pattern)?;
        let atom = graph.atom(*target)?;
        for (field, actual) in pattern.fields.iter().zip(&atom.fields) {
            search.charge_work(1)?;
            if assignment
                .get(field)
                .is_some_and(|expected| expected != actual)
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collection_insertion_order_does_not_change_search_with_fixed_atom_keys() {
        let f =
            graph_core::fixture::load(include_str!("../../../examples/fixtures/c1.atoms")).unwrap();
        let execution = Execution::new(f.graph, Default::default()).unwrap();
        let original = execution.view().unwrap().configuration().clone();
        let mut reordered = Configuration::default();
        for (head, rule) in original.rules.iter().rev() {
            let mut rule = rule.clone();
            rule.lhs = rule.lhs.iter().rev().copied().collect();
            rule.rhs = rule.rhs.iter().rev().copied().collect();
            rule.representation = rule.representation.iter().rev().copied().collect();
            reordered.rules.insert(*head, rule);
        }
        reordered.quoted = original
            .quoted
            .iter()
            .rev()
            .map(|(a, b)| (*a, *b))
            .collect();
        reordered.operative_memberships = original
            .operative_memberships
            .iter()
            .rev()
            .copied()
            .collect();
        let mut outputs = Vec::new();
        for config in [&original, &reordered] {
            let mut search = execution.search();
            let hidden = hidden(config, &mut search).unwrap();
            let mut output = Vec::new();
            for rule in config.rules.values() {
                enumerate(execution.graph(), rule, &hidden, &mut search, |s, b| {
                    s.reserve_candidate(b.assignments().len())?;
                    output.push(b);
                    Ok(())
                })
                .unwrap();
            }
            outputs.push((output, search.work_used()));
            search.finish().unwrap();
        }
        assert_eq!(outputs[0], outputs[1]);
    }

    #[test]
    fn unexpected_binding_failure_is_not_an_expected_applicability_rejection() {
        let f =
            graph_core::fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
        let execution = Execution::new(f.graph, Default::default()).unwrap();
        let mut search = execution.search();
        let result =
            search.prepare_classified(OperationalBinding::new(f.names["sl"], BTreeMap::new()));
        assert!(
            matches!(result,Err(AdmissionFailure::Failed(e)) if e.kind()==ErrorKind::InvalidBinding)
        );
        assert!(matches!(
            AdmissionFailure::rejected(
                Diagnostic::exhausted(graph_core::LimitKind::Atoms, "limit"),
                ErrorKind::InvalidConfiguration
            ),
            AdmissionFailure::Failed(_)
        ));
        assert!(matches!(
            AdmissionFailure::rejected(
                Diagnostic::new(
                    ErrorKind::InvalidConfiguration,
                    "unexpected deletion identity"
                ),
                ErrorKind::Dangling
            ),
            AdmissionFailure::Failed(_)
        ));
    }
}
