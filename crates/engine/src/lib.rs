//! Bounded complete matching, applicability and checked atomic graph transactions.
//! Reproducible scheduling and bounded observation are layered above admission.
pub mod causal;
mod checked;
pub mod conditions;
pub mod conflict;
pub mod counterfactual;
pub mod indexed;
mod matching;
pub mod meta_activity;
pub mod observation;
mod one_shot;
pub mod recording;
pub mod runner;
pub mod scheduling;
mod screened;
pub mod stability;
pub mod trace;
pub mod workflow;
pub use checked::{
    CandidateSearch, Execution, ExecutionLimits, GroundMatch, GroundRule, OperationalBinding,
    PreparedStep, SearchLimits,
};
use graph_core::{
    ensure, validate_with_limits, AtomKey, Binding, Configuration, Diagnostic, ErrorKind, Graph,
    GraphLimits, Kind, NewAtom, Result, Role, Target,
};
pub use matching::{EnabledSet, RejectedBinding};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub source: AtomKey,
    pub source_revision: AtomKey,
    pub consumed: BTreeSet<AtomKey>,
    pub preserved: BTreeSet<AtomKey>,
    pub produced: BTreeSet<AtomKey>,
    pub read: BTreeSet<AtomKey>,
    pub entered: BTreeSet<AtomKey>,
    pub exited: BTreeSet<AtomKey>,
    pub altered: BTreeSet<AtomKey>,
}

/// Apply only after checking matching, dangling, post-state validity, lifecycle and GC.
/// Any error leaves `graph`, including its identity allocator, unchanged.
pub fn apply_with_binding(
    graph: &mut Graph,
    source: AtomKey,
    binding: &BTreeMap<AtomKey, AtomKey>,
) -> Result<Event> {
    apply_with_binding_with_limits(graph, source, binding, &GraphLimits::default())
}

/// Reference path with explicit resource bounds. Construction edits between calls
/// are not guarded events and cannot silently extend a closed execution history.
pub fn apply_with_binding_with_limits(
    graph: &mut Graph,
    source: AtomKey,
    binding: &BTreeMap<AtomKey, AtomKey>,
    limits: &GraphLimits,
) -> Result<Event> {
    apply_candidate(graph, source, binding, limits).map_err(AdmissionFailure::diagnostic)
}

/// Only explicitly identified applicability predicates may become rejected matches.
/// All other errors abort general enumeration; resource errors are never rejection.
pub(crate) enum AdmissionFailure {
    Rejected(Diagnostic),
    Failed(Diagnostic),
}
impl AdmissionFailure {
    fn rejected(error: Diagnostic, expected: ErrorKind) -> Self {
        if error.kind() != expected || error.kind() == ErrorKind::ResourceExhaustion {
            Self::Failed(error)
        } else {
            Self::Rejected(error)
        }
    }
    pub(crate) fn diagnostic(self) -> Diagnostic {
        match self {
            Self::Rejected(e) | Self::Failed(e) => e,
        }
    }
}
impl From<Diagnostic> for AdmissionFailure {
    fn from(error: Diagnostic) -> Self {
        Self::Failed(error)
    }
}

pub(crate) fn apply_candidate(
    graph: &mut Graph,
    source: AtomKey,
    binding: &BTreeMap<AtomKey, AtomKey>,
    limits: &GraphLimits,
) -> std::result::Result<Event, AdmissionFailure> {
    let before = validate_with_limits(graph, limits)?;
    let (candidate, event, _) =
        prepare_from_validated(graph, source, binding, limits, &before, false)?;
    *graph = candidate;
    Ok(event)
}

/// Internal only: `before` must be the full validator's result for this exact
/// immutable carrier and limits. Execution owns that invariant; callers cannot
/// supply a detached/edited configuration through the public admission API.
pub(crate) fn prepare_from_validated(
    graph: &Graph,
    source: AtomKey,
    binding: &BTreeMap<AtomKey, AtomKey>,
    limits: &GraphLimits,
    before: &Configuration,
    framed: bool,
) -> std::result::Result<(Graph, Event, Configuration), AdmissionFailure> {
    let rule = before.rules.get(&source).ok_or_else(|| {
        Diagnostic::new(ErrorKind::InvalidBinding, "source is not live").at_atom(source)
    })?;
    let binding_error = |e: Diagnostic| e.with_kind(ErrorKind::InvalidBinding).at_atom(source);
    ensure(rule.active, "source is inactive").map_err(binding_error)?;
    ensure(
        binding.keys().eq(rule.lhs.iter()),
        "binding must cover exactly the LHS",
    )
    .map_err(binding_error)?;
    let image: BTreeSet<_> = binding.values().copied().collect();
    ensure(image.len() == binding.len(), "non-injective binding").map_err(binding_error)?;
    let hidden: BTreeSet<_> = before
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    for (pattern, target) in binding {
        let p = graph.atom(*pattern).map_err(binding_error)?;
        let t = graph.atom(*target).map_err(binding_error)?;
        let at_binding = |e: Diagnostic| {
            binding_error(e)
                .at_atom(*pattern)
                .at_atom(*target)
                .at_role(p.kind.role().or(t.kind.role()))
        };
        ensure(
            rule.meta || !hidden.contains(target),
            "object match enters rule representation",
        )
        .map_err(at_binding)?;
        match &p.kind {
            Kind::PatternVariable { admits, binding } => {
                ensure(admits.contains(&t.kind.sort()), "variable sort mismatch")
                    .map_err(at_binding)?;
                ensure(
                    *binding != Binding::AppliedRule || *target == source,
                    "AppliedRule mismatch",
                )
                .map_err(at_binding)?;
            }
            _ => {
                ensure(
                    p.kind == t.kind && p.fields.len() == t.fields.len(),
                    "concrete atom mismatch",
                )
                .map_err(at_binding)?;
                if let Some((slot, (a, b))) = p
                    .fields
                    .iter()
                    .zip(&t.fields)
                    .enumerate()
                    .find(|(_, (a, b))| binding.get(a) != Some(b))
                {
                    return Err(at_binding(Diagnostic::new(
                        ErrorKind::InvalidBinding,
                        "field mismatch",
                    ))
                    .at_atom(*a)
                    .at_atom(*b)
                    .at_slot(slot)
                    .into());
                }
            }
        }
    }
    let consumed: BTreeSet<_> = rule
        .lhs
        .difference(&rule.rhs)
        .map(|id| binding[id])
        .collect();
    let context = graph.closure(&image)?;
    ensure(
        rule.meta || context.is_disjoint(&hidden),
        "object grounding enters rule representation",
    )
    .map_err(binding_error)
    .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::InvalidBinding))?;
    let preserved = context.difference(&consumed).copied().collect();
    let mut read = context;
    read.extend(&rule.representation);
    let fresh: Vec<_> = rule.rhs.difference(&rule.lhs).copied().collect();
    let positions: BTreeMap<_, _> = fresh.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let batch = fresh
        .iter()
        .map(|id| {
            let atom = graph.atom(*id)?;
            let fields = atom
                .fields
                .iter()
                .map(|field| {
                    if let Some(index) = positions.get(field) {
                        Ok(Target::Fresh(*index))
                    } else {
                        binding
                            .get(field)
                            .copied()
                            .map(Target::Existing)
                            .ok_or_else(|| {
                                Diagnostic::new(ErrorKind::InvalidBinding, "unresolved RHS field")
                                    .at_atom(source)
                                    .at_atom(*id)
                                    .at_atom(*field)
                                    .at_role(atom.kind.role())
                            })
                    }
                })
                .collect::<Result<_>>()?;
            Ok(NewAtom {
                kind: atom.kind.clone(),
                fields,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut candidate = graph.clone();
    candidate
        .remove_set(&consumed)
        .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Dangling))?;
    let produced: BTreeSet<_> = candidate
        .insert_batch_with_limits(&batch, limits)?
        .into_iter()
        .collect();
    let after = if framed {
        data_frame(
            graph, &candidate, before, &hidden, &consumed, &produced, limits,
        )
    } else {
        None
    };
    let after = match after {
        Some(certificate) => certificate,
        None => validate_with_limits(&candidate, limits)
            .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::InvalidConfiguration))?,
    };
    let pre_heads: BTreeSet<_> = before.rules.keys().copied().collect();
    let post_heads: BTreeSet<_> = after.rules.keys().copied().collect();
    let entered: BTreeSet<_> = post_heads.difference(&pre_heads).copied().collect();
    let exited: BTreeSet<_> = pre_heads.difference(&post_heads).copied().collect();
    let mut altered = BTreeSet::new();
    for head in &entered {
        ensure(
            produced.contains(&after.rules[head].revision),
            "entry requires fresh revision",
        )
        .map_err(|e| {
            e.with_kind(ErrorKind::Lifecycle)
                .at_atom(*head)
                .at_role(Some(Role::Revision))
        })
        .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Lifecycle))?;
        for (id, atom) in graph
            .iter()
            .filter(|(_, a)| a.kind.role() == Some(Role::Revision) && a.fields[0] == *head)
        {
            let _ = atom;
            ensure(
                consumed.contains(&id),
                "entry retains an old revision template",
            )
            .map_err(|e| {
                e.with_kind(ErrorKind::Lifecycle)
                    .at_atom(*head)
                    .at_atom(id)
                    .at_role(Some(Role::Revision))
            })
            .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Lifecycle))?;
        }
    }
    for head in &exited {
        ensure(
            consumed.contains(&before.rules[head].revision),
            "exit must consume executable revision",
        )
        .map_err(|e| {
            e.with_kind(ErrorKind::Lifecycle)
                .at_atom(*head)
                .at_role(Some(Role::Revision))
        })
        .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Lifecycle))?;
        if candidate.get(*head).is_none() {
            ensure(
                before.rules[head].representation.is_subset(&consumed),
                "head deletion must consume its representation",
            )
            .map_err(|e| e.with_kind(ErrorKind::Lifecycle).at_atom(*head))
            .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Lifecycle))?;
        }
    }
    for head in pre_heads.intersection(&post_heads) {
        let pre = &before.rules[head];
        let post = &after.rules[head];
        if pre.body != post.body {
            altered.insert(*head);
            ensure(
                consumed.contains(&pre.revision) && produced.contains(&post.revision),
                "alteration requires revision replacement",
            )
            .map_err(|e| {
                e.with_kind(ErrorKind::Lifecycle)
                    .at_atom(*head)
                    .at_role(Some(Role::Revision))
            })
            .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Lifecycle))?;
        } else {
            ensure(
                pre.revision == post.revision,
                "unchanged rule cannot churn revision",
            )
            .map_err(|e| {
                e.with_kind(ErrorKind::Lifecycle)
                    .at_atom(*head)
                    .at_role(Some(Role::Revision))
            })
            .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Lifecycle))?;
        }
    }
    for head in entered.union(&altered) {
        ensure(
            after.rules[head].rank < rule.rank,
            "GC: affected post-rank is not below source pre-rank",
        )
        .map_err(|e| {
            e.with_kind(ErrorKind::Guardedness)
                .at_atom(*head)
                .at_atom(source)
                .at_role(Some(Role::Rank))
        })
        .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::Guardedness))?;
    }
    let event = Event {
        source,
        source_revision: rule.revision,
        consumed,
        preserved,
        produced,
        read,
        entered,
        exited,
        altered,
    };
    Ok((candidate, event, after))
}

/// Ordinary-data frame lemma. No membership, metadata, rank, variable or quoted
/// syntax changes. Existing incidences are immutable; checked deletion established
/// the dangling condition. New links have constructor endpoints. Thus all decoded
/// representations and recursive packages are unchanged. The exact full-validator
/// charge changes only with top-level atom/field scan coefficients.
fn data_frame(
    before_graph: &Graph,
    after_graph: &Graph,
    before: &Configuration,
    hidden: &BTreeSet<AtomKey>,
    consumed: &BTreeSet<AtomKey>,
    produced: &BTreeSet<AtomKey>,
    limits: &GraphLimits,
) -> Option<Configuration> {
    // Nested-package validation has additional outer-carrier closure charges;
    // keep the full validator until a separate coefficient certificate covers it.
    if before.rules.values().any(|r| !r.package_atoms.is_empty()) {
        return None;
    }
    let ordinary = |atom: &graph_core::Atom| {
        matches!(
            atom.kind,
            Kind::Constructor(_)
                | Kind::Record {
                    role: Role::ObjectLink,
                    ..
                }
        )
    };
    if !hidden.is_disjoint(consumed)
        || consumed
            .iter()
            .any(|id| !ordinary(before_graph.get(*id).unwrap()))
    {
        return None;
    }
    for id in produced {
        let atom = after_graph.get(*id)?;
        if !ordinary(atom)
            || atom.fields.iter().any(|target| {
                !matches!(
                    after_graph.get(*target).map(|a| &a.kind),
                    Some(Kind::Constructor(_))
                )
            })
        {
            return None;
        }
    }
    let rules = before.rules.len() as i128;
    let active = before.rules.values().filter(|r| r.active).count() as i128;
    let work = (before.validation_work as i128)
        + (2 + 2 * rules + 2 * active) * (after_graph.len() as i128 - before_graph.len() as i128)
        + (rules + 2 * active)
            * (after_graph.field_count() as i128 - before_graph.field_count() as i128);
    let work = usize::try_from(work).ok()?;
    if work > limits.max_validation_work {
        return None;
    }
    let mut after = before.clone();
    after.validation_work = work;
    Some(after)
}
