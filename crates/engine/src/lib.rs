//! Bounded complete matching, applicability and checked atomic graph transactions.
//! Reproducible scheduling and bounded observation are layered above admission.
mod checked;
mod matching;
pub mod observation;
pub mod runner;
pub mod scheduling;
pub use checked::{
    CandidateSearch, Execution, ExecutionLimits, GroundMatch, GroundRule, OperationalBinding,
    PreparedStep, SearchLimits,
};
use graph_core::{
    ensure, validate_with_limits, AtomKey, Binding, Diagnostic, ErrorKind, Graph, GraphLimits,
    Kind, NewAtom, Result, Role, Target,
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
    let after = validate_with_limits(&candidate, limits)
        .map_err(|error| AdmissionFailure::rejected(error, ErrorKind::InvalidConfiguration))?;
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
    *graph = candidate;
    Ok(event)
}
