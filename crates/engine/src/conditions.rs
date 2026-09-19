//! Absence witnesses for monotone removal of constructor quotation memberships.
//! This sidecar neither modifies the portable trace nor supplies general causality.
use crate::causal::{AnalysisLimits, Budget};
use crate::recording::{AtomId, AtomRecord, EventId};
use crate::trace::VerifiedTrace;
use graph_core::{Diagnostic, ErrorKind, Kind, Result, Role};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "reflective-visibility-witnesses-v1";
pub const FRAGMENT: &str = "monotone-constructor-unquotation-v1";
pub const LIMITATION: &str = "Necessary constructor-visibility conditions within one checked history, not sufficient admission, complete causality, counterfactual executions or independence. New operative constructor memberships are outside this fragment.";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Witness {
    pub removed_by: EventId,
    pub used_by: EventId,
    pub atom: AtomId,
    pub membership: AtomId,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct VisibilityReport {
    pub schema: &'static str,
    pub fragment: &'static str,
    pub limitation: &'static str,
    pub initial_sha256: String,
    pub witnesses: Vec<Witness>,
}

// Independent of matching: derive operative membership by the carrier definition.
fn scan(
    atoms: &BTreeMap<AtomId, AtomRecord>,
    budget: &mut Budget,
) -> Result<(BTreeMap<AtomId, AtomId>, BTreeSet<AtomId>)> {
    budget.work(atoms.len())?;
    budget.retain(atoms.len().saturating_mul(3))?;
    let named: BTreeSet<_> = atoms
        .values()
        .filter(|a| a.kind.role().is_some_and(Role::membership))
        .map(|a| a.fields[1])
        .collect();
    let mut blockers = BTreeMap::new();
    let mut quoted = BTreeSet::new();
    for (id, a) in atoms {
        budget.work(1)?;
        if a.kind.role().is_some_and(Role::membership) && !named.contains(id) {
            let target = a.fields[1];
            quoted.insert(target);
            if matches!(atoms[&target].kind, Kind::Constructor(_)) {
                blockers.insert(*id, target);
            }
        }
    }
    Ok((blockers, quoted))
}

/// Only accepts an already replay-verified trace. Rejects the entire result on
/// fragment escape or resource exhaustion; no partial coverage is silently emitted.
pub fn extract(trace: &VerifiedTrace, limits: AnalysisLimits) -> Result<VisibilityReport> {
    let doc = trace.document();
    let mut budget = Budget::new(limits);
    let size = |a: &AtomRecord| {
        1usize
            .saturating_add(a.fields.len())
            .saturating_add(match &a.kind {
                Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => s.len(),
                _ => 0,
            })
    };
    for a in doc.initial.atoms.values() {
        budget.work(size(a))?;
        budget.retain(size(a))?;
    }
    let mut atoms = doc.initial.atoms.clone();
    let (initial, _) = scan(&atoms, &mut budget)?;
    let mut removed: BTreeMap<AtomId, Vec<(AtomId, EventId)>> = BTreeMap::new();
    let mut witnesses = Vec::new();
    for index in 0..=doc.events.len() {
        let (blockers, quoted) = scan(&atoms, &mut budget)?;
        for (id, target) in &initial {
            budget.work(1)?;
            if atoms.contains_key(id) && blockers.get(id) != Some(target) {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidConfiguration,
                    "visibility fragment excludes quotation of an existing constructor membership",
                ));
            }
        }
        for (id, target) in &blockers {
            budget.work(1)?;
            if initial.get(id) != Some(target) {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidConfiguration,
                    "visibility fragment excludes newly operative constructor memberships",
                ));
            }
        }
        let Some(event) = doc.events.get(index) else {
            break;
        };
        budget.work(atoms.len())?;
        let object_mode = atoms.iter().any(|(id, a)| {
            !quoted.contains(id)
                && a.kind.role() == Some(Role::ModeObject)
                && a.fields[0] == event.redex.source
        });
        if object_mode {
            let mut seen = BTreeSet::new();
            for (_, target) in &event.redex.binding {
                budget.work(1)?;
                budget.retain(1)?;
                if !seen.insert(*target) || !matches!(atoms[target].kind, Kind::Constructor(_)) {
                    continue;
                }
                // Checked object matching establishes absence; never infer admission from it.
                debug_assert!(!quoted.contains(target));
                for (membership, removed_by) in removed.get(target).into_iter().flatten() {
                    budget.work(1)?;
                    budget.retain(4)?;
                    witnesses.push(Witness {
                        removed_by: *removed_by,
                        used_by: event.id,
                        atom: *target,
                        membership: *membership,
                    });
                }
            }
        }
        for id in &event.footprint.consumed {
            budget.work(1)?;
            if let Some(target) = initial.get(id) {
                budget.retain(3)?;
                removed.entry(*target).or_default().push((*id, event.id));
            }
            atoms.remove(id);
        }
        for (id, a) in &event.fresh {
            budget.work(size(a))?;
            budget.retain(size(a))?;
            atoms.insert(*id, a.clone());
        }
    }
    Ok(VisibilityReport {
        schema: SCHEMA,
        fragment: FRAGMENT,
        limitation: LIMITATION,
        initial_sha256: doc.initial_sha256.clone(),
        witnesses,
    })
}
