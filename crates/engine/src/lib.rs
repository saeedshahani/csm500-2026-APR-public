//! Transactional single-step reference implementation with an explicit injective binding.
//! Match enumeration, scheduling, caching and concurrency are deliberately not implemented.
use graph_core::{ensure, validate, AtomKey, Binding, Graph, Kind, NewAtom, Result, Role, Target};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
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
    let before = validate(graph)?;
    let rule = before
        .rules
        .get(&source)
        .ok_or_else(|| graph_core::Diagnostic("source is not live".into()))?;
    ensure(rule.active, "source is inactive")?;
    ensure(
        binding.keys().copied().collect::<BTreeSet<_>>() == rule.lhs,
        "binding must cover exactly the LHS",
    )?;
    let image: BTreeSet<_> = binding.values().copied().collect();
    ensure(image.len() == binding.len(), "non-injective binding")?;
    let hidden: BTreeSet<_> = before
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    for (pattern, target) in binding {
        let p = graph.atom(*pattern)?;
        let t = graph.atom(*target)?;
        ensure(
            rule.meta || !hidden.contains(target),
            "object match enters rule representation",
        )?;
        match &p.kind {
            Kind::PatternVariable { admits, binding } => {
                ensure(admits.contains(&t.kind.sort()), "variable sort mismatch")?;
                ensure(
                    *binding != Binding::AppliedRule || *target == source,
                    "AppliedRule mismatch",
                )?;
            }
            _ => {
                ensure(
                    p.kind == t.kind && p.fields.len() == t.fields.len(),
                    "concrete atom mismatch",
                )?;
                ensure(
                    p.fields
                        .iter()
                        .zip(&t.fields)
                        .all(|(a, b)| binding.get(a) == Some(b)),
                    "field mismatch",
                )?;
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
    )?;
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
                            .ok_or_else(|| graph_core::Diagnostic("unresolved RHS field".into()))
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
    candidate.remove_set(&consumed)?;
    let produced: BTreeSet<_> = candidate.insert_batch(&batch)?.into_iter().collect();
    let after = validate(&candidate)?;
    let pre_heads: BTreeSet<_> = before.rules.keys().copied().collect();
    let post_heads: BTreeSet<_> = after.rules.keys().copied().collect();
    let entered: BTreeSet<_> = post_heads.difference(&pre_heads).copied().collect();
    let exited: BTreeSet<_> = pre_heads.difference(&post_heads).copied().collect();
    let mut altered = BTreeSet::new();
    for head in &entered {
        ensure(
            produced.contains(&after.rules[head].revision),
            "entry requires fresh revision",
        )?;
        for (id, atom) in graph
            .iter()
            .filter(|(_, a)| a.kind.role() == Some(Role::Revision) && a.fields[0] == *head)
        {
            let _ = atom;
            ensure(
                consumed.contains(&id),
                "entry retains an old revision template",
            )?;
        }
    }
    for head in &exited {
        ensure(
            consumed.contains(&before.rules[head].revision),
            "exit must consume executable revision",
        )?;
        if candidate.get(*head).is_none() {
            ensure(
                before.rules[head].representation.is_subset(&consumed),
                "head deletion must consume its representation",
            )?;
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
            )?;
        } else {
            ensure(
                pre.revision == post.revision,
                "unchanged rule cannot churn revision",
            )?;
        }
    }
    for head in entered.union(&altered) {
        ensure(
            after.rules[head].rank < rule.rank,
            "GC: affected post-rank is not below source pre-rank",
        )?;
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
