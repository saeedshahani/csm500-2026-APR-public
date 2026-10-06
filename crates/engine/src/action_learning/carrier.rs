use super::*;
use crate::{
    observation::Observer,
    recording::Recording,
    trace::{GraphData, TraceDocument, VerifiedTrace},
    Execution, ExecutionLimits,
};
use graph_core::{AtomKey, Binding, Graph, Kind, Role, Sort};
use serde_json::{json, Value};

/// A fresh ordinary carrier, tied to an immutable acquisition snapshot.
pub struct Compiled {
    pub graph: Graph,
    syntax: BTreeSet<AtomKey>,
    coordinates: Vec<GroundAtom>,
}
fn ctor(g: &mut Graph, label: String) -> Result<AtomKey, Error> {
    Ok(g.insert(Kind::Constructor(label), &[])?)
}
fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> Result<AtomKey, Error> {
    Ok(g.insert(Kind::record(role), fields)?)
}
fn cell(g: &mut Graph, key: AtomKey, value: AtomKey) -> Result<AtomKey, Error> {
    Ok(g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some("action.value".into()),
        },
        &[key, value],
    )?)
}
fn boolean(value: bool) -> String {
    format!("action.bool:{value}")
}

/// Compile model conditions and assignments, without evaluating the action word.
/// Each guarded stage is a rule over cells. An unmet condition blocks execution.
pub fn compile(learner: &Learner, request: &Run) -> Result<Compiled, Error> {
    if request.expected_version != learner.state.version || request.actions.len() > 16 {
        return Err(invalid("stale model version or action word bound exceeded"));
    }
    let world = learner.state.schema.world(&request.initial)?;
    let coordinates: Vec<_> = world.keys().cloned().collect();
    let indices: BTreeMap<_, _> = coordinates
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect();
    let mut graph = Graph::new();
    let mut syntax = BTreeSet::new();
    ctor(
        &mut graph,
        format!(
            "action.model:{}:{}",
            learner.state.version, learner.state.evidence_digest
        ),
    )?;
    for (stage, action) in request.actions.iter().enumerate() {
        let keys = learner.coordinates(action, &request.initial)?;
        let model = &learner.state.models[&action.name];
        if model.positive_count == 0 || model.quarantined {
            return Err(invalid("action has no usable acquired model"));
        }
        let head = graph.insert(Kind::RuleHead, &[])?;
        let zero = graph.insert(Kind::Nat, &[])?;
        record(&mut graph, Role::Rank, &[head, zero])?;
        record(&mut graph, Role::Revision, &[head])?;
        record(&mut graph, Role::ModeObject, &[head])?;
        record(&mut graph, Role::Active, &[head])?;
        ctor(
            &mut graph,
            format!(
                "action.rule:{stage}:epoch={}:{}",
                model.epoch,
                serde_json::to_string(action).map_err(|e| invalid(&e.to_string()))?
            ),
        )?;
        let mut lhs = vec![ctor(&mut graph, format!("action.stage:{stage}"))?];
        let mut rhs = vec![ctor(&mut graph, format!("action.stage:{}", stage + 1))?];
        for (i, key) in keys.iter().enumerate() {
            let pre = model.preconditions[i];
            let effect = model.effects[i];
            if pre.is_none() && effect.is_none() {
                continue;
            }
            let anchor = ctor(&mut graph, format!("action.coordinate:{}", indices[key]))?;
            lhs.push(anchor);
            rhs.push(anchor);
            let old = if let Some(v) = pre {
                ctor(&mut graph, boolean(v))?
            } else {
                graph.insert(Kind::variable(Sort::Constructor, Binding::Free), &[])?
            };
            let old_cell = cell(&mut graph, anchor, old)?;
            lhs.extend([old, old_cell]);
            if let Some(v) = effect {
                let new = ctor(&mut graph, boolean(v))?;
                let new_cell = cell(&mut graph, anchor, new)?;
                rhs.extend([new, new_cell]);
            } else {
                rhs.extend([old, old_cell]);
            }
        }
        for key in lhs {
            syntax.insert(key);
            record(&mut graph, Role::Lhs, &[head, key])?;
        }
        for key in rhs {
            syntax.insert(key);
            record(&mut graph, Role::Rhs, &[head, key])?;
        }
    }
    ctor(&mut graph, "action.stage:0".into())?;
    for (i, key) in coordinates.iter().enumerate() {
        let anchor = ctor(&mut graph, format!("action.coordinate:{i}"))?;
        let description = ctor(
            &mut graph,
            serde_json::to_string(key).map_err(|e| invalid(&e.to_string()))?,
        )?;
        graph.insert(
            Kind::Record {
                role: Role::ObjectLink,
                label: Some("action.fact".into()),
            },
            &[anchor, description],
        )?;
        let value = ctor(&mut graph, boolean(world[key]))?;
        cell(&mut graph, anchor, value)?;
    }
    graph_core::validate_with_limits(&graph, &Default::default())?;
    Ok(Compiled {
        graph,
        syntax,
        coordinates,
    })
}
impl Compiled {
    fn readback(&self, graph: &Graph) -> Result<Vec<Fact>, Error> {
        let mut facts = BTreeMap::new();
        for (key, atom) in graph.iter() {
            if self.syntax.contains(&key) {
                continue;
            }
            if matches!(&atom.kind, Kind::Record { role: Role::ObjectLink, label: Some(s) } if s == "action.value")
            {
                let anchor = graph
                    .get(atom.fields[0])
                    .ok_or_else(|| invalid("missing cell anchor"))?;
                let value = graph
                    .get(atom.fields[1])
                    .ok_or_else(|| invalid("missing cell value"))?;
                let Kind::Constructor(label) = &anchor.kind else {
                    return Err(invalid("invalid cell anchor"));
                };
                let index = label
                    .strip_prefix("action.coordinate:")
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|i| *i < self.coordinates.len())
                    .ok_or_else(|| invalid("invalid coordinate"))?;
                let v = match &value.kind {
                    Kind::Constructor(s) if s == "action.bool:true" => true,
                    Kind::Constructor(s) if s == "action.bool:false" => false,
                    _ => return Err(invalid("invalid Boolean cell")),
                };
                if facts.insert(index, v).is_some() {
                    return Err(invalid("duplicate cell"));
                }
            }
        }
        if facts.len() != self.coordinates.len() {
            return Err(invalid("incomplete readback"));
        }
        Ok(facts
            .into_iter()
            .map(|(i, value)| Fact {
                atom: self.coordinates[i].clone(),
                value,
            })
            .collect())
    }
}
/// Fit, compile and execute a bounded action word; replay the exported trace.
pub fn execute(request: &Request) -> Result<Value, Error> {
    let (learner, receipts) = fit(request)?;
    let mut result = json!({"schema":"action-learning-run-v1", "model":learner.snapshot(), "observations":receipts});
    if let Some(plan) = &request.run {
        let compiled = compile(&learner, plan)?;
        let mut limits = ExecutionLimits::default();
        if let Some(work) = plan.native_max_work {
            if work > limits.search.max_work {
                return Err(invalid("native work may only lower the default"));
            }
            limits.search.max_work = work;
        }
        let mut run = Execution::new(compiled.graph.clone(), limits)?;
        let mut recording = Recording::new(&run, Default::default())?;
        let mut states = vec![compiled.readback(run.graph())?];
        let mut admitted = 0;
        for _ in &plan.actions {
            let enabled = run.enabled()?.into_steps();
            if enabled.is_empty() {
                break;
            }
            if enabled.len() != 1 {
                return Err(invalid("ambiguous acquired action execution"));
            }
            recording.commit(&mut run, enabled.into_iter().next().expect("one step"))?;
            states.push(compiled.readback(run.graph())?);
            admitted += 1;
        }
        let doc = TraceDocument::from_recording(
            &recording,
            &Observer::all(),
            None,
            Default::default(),
            Default::default(),
        )?;
        let bytes = doc.to_json(16_000_000)?;
        let replay = VerifiedTrace::import(&bytes, Default::default())?.state_at(admitted)?;
        if GraphData::from_graph(&replay) != GraphData::from_graph(run.graph()) {
            return Err(invalid("action replay mismatch"));
        }
        result["execution"] = json!({"admitted_steps":admitted,"completed":admitted == plan.actions.len(),
            "states":states,"portable_trace_verified":true,"trace":doc});
    }
    Ok(result)
}
