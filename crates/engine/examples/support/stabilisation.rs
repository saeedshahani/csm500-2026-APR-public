//! Independently instantiated finite fault domain; faults create new baselines.
#[allow(dead_code)]
#[path = "body_revisions.rs"]
pub mod bodies;
use engine::{stability::Contract, Execution};
use graph_core::{Graph, Kind, Role};
use std::collections::BTreeSet;
pub fn single(status: &str, upgraded: bool) -> (Graph, Contract) {
    let mut graph = parser::load(include_str!(
        "../../../../examples/practical/repair-policy.graph"
    ))
    .unwrap()
    .into_graph();
    let contract: Contract = serde_json::from_str(include_str!(
        "../../../../examples/practical/repair-policy.contract.json"
    ))
    .unwrap();
    set_status(&mut graph, "Ticket", &[status]);
    if upgraded {
        let cfg = graph_core::validate(&graph).unwrap();
        let mut run = Execution::new(graph, Default::default()).unwrap();
        let step = run
            .enabled()
            .unwrap()
            .into_steps()
            .into_iter()
            .find(|s| cfg.rules[&s.binding().source()].meta)
            .unwrap();
        run.commit(step).unwrap();
        graph = run.graph().clone();
    }
    (graph, contract)
}
pub fn set_status(g: &mut Graph, name: &str, statuses: &[&str]) {
    let cfg = graph_core::validate(g).unwrap();
    let visible = |k| cfg.rules.values().all(|r| !r.representation.contains(&k));
    let item = g
        .iter()
        .find(|(k, a)| visible(*k) && a.kind == Kind::Constructor(name.into()))
        .unwrap()
        .0;
    let removed: BTreeSet<_> = g
        .iter()
        .filter(|(k, a)| {
            visible(*k)
                && matches!(&a.kind,Kind::Record{role:Role::ObjectLink,..} if a.fields[0]==item)
        })
        .map(|(k, _)| k)
        .collect();
    g.remove_set(&removed).unwrap();
    for status in statuses {
        g.insert(
            Kind::Record {
                role: Role::ObjectLink,
                label: Some((*status).into()),
            },
            &[item, item],
        )
        .unwrap();
    }
}
pub fn family(states: &[usize]) -> (Graph, Contract) {
    assert!(!states.is_empty());
    let mut graph = Graph::new();
    let mut contract = single("done", true).1;
    contract.preserve.clear();
    contract.requirements.clear();
    for (i, s) in states.iter().enumerate() {
        assert!(*s < 6);
        let (g, c) = single(["pending", "audited", "done"][s % 3], *s >= 3);
        let prefix = format!("{i}:");
        bodies::append(&mut graph, &g, &prefix);
        let c = bodies::prefixed(&c, &prefix);
        contract.preserve.extend(c.preserve);
        contract.requirements.extend(c.requirements);
    }
    (graph, contract)
}

/// Independent abstract transitions for one policy/data component.
pub fn abstract_next(s: usize) -> Vec<usize> {
    let mut out = Vec::new();
    if s < 3 {
        out.push(s + 3);
    }
    match s % 3 {
        0 => out.push(if s < 3 { 2 } else { 4 }),
        1 => out.push(s + 1),
        _ => (),
    }
    out
}
pub fn potential(states: &[usize]) -> usize {
    states
        .iter()
        .map(|s| [2, 1, 0][s % 3] + usize::from(*s < 3))
        .sum()
}
