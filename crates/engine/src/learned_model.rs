//! Bounded compilation of an acquired deterministic transition table.
//!
//! Acquisition and the host organization are outside admission. Once compiled,
//! each action is exactly one admitted object rewrite, recorded and re-imported.
//! This API establishes table/trace correspondence, not environmental truth.
use crate::observation::Observer;
use crate::recording::Recording;
use crate::trace::{GraphData, TraceDocument, VerifiedTrace};
use crate::{Execution, ExecutionLimits};
use graph_core::{AtomKey, Diagnostic, Graph, Kind, Role};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Malformed model input is distinct from native resource/admission failure.
#[derive(Debug)]
pub enum Error {
    Input(String),
    Native(Diagnostic),
}
impl From<Diagnostic> for Error {
    fn from(value: Diagnostic) -> Self {
        Self::Native(value)
    }
}
impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::Input(value.to_owned())
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(s) => f.write_str(s),
            Self::Native(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}

fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> Result<AtomKey, Error> {
    Ok(g.insert(Kind::record(role), fields)?)
}
fn ctor(g: &mut Graph, label: String) -> Result<AtomKey, Error> {
    Ok(g.insert(Kind::Constructor(label), &[])?)
}

/// Execute a supplied finite binary word under a supplied deterministic table.
///
/// Input fields: `states` (1..32 distinct names of at most 256 UTF-8 bytes),
/// `transitions` (at most 64 `{from, action, target}` records), `start`, and
/// `word` (at most 16 binary actions). Optional `native_max_work` may lower the
/// normal search budget. Additional record metadata is ignored. Undefined
/// transitions on the requested path are rejected before constructing a graph.
/// These are input bounds, not a guarantee of completion: normal validation,
/// matching, recording and replay budgets still apply to the compiled graph.
/// The returned object includes projections and a portable verified trace.
pub fn execute(input: &Value) -> Result<Value, Error> {
    let states = input["states"].as_array().ok_or("states missing")?;
    if states.is_empty() || states.len() > 32 {
        return Err("state bound 1..32".into());
    }
    let names: Vec<&str> = states
        .iter()
        .map(|s| {
            s.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 256)
                .ok_or("state must contain 1..256 UTF-8 bytes")
        })
        .collect::<Result<_, _>>()?;
    if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
        return Err("duplicate state".into());
    }
    let ids: BTreeMap<_, _> = names.iter().enumerate().map(|(i, s)| (*s, i)).collect();
    let transitions = input["transitions"]
        .as_array()
        .ok_or("transitions missing")?;
    if transitions.len() > 64 {
        return Err("transition bound 64".into());
    }
    let mut model = BTreeMap::new();
    for t in transitions {
        let from = *ids
            .get(t["from"].as_str().ok_or("bad from")?)
            .ok_or("unknown from")?;
        let to = *ids
            .get(t["target"].as_str().ok_or("bad target")?)
            .ok_or("unknown target")?;
        let action = t["action"]
            .as_u64()
            .filter(|a| *a < 2)
            .ok_or("invalid action")?;
        if model.insert((from, action), to).is_some() {
            return Err("duplicate state-action".into());
        }
    }
    let actions = input["word"].as_array().ok_or("word missing")?;
    if actions.len() > 16 {
        return Err("word bound 16".into());
    }
    let word: Vec<u64> = actions
        .iter()
        .map(|a| a.as_u64().filter(|a| *a < 2).ok_or("invalid word action"))
        .collect::<Result<_, _>>()?;
    let start = *ids
        .get(input["start"].as_str().ok_or("start missing")?)
        .ok_or("unknown start")?;
    let mut state = start;
    let mut expected = vec![names[start]];
    for action in &word {
        state = *model
            .get(&(state, *action))
            .ok_or("unknown transition on requested path")?;
        expected.push(names[state]);
    }
    let mut limits = ExecutionLimits::default();
    if let Some(value) = input.get("native_max_work") {
        let work = value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("invalid work budget")?;
        if work > limits.search.max_work {
            return Err("work budget may only lower the native default".into());
        }
        limits.search.max_work = work;
    }
    let mut graph = Graph::new();
    let mut syntax = BTreeSet::new();
    for (i, action) in word.iter().enumerate() {
        for ((s, a), t) in &model {
            if a != action {
                continue;
            }
            let head = graph.insert(Kind::RuleHead, &[])?;
            let zero = graph.insert(Kind::Nat, &[])?;
            record(&mut graph, Role::Rank, &[head, zero])?;
            record(&mut graph, Role::Revision, &[head])?;
            record(&mut graph, Role::ModeObject, &[head])?;
            record(&mut graph, Role::Active, &[head])?;
            let lhs = ctor(&mut graph, format!("at:{i}:{s}"))?;
            let rhs = ctor(&mut graph, format!("at:{}:{t}", i + 1))?;
            syntax.extend([lhs, rhs]);
            record(&mut graph, Role::Lhs, &[head, lhs])?;
            record(&mut graph, Role::Rhs, &[head, rhs])?;
        }
    }
    ctor(&mut graph, format!("at:0:{start}"))?;
    // No external graph edits occur after this boundary.
    let mut run = Execution::new(graph, limits)?;
    let mut log = Recording::new(&run, Default::default())?;
    let mut projected = vec![names[start]];
    for i in 0..word.len() {
        let enabled = run.enabled()?.into_steps();
        if enabled.len() != 1 {
            return Err("native cursor must enable exactly one step".into());
        }
        let step = enabled.into_iter().next().ok_or("missing admitted step")?;
        log.commit(&mut run, step)?;
        let mut cursor = None;
        for (key, atom) in run.graph().iter() {
            if syntax.contains(&key) {
                continue;
            }
            if let Kind::Constructor(label) = &atom.kind {
                let prefix = format!("at:{}:", i + 1);
                let index = label
                    .strip_prefix(&prefix)
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|s| *s < names.len())
                    .ok_or("malformed native cursor")?;
                if cursor.replace(names[index]).is_some() {
                    return Err("native cursor not unique".into());
                }
            }
        }
        projected.push(cursor.ok_or("native cursor missing")?);
    }
    if projected != expected {
        return Err("correspondence failure".into());
    }
    if !run.enabled()?.into_steps().is_empty() {
        return Err("unexpected final step".into());
    }
    let doc = TraceDocument::from_recording(
        &log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )?;
    let bytes = doc.to_json(16_000_000)?;
    let checked = VerifiedTrace::import(&bytes, Default::default())?;
    let replay = checked.state_at(word.len())?;
    if GraphData::from_graph(&replay) != GraphData::from_graph(run.graph()) {
        return Err("replay mismatch".into());
    }
    Ok(
        json!({"schema":"learned-model-run-v1","expected":expected,"projected":projected,
        "admitted_steps":word.len(),"portable_trace_verified":true,"trace":doc}),
    )
}
