use super::{verify_packet, Error, Packet};
use crate::observation::Observer;
use crate::recording::Recording;
use crate::trace::{GraphData, TraceDocument, VerifiedTrace};
use crate::{Execution, ExecutionLimits};
use graph_core::{AtomKey, Binding, Graph, Kind, Role, Sort};
use serde_json::{json, Value};

fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> Result<AtomKey, Error> {
    Ok(g.insert(Kind::record(role), fields)?)
}
fn link(g: &mut Graph, label: &str, fields: &[AtomKey]) -> Result<AtomKey, Error> {
    Ok(g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(label.into()),
        },
        fields,
    )?)
}
/// Verify then transport the exact packet using ordinary admission and replay.
/// Exported consistency is not origin authentication; the caller owns the context.
pub fn publish(packet: &Packet) -> Result<Value, Error> {
    verify_packet(packet)?;
    let payload =
        serde_json::to_string(&serde_json::to_value(packet).map_err(|_| "packet serialization")?)
            .map_err(|_| "packet serialization")?;
    let mut graph = Graph::new();
    let head = graph.insert(Kind::RuleHead, &[])?;
    let zero = graph.insert(Kind::Nat, &[])?;
    record(&mut graph, Role::Rank, &[head, zero])?;
    for role in [Role::ModeObject, Role::Revision, Role::Active] {
        record(&mut graph, role, &[head])?;
    }
    let pattern = graph.insert(Kind::Constructor("NumericalComponent".into()), &[])?;
    let variable = graph.insert(Kind::variable(Sort::Constructor, Binding::Free), &[])?;
    let pending = link(&mut graph, "pending", &[pattern, variable])?;
    let done = link(&mut graph, packet.status().label(), &[pattern, variable])?;
    for atom in [pattern, variable, pending] {
        record(&mut graph, Role::Lhs, &[head, atom])?;
    }
    for atom in [pattern, variable, done] {
        record(&mut graph, Role::Rhs, &[head, atom])?;
    }
    let component = graph.insert(Kind::Constructor("NumericalComponent".into()), &[])?;
    let value = graph.insert(Kind::Constructor(payload.clone()), &[])?;
    link(&mut graph, "pending", &[component, value])?;
    let mut run = Execution::new(graph, ExecutionLimits::default())?;
    let mut log = Recording::new(&run, Default::default())?;
    let mut enabled = run.enabled()?.into_steps();
    if enabled.len() != 1 {
        return Err("publication requires one admitted step".into());
    }
    log.commit(&mut run, enabled.pop().ok_or("missing publication step")?)?;
    if !run.enabled()?.into_steps().is_empty() {
        return Err("publication did not terminate".into());
    }
    let exact = run
        .graph()
        .iter()
        .any(|(id, a)| id == value && a.kind == Kind::Constructor(payload.clone()));
    let links: Vec<_> = run
        .graph()
        .iter()
        .filter(|(_, a)| {
            matches!(
                a.kind,
                Kind::Record {
                    role: Role::ObjectLink,
                    ..
                }
            ) && a.fields.as_slice() == [component, value]
        })
        .collect();
    if !exact
        || links.len() != 1
        || links[0].1.kind
            != (Kind::Record {
                role: Role::ObjectLink,
                label: Some(packet.status().label().into()),
            })
    {
        return Err("publication endpoint mismatch".into());
    }
    let doc = TraceDocument::from_recording(
        &log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )?;
    let bytes = doc.to_json(2_000_000)?;
    let checked = VerifiedTrace::import(&bytes, Default::default())?;
    if GraphData::from_graph(&checked.state_at(1)?) != GraphData::from_graph(run.graph()) {
        return Err("publication replay mismatch".into());
    }
    Ok(
        json!({"packet":packet,"trace":doc,"events":1,"atoms":run.graph().len(),"trace_checked":true,"replayed":true}),
    )
}
