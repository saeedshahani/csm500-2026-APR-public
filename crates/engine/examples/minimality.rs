//! Research bridge: bounded native corpus and explicit candidate admission.
//! The universal encodings themselves are independently implemented in JavaScript.
use engine::recording::Footprint;
use engine::trace::GraphData;
use engine::{Execution, OperationalBinding};
use graph_core::{validate, Graph};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead};

fn query(graph: &Graph, binding: &OperationalBinding) -> Value {
    json!({"graph": GraphData::from_graph(graph), "source": binding.source().number().to_string(),
        "binding":binding.assignments().iter().map(|(a,b)|[a.number().to_string(),b.number().to_string()]).collect::<Vec<_>>()})
}
fn answer(q: Value) -> Value {
    let data: GraphData = serde_json::from_value(q["graph"].clone()).unwrap();
    let graph = data.restore(&Default::default()).unwrap();
    let mut run = Execution::new(graph, Default::default()).unwrap();
    let key = |x: &Value| {
        run.graph()
            .key(x.as_str().unwrap().parse().unwrap())
            .unwrap()
    };
    let binding = OperationalBinding::new(
        key(&q["source"]),
        q["binding"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (key(&p[0]), key(&p[1])))
            .collect(),
    );
    match run.prepare(binding) {
        Ok(step) => {
            let event = run.commit(step).unwrap();
            json!({"status":"Admitted", "graph":GraphData::from_graph(run.graph()),
                "source":event.source.number().to_string(),"revision":event.source_revision.number().to_string(),
                "footprint":Footprint::from(&event)})
        }
        Err(error) => {
            json!({"status":format!("{:?}",error.kind()),"graph":GraphData::from_graph(run.graph())})
        }
    }
}
fn main() {
    let mode = std::env::args().nth(1).expect("corpus|admit");
    if mode == "admit" {
        for line in io::stdin().lock().lines() {
            println!("{}", answer(serde_json::from_str(&line.unwrap()).unwrap()));
        }
        return;
    }
    assert_eq!(mode, "corpus");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
            .unwrap();
    let paths: BTreeSet<_> = manifest
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["authored"].as_str())
        .collect();
    let mut graphs = Vec::new();
    for name in ["a2", "a4", "b2", "c1"] {
        let f = graph_core::fixture::load(
            &std::fs::read_to_string(root.join(format!("fixtures/{name}.atoms"))).unwrap(),
        )
        .unwrap();
        graphs.push((format!("fixture/{name}"), f.graph));
    }
    for name in paths {
        let text = std::fs::read_to_string(root.join(name)).unwrap();
        let g = if name.ends_with(".lambda") {
            let term = parser::lambda::parse(&text, Default::default()).unwrap();
            parser::lambda::encode(&term, Default::default())
                .unwrap()
                .graph
        } else {
            parser::load(&text).unwrap().into_graph()
        };
        graphs.push((name.into(), g));
    }
    let mut snapshots = Vec::new();
    let mut cases = Vec::new();
    for (name, graph) in graphs {
        let mut run = Execution::new(graph, Default::default()).unwrap();
        // Four deterministic prefix states; enumerate every raw binding at each.
        for prefix in 0..4 {
            let configuration = validate(run.graph()).unwrap();
            let pattern: BTreeSet<_> = configuration
                .rules
                .values()
                .flat_map(|r| r.lhs.union(&r.rhs).map(|id| id.number().to_string()))
                .collect();
            snapshots.push(
                json!({"name":name,"prefix":prefix,"graph":GraphData::from_graph(run.graph()),"pattern":pattern}),
            );
            for &head in configuration.rules.keys() {
                let mut bindings = run.bindings(head).unwrap();
                // Empty proposals also challenge admission (not necessarily raw matches).
                bindings.push(OperationalBinding::new(head, BTreeMap::new()));
                for binding in bindings {
                    let q = query(run.graph(), &binding);
                    cases.push(
                        json!({"name":name,"prefix":prefix,"query":q,"answer":answer(q.clone())}),
                    );
                }
            }
            let Some(step) = run.enabled().unwrap().into_steps().into_iter().next() else {
                break;
            };
            run.commit(step).unwrap();
        }
    }
    println!(
        "{}",
        json!({"schema":"minimality-native-v1","snapshots":snapshots,"cases":cases})
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::{Binding, Kind, Sort};

    #[test]
    fn valid_executable_carriers_can_reverse_the_multiplicity_path_cost_order() {
        let text = include_str!("../../../examples/fixtures/a2.atoms");
        let mut graph = graph_core::fixture::load(text).unwrap().graph;
        for _ in 0..128 {
            graph
                .insert(Kind::variable(Sort::Constructor, Binding::Free), &[])
                .unwrap();
        }
        // Unowned variable atoms are core-valid data; the arithmetic still executes.
        assert_eq!((graph.len(), graph.field_count()), (159, 39));
        let variables = graph
            .iter()
            .filter(|(_, a)| matches!(a.kind, Kind::PatternVariable { .. }))
            .count();
        let first = graph.iter().filter(|(_, a)| !a.fields.is_empty()).count();
        let second = graph.iter().filter(|(_, a)| a.fields.len() == 2).count();
        assert_eq!((variables, first, second), (129, 21, 18));
        // H2 formulas with 21 records, 129 singleton admissions and no AppliedRule.
        let multiset = (19 + 159 + 39, 18 + 159 + 21 + 129 + 129 + 3 * 39);
        let positional = (15 + 159 + 129 + 18, 14 + 159 + 2 * 129 + 129 + 21 + 2 * 18);
        assert_eq!((multiset, positional), ((217, 573), (321, 617)));
        let mut run = Execution::new(graph, Default::default()).unwrap();
        let step = run
            .enabled()
            .unwrap()
            .into_steps()
            .into_iter()
            .next()
            .unwrap();
        run.commit(step).unwrap();
    }
}
