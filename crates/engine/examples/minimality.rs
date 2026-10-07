//! Research bridge: bounded native corpus and explicit candidate admission.
//! The universal encodings themselves are independently implemented in JavaScript.
use engine::recording::Footprint;
use engine::trace::GraphData;
use engine::{Execution, OperationalBinding};
use graph_core::{validate, Graph};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead};

fn authored_graph(name: &str, text: &str) -> Result<Graph, String> {
    match std::path::Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
    {
        Some("graph") => parser::load(text)
            .map(|loaded| loaded.into_graph())
            .map_err(|e| format!("{e:?}")),
        Some("lambda") => parser::lambda::parse(text, Default::default())
            .and_then(|term| parser::lambda::encode(&term, Default::default()))
            .map(|encoded| encoded.graph)
            .map_err(|e| format!("{e:?}")),
        Some("json") if name == "reflection/versioned.json" => {
            let request = serde_json::from_str(text).map_err(|e| format!("{e}"))?;
            engine::versioned::compile(&request).map_err(|e| format!("{e:?}"))
        }
        _ => Err(format!("unsupported authored format: {name}")),
    }
}

fn check_generated(graph: &Graph, stored: &str) -> Result<(), String> {
    let stored = parser::load(stored)
        .map_err(|e| format!("{e:?}"))?
        .into_graph();
    let print = |graph| parser::print(graph).map_err(|e| format!("{e:?}"));
    if print(graph)? != print(&stored)? {
        return Err("compiler/generated-source disagreement".into());
    }
    Ok(())
}

fn selected_paths(manifest: &Value, requested: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let known: BTreeSet<String> = manifest
        .as_array()
        .ok_or("catalogue must be an array")?
        .iter()
        .map(|p| {
            p["authored"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "missing authored path".to_owned())
        })
        .collect::<Result<_, _>>()?;
    let Some(requested) = requested else {
        return Ok(known.into_iter().collect());
    };
    let chosen: BTreeSet<_> = requested.iter().cloned().collect();
    if chosen.is_empty() || chosen.len() != requested.len() || !chosen.is_subset(&known) {
        return Err("requested corpus must contain distinct existing authored paths".into());
    }
    Ok(chosen.into_iter().collect())
}

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
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = args.first().ok_or("corpus [--paths]|admit")?;
    if mode == "admit" {
        if args.len() != 1 {
            return Err("admit accepts no options".into());
        }
        for line in io::stdin().lock().lines() {
            println!("{}", answer(serde_json::from_str(&line.unwrap()).unwrap()));
        }
        return Ok(());
    }
    if mode != "corpus" || !(args.len() == 1 || args == ["corpus", "--paths"]) {
        return Err(
            "usage: corpus [--paths]|admit; --paths reads a JSON path array on stdin".into(),
        );
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
            .unwrap();
    let requested = if args.len() == 2 {
        Some(
            serde_json::from_reader(io::stdin())
                .map_err(|e| format!("invalid corpus selection: {e}"))?,
        )
    } else {
        None
    };
    let paths = selected_paths(&manifest, requested)?;
    let mut graphs = Vec::new();
    for name in ["a2", "a4", "b2", "c1"] {
        let f = graph_core::fixture::load(
            &std::fs::read_to_string(root.join(format!("fixtures/{name}.atoms"))).unwrap(),
        )
        .unwrap();
        graphs.push((format!("fixture/{name}"), f.graph));
    }
    for name in paths {
        let text = std::fs::read_to_string(root.join(&name)).map_err(|e| format!("{name}: {e}"))?;
        let g = authored_graph(&name, &text).map_err(|e| format!("{name}: {e}"))?;
        if !name.ends_with(".graph") {
            let entry = manifest
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["authored"] == name)
                .unwrap();
            let generated = entry["file"]
                .as_str()
                .ok_or("missing generated graph path")?;
            let stored = std::fs::read_to_string(root.join(generated))
                .map_err(|e| format!("{generated}: {e}"))?;
            check_generated(&g, &stored).map_err(|e| format!("{e}: {name} -> {generated}"))?;
        }
        graphs.push((name, g));
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::{Binding, Kind, Sort};

    #[test]
    fn authored_formats_and_frozen_selection_are_explicit() {
        let manifest = json!([{"authored":"a.graph"},{"authored":"reflection/versioned.json"}]);
        assert_eq!(
            selected_paths(&manifest, Some(vec!["a.graph".into()])).unwrap(),
            ["a.graph"]
        );
        assert_eq!(selected_paths(&manifest, None).unwrap().len(), 2);
        for paths in [
            vec![],
            vec!["missing.graph".into()],
            vec!["a.graph".into(), "a.graph".into()],
        ] {
            assert!(selected_paths(&manifest, Some(paths)).is_err());
        }
        assert!(authored_graph("unknown.txt", "program {}")
            .unwrap_err()
            .contains("unsupported"));
        assert!(authored_graph("unknown.json", "{}")
            .unwrap_err()
            .contains("unsupported"));
        assert!(authored_graph("reflection/versioned.json", "{}").is_err());
        let request = include_str!("../../../examples/reflection/versioned.json");
        let graph = authored_graph("reflection/versioned.json", request).unwrap();
        validate(&graph).unwrap();
        check_generated(
            &graph,
            include_str!("../../../examples/reflection/versioned.graph"),
        )
        .unwrap();
        assert!(check_generated(&graph, "program {}")
            .unwrap_err()
            .contains("disagreement"));
        assert!(check_generated(&graph, "invalid graph").is_err());
    }

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
