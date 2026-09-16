//! Independent ranked-heap expectations for lambda-encoding.md T1–T2.
//! This is a comparison harness, not a new production language or evaluator.
#[allow(dead_code)]
mod support;
use engine::{Execution, ExecutionLimits};
use graph_core::{validate, AtomKey, ErrorKind, Graph, Kind, Role};
use std::collections::{BTreeMap, BTreeSet};
use support::{ctor, link, rule, sides};

type Heap = BTreeMap<AtomKey, (String, Vec<AtomKey>)>;

fn arity(symbol: &str) -> Option<usize> {
    match symbol {
        "pair" => Some(2),
        "next" => Some(1),
        "a" | "b" => Some(0),
        _ => None,
    }
}

fn encode(g: &mut Graph, nodes: &[(&str, &[usize])]) -> Vec<AtomKey> {
    let ids: Vec<_> = nodes.iter().map(|(label, _)| ctor(g, label)).collect();
    for (owner, (label, children)) in nodes.iter().enumerate() {
        assert_eq!(arity(label), Some(children.len()));
        for (slot, child) in children.iter().enumerate() {
            link(g, &format!("arg:{slot}"), ids[owner], ids[*child]);
        }
    }
    ids
}

// Decode the data heap independently of rule matching and transaction deltas.
// Signature constraints belong to this interpretation, not core validation.
fn decode(g: &Graph) -> Result<Heap, &'static str> {
    let configuration = validate(g).map_err(|_| "invalid carrier")?;
    let hidden: BTreeSet<_> = configuration
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    let mut fields = BTreeMap::new();
    let mut vertices = BTreeMap::new();
    for (id, atom) in g.iter().filter(|(id, _)| !hidden.contains(id)) {
        match &atom.kind {
            Kind::Constructor(label) => {
                arity(label).ok_or("unknown symbol")?;
                vertices.insert(id, label.clone());
            }
            Kind::Record {
                role: Role::ObjectLink,
                label: Some(label),
            } => {
                let slot = label
                    .strip_prefix("arg:")
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or("invalid position")?;
                if *label != format!("arg:{slot}") {
                    return Err("noncanonical position");
                }
                if fields
                    .insert((atom.fields[0], slot), atom.fields[1])
                    .is_some()
                {
                    return Err("duplicate position");
                }
            }
            _ => return Err("non-term data"),
        }
    }
    for ((owner, slot), target) in &fields {
        let label = vertices.get(owner).ok_or("missing owner")?;
        if *slot >= arity(label).unwrap() || !vertices.contains_key(target) {
            return Err("invalid incidence");
        }
    }
    vertices
        .into_iter()
        .map(|(id, label)| {
            let children = (0..arity(&label).unwrap())
                .map(|slot| fields.get(&(id, slot)).copied().ok_or("missing position"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((id, (label, children)))
        })
        .collect()
}

fn unfold(heap: &Heap, node: AtomKey, depth: usize) -> String {
    if depth == 0 {
        return "…".into();
    }
    let (symbol, children) = &heap[&node];
    format!(
        "{}({})",
        symbol,
        children
            .iter()
            .map(|child| unfold(heap, *child, depth - 1))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[test]
fn ranked_incidence_preserves_sharing_positions_and_cyclic_prefixes() {
    for nodes in [
        vec![("pair", vec![1, 1]), ("a", vec![])],
        vec![("pair", vec![1, 2]), ("a", vec![]), ("b", vec![])],
        vec![("next", vec![0])],
    ] {
        let mut g = Graph::new();
        let borrowed: Vec<_> = nodes.iter().map(|(s, c)| (*s, c.as_slice())).collect();
        let ids = encode(&mut g, &borrowed);
        let expected: Heap = nodes
            .iter()
            .enumerate()
            .map(|(i, (s, c))| (ids[i], (s.to_string(), c.iter().map(|j| ids[*j]).collect())))
            .collect();
        assert_eq!(decode(&g).unwrap(), expected);
        if nodes[0].0 == "next" {
            assert_eq!(unfold(&expected, ids[0], 3), "next(next(next(…)))");
        }
    }
    let mut shared = Graph::new();
    let a = encode(&mut shared, &[("pair", &[1, 1]), ("a", &[])]);
    let mut copied = Graph::new();
    let b = encode(&mut copied, &[("pair", &[1, 2]), ("a", &[]), ("a", &[])]);
    assert_eq!(
        unfold(&decode(&shared).unwrap(), a[0], 3),
        unfold(&decode(&copied).unwrap(), b[0], 3)
    );
    assert_ne!(shared.len(), copied.len()); // Unfolding equality loses sharing.
}

#[test]
fn an_explicit_argument_redirect_preserves_the_other_shared_edge_and_program() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "pair");
    let a = ctor(&mut g, "a");
    let b = ctor(&mut g, "b");
    let old = link(&mut g, "arg:0", p, a);
    let new = link(&mut g, "arg:0", p, b);
    sides(&mut g, h, &[p, a, b, old], &[p, a, b, new]);
    let program = g.snapshot();
    let ids = encode(&mut g, &[("pair", &[1, 1]), ("a", &[]), ("b", &[])]);
    let mut expected = decode(&g).unwrap();
    expected.get_mut(&ids[0]).unwrap().1[0] = ids[2];
    let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
    let mut enabled = run.enabled().unwrap().into_steps();
    assert_eq!(enabled.len(), 1);
    let event = run.commit(enabled.pop().unwrap()).unwrap();
    assert_eq!(event.consumed.len(), 1);
    assert_eq!(event.produced.len(), 1);
    assert!(event.entered.is_empty() && event.exited.is_empty() && event.altered.is_empty());
    assert_eq!(decode(run.graph()).unwrap(), expected);
    for (id, atom) in program {
        assert_eq!(run.graph().get(id), Some(&atom));
    }
}

#[test]
fn cyclic_argument_redirect_uses_one_repeated_template_identity() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "next");
    let a = ctor(&mut g, "a");
    let old = link(&mut g, "arg:0", p, p);
    let new = link(&mut g, "arg:0", p, a);
    sides(&mut g, h, &[p, a, old], &[p, a, new]);
    let ids = encode(&mut g, &[("next", &[0]), ("a", &[])]);
    let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
    let mut steps = run.enabled().unwrap().into_steps();
    assert_eq!(steps.len(), 1);
    run.commit(steps.pop().unwrap()).unwrap();
    assert_eq!(
        unfold(&decode(run.graph()).unwrap(), ids[0], 3),
        "next(a())"
    );
}

#[test]
fn deleting_a_shared_vertex_requires_accounting_for_every_incoming_record() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "pair");
    let a = ctor(&mut g, "a");
    let old = link(&mut g, "arg:0", p, a);
    sides(&mut g, h, &[p, a, old], &[p]);
    encode(&mut g, &[("pair", &[1, 1]), ("a", &[])]);
    let run = Execution::new(g, ExecutionLimits::default()).unwrap();
    let before = run.graph().snapshot();
    let enabled = run.enabled().unwrap();
    assert!(enabled.steps().is_empty());
    assert_eq!(enabled.rejected().len(), 1);
    assert_eq!(enabled.rejected()[0].diagnostic.kind(), ErrorKind::Dangling);
    assert_eq!(run.graph().snapshot(), before);
}

#[test]
fn carrier_validity_does_not_enforce_the_ranked_term_signature() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let p = ctor(&mut g, "pair");
    let a = ctor(&mut g, "a");
    let old = link(&mut g, "arg:0", p, a);
    sides(&mut g, h, &[p, a, old], &[p, a]);
    encode(&mut g, &[("pair", &[1, 1]), ("a", &[])]);
    let mut run = Execution::new(g, ExecutionLimits::default()).unwrap();
    let step = run.enabled().unwrap().into_steps().pop().unwrap();
    run.commit(step).unwrap();
    validate(run.graph()).unwrap();
    assert_eq!(decode(run.graph()), Err("missing position"));
}
