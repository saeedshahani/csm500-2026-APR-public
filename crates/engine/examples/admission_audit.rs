//! Native finite-family bridge. Expected bindings/effects are external to this executable.
#[path = "../tests/support/mod.rs"]
#[allow(dead_code)]
mod support;
use engine::trace::GraphData;
use engine::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{AtomKey, Graph, Role, Sort};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{self, BufRead};
use support::*;

fn maps(bindings: Vec<OperationalBinding>, templates: &[AtomKey]) -> Vec<Vec<u64>> {
    let mut rows: Vec<_> = bindings
        .iter()
        .map(|b| {
            templates
                .iter()
                .map(|p| b.assignments()[p].number())
                .collect()
        })
        .collect();
    rows.sort();
    rows
}
fn answer(input: Value) -> Value {
    let n = input["nodes"].as_u64().unwrap() as usize;
    assert!(n <= 4);
    let edges = input["edges"].as_array().unwrap();
    assert!(edges.len() <= 16);
    let schema = input["schema"].as_str().unwrap();
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let templates = match schema {
        "delete-node" | "replace-node" => {
            let x = variable(&mut g, Sort::Constructor);
            let rhs = if schema == "replace-node" {
                let c = ctor(&mut g, "New");
                let e = link(&mut g, "new", c, c);
                vec![c, e]
            } else {
                vec![]
            };
            sides(&mut g, h, &[x], &rhs);
            vec![x]
        }
        "delete-link" | "copy-marker" => {
            let e = variable(&mut g, Sort::Record(Role::ObjectLink));
            let rhs = if schema == "copy-marker" {
                let c = ctor(&mut g, "Mark");
                vec![e, c]
            } else {
                vec![]
            };
            sides(&mut g, h, &[e], &rhs);
            vec![e]
        }
        "reverse" | "reverse-loop" | "delete-pair" => {
            let x = variable(&mut g, Sort::Constructor);
            let y = if schema == "reverse-loop" {
                x
            } else {
                variable(&mut g, Sort::Constructor)
            };
            let e = link(&mut g, "edge", x, y);
            let mut lhs = vec![x];
            if y != x {
                lhs.push(y);
            }
            lhs.push(e);
            let rhs = if schema == "delete-pair" {
                vec![]
            } else {
                let r = link(&mut g, "reversed", y, x);
                let mut v = vec![x];
                if y != x {
                    v.push(y);
                }
                v.push(r);
                v
            };
            sides(&mut g, h, &lhs, &rhs);
            lhs
        }
        _ => panic!("unknown schema"),
    };
    let nodes: Vec<_> = (0..n).map(|_| ctor(&mut g, "N")).collect();
    let mut data = nodes.clone();
    for edge in edges {
        let a = edge[0].as_u64().unwrap() as usize;
        let b = edge[1].as_u64().unwrap() as usize;
        let label = if edge[2].as_u64().unwrap() == 0 {
            "edge"
        } else {
            "other"
        };
        data.push(link(&mut g, label, nodes[a], nodes[b]));
    }
    if input["exhaust_identity"] == true {
        let snapshot: Vec<_> = g
            .iter()
            .map(|(id, a)| {
                (
                    id.number(),
                    a.kind.clone(),
                    a.fields.iter().map(|x| x.number()).collect(),
                )
            })
            .collect();
        g = Graph::restore_identity_snapshot(&snapshot, u64::MAX, &Default::default()).unwrap();
    }
    let mut limits = ExecutionLimits::default();
    limits.search.max_work = 50_000_000;
    if let Some(v) = input["max_work"].as_u64() {
        limits.search.max_work = v as usize;
    }
    if let Some(v) = input["max_candidates"].as_u64() {
        limits.search.max_candidates = v as usize;
    }
    let run = Execution::new(g.clone(), limits).unwrap();
    let query = |indexed| {
        let result = if indexed {
            run.bindings_indexed(h)
        } else {
            run.bindings(h)
        };
        match result {
            Ok(b) => json!({"bindings":maps(b,&templates)}),
            Err(e) => {
                json!({"error":format!("{:?}",e.kind()),"limit":format!("{:?}",e.context().limit)})
            }
        }
    };
    let enabled = match run.enabled() {
        Ok(e) => {
            json!({"bindings":maps(e.steps().iter().map(|s|s.binding().clone()).collect(),&templates),"rejected":e.rejected().len()})
        }
        Err(e) => {
            json!({"error":format!("{:?}",e.kind()),"limit":format!("{:?}",e.context().limit)})
        }
    };
    let mut cases = Vec::new();
    // Supplied explicit target tuples test admission even when enumeration exhausts.
    let tuples: Vec<Vec<AtomKey>> = if let Some(tuples) = input["tuples"].as_array() {
        tuples
            .iter()
            .map(|t| {
                t.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| data[v.as_u64().unwrap() as usize])
                    .collect()
            })
            .collect()
    } else {
        run.bindings(h)
            .unwrap()
            .iter()
            .map(|b| templates.iter().map(|t| b.assignments()[t]).collect())
            .collect()
    };
    for targets in tuples {
        assert_eq!(targets.len(), templates.len());
        let mut execution = Execution::new(g.clone(), limits).unwrap();
        let assignments: BTreeMap<_, _> = templates
            .iter()
            .copied()
            .zip(targets.iter().copied())
            .collect();
        let before = GraphData::from_graph(execution.graph());
        let row = match execution.prepare(OperationalBinding::new(h, assignments)) {
            Ok(p) => {
                let e = execution.commit(p).unwrap();
                json!({"status":"Admitted","consumed":e.consumed.iter().map(|x|x.number()).collect::<Vec<_>>(),"preserved":e.preserved.iter().map(|x|x.number()).collect::<Vec<_>>(),"produced":e.produced.iter().map(|x|x.number()).collect::<Vec<_>>(),"after":GraphData::from_graph(execution.graph())})
            }
            Err(e) => {
                json!({"status":format!("{:?}",e.kind()),"limit":format!("{:?}",e.context().limit),"after":GraphData::from_graph(execution.graph())})
            }
        };
        if row["status"] != "Admitted" {
            assert_eq!(serde_json::to_value(before).unwrap(), row["after"]);
        }
        cases.push(
            json!({"targets":targets.iter().map(|x|x.number()).collect::<Vec<_>>(),"result":row}),
        );
    }
    json!({"input":input,"before":GraphData::from_graph(&g),"data":data.iter().map(|x|x.number()).collect::<Vec<_>>(),"reference":query(false),"indexed":query(true),"enabled":enabled,"cases":cases})
}
fn main() {
    for line in io::stdin().lock().lines() {
        println!("{}", answer(serde_json::from_str(&line.unwrap()).unwrap()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::Kind;
    #[test]
    fn budgets_are_inconclusive_and_identity_exhaustion_is_not_semantic_rejection() {
        for input in [
            json!({"nodes":1,"edges":[],"schema":"replace-node","tuples":[[0]],"max_work":0}),
            json!({"nodes":1,"edges":[],"schema":"replace-node","tuples":[[0]],"max_candidates":0}),
            json!({"nodes":1,"edges":[],"schema":"replace-node","tuples":[[0]],"exhaust_identity":true}),
        ] {
            let r = answer(input);
            assert_eq!(r["cases"][0]["result"]["status"], "ResourceExhaustion");
            assert_eq!(r["cases"][0]["result"]["after"], r["before"]);
        }
        let r = answer(json!({"nodes":1,"edges":[],"schema":"replace-node"}));
        assert_eq!(r["cases"][0]["result"]["status"], "Admitted");
    }
    #[test]
    fn raw_noninjective_and_wrong_sort_bindings_are_rejected_atomically() {
        let r = answer(
            json!({"nodes":2,"edges":[[0,1,0]],"schema":"reverse","tuples":[[0,0,2],[2,1,0]]}),
        );
        for c in r["cases"].as_array().unwrap() {
            assert_eq!(c["result"]["status"], "InvalidBinding");
            assert_eq!(c["result"]["after"], r["before"]);
        }
    }
    #[test]
    fn closure_does_not_establish_complete_configuration_validity() {
        let mut g = Graph::new();
        let n = g.insert(Kind::Nat, &[]).unwrap();
        let e = g
            .insert(
                Kind::Record {
                    role: Role::ObjectLink,
                    label: Some("bad".into()),
                },
                &[n, n],
            )
            .unwrap();
        assert_eq!(g.get(e).unwrap().fields.len(), 2);
        assert!(graph_core::validate(&g).is_err());
    }
}
