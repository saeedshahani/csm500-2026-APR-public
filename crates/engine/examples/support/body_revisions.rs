//! Authored examples and graph-copy helpers for the Loop D experiments.
use engine::stability::{Contract, Requirement};
use graph_core::{AtomKey, Binding, Graph, Kind, NewAtom, Role, Sort, Target};
use std::collections::{BTreeMap, BTreeSet};

pub fn output() -> (Graph, Contract) {
    (
        parser::load(include_str!(
            "../../../../examples/practical/revise-output.graph"
        ))
        .unwrap()
        .into_graph(),
        serde_json::from_str(include_str!(
            "../../../../examples/practical/revise-output.contract.json"
        ))
        .unwrap(),
    )
}
pub fn guard(n: usize) -> (Graph, Contract) {
    assert!(n > 0 && n <= 12);
    let source = include_str!("../../../../examples/practical/revise-guard.graph");
    let prefix = source.split(" atom a =").next().unwrap();
    let mut source = prefix.to_owned();
    let mut requirements = Vec::new();
    for i in 0..n {
        let name = format!("Work:{i}");
        source.push_str(&format!("atom item_{i} = Constructor(\"{name}\"); atom status_{i} = Record[ObjectLink](\"pending\", item_{i}, item_{i});\n"));
        requirements.push(Requirement {
            from: name.clone(),
            targets: BTreeSet::from([name]),
            labels: BTreeSet::from(["pending".into(), "done".into()]),
            legitimate_labels: BTreeSet::from(["done".into()]),
        });
    }
    source.push_str("}\n");
    (
        parser::load(&source).unwrap().into_graph(),
        Contract {
            schema: "recovery-contract-v1".into(),
            preserve: BTreeSet::new(),
            requirements,
            stable_when_legitimate: true,
        },
    )
}
fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> AtomKey {
    g.insert(Kind::record(role), fields).unwrap()
}
fn ctor(g: &mut Graph, name: &str) -> AtomKey {
    g.insert(Kind::Constructor(name.into()), &[]).unwrap()
}
fn link(g: &mut Graph, label: &str, a: AtomKey) -> AtomKey {
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(label.into()),
        },
        &[a, a],
    )
    .unwrap()
}

/// A body-editing cycle guarded by an ordinary record that is produced only
/// after useful processing. Consuming/recreating that gate excludes quoted
/// gate patterns by the ordinary dangling condition, without a new primitive.
pub fn modulating(gated: bool) -> (Graph, Contract) {
    let loaded = parser::load(include_str!(
        "../../../../examples/practical/revise-output.graph"
    ))
    .unwrap();
    let process = loaded.name("process").unwrap();
    let upgrade = loaded.name("upgrade").unwrap();
    let mut g = loaded.into_graph();
    let (_, contract) = output();
    let down = g.insert(Kind::RuleHead, &[]).unwrap();
    let z = g.insert(Kind::Nat, &[]).unwrap();
    let one = g.insert(Kind::Nat, &[z]).unwrap();
    record(&mut g, Role::Rank, &[down, one]);
    record(&mut g, Role::ModeMeta, &[down]);
    record(&mut g, Role::Revision, &[down]);
    record(&mut g, Role::Active, &[down]);
    let h = g
        .insert(Kind::variable(Sort::RuleHead, Binding::Free), &[])
        .unwrap();
    let mode = record(&mut g, Role::ModeObject, &[h]);
    let old_rev = record(&mut g, Role::Revision, &[h]);
    let new_rev = record(&mut g, Role::Revision, &[h]);
    let item = ctor(&mut g, "Ticket");
    let old = link(&mut g, "audited", item);
    let new = link(&mut g, "done", item);
    let om = record(&mut g, Role::Rhs, &[h, old]);
    let nm = record(&mut g, Role::Rhs, &[h, new]);
    for k in [h, mode, old_rev, item, old, om] {
        record(&mut g, Role::Lhs, &[down, k]);
    }
    for k in [h, mode, new_rev, item, new, nm] {
        record(&mut g, Role::Rhs, &[down, k]);
    }
    if gated {
        let marker = ctor(&mut g, "Finished");
        let gate = link(&mut g, "revision-gate", marker);
        record(&mut g, Role::Rhs, &[process, marker]);
        record(&mut g, Role::Rhs, &[process, gate]);
        for head in [upgrade, down] {
            let marker = ctor(&mut g, "Finished");
            let gate = link(&mut g, "revision-gate", marker);
            let after = link(&mut g, "revision-gate", marker);
            for k in [marker, gate] {
                record(&mut g, Role::Lhs, &[head, k]);
            }
            for k in [marker, after] {
                record(&mut g, Role::Rhs, &[head, k]);
            }
        }
    }
    (g, contract)
}

/// Disjoint-copy a whole package with labels renamed, retaining cycles and
/// quotation. This is a construction operation, not a reflective event.
pub fn append(target: &mut Graph, source: &Graph, prefix: &str) {
    let positions: BTreeMap<_, _> = source
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (k, i))
        .collect();
    let batch: Vec<_> = source
        .iter()
        .map(|(_, a)| {
            let kind = match &a.kind {
                Kind::Constructor(s) => Kind::Constructor(format!("{prefix}{s}")),
                Kind::Record {
                    role,
                    label: Some(s),
                } => Kind::Record {
                    role: *role,
                    label: Some(format!("{prefix}{s}")),
                },
                k => k.clone(),
            };
            NewAtom {
                kind,
                fields: a
                    .fields
                    .iter()
                    .map(|k| Target::Fresh(positions[k]))
                    .collect(),
            }
        })
        .collect();
    target.insert_batch(&batch).unwrap();
}
pub fn prefixed(contract: &Contract, prefix: &str) -> Contract {
    Contract {
        schema: contract.schema.clone(),
        preserve: contract
            .preserve
            .iter()
            .map(|s| format!("{prefix}{s}"))
            .collect(),
        stable_when_legitimate: contract.stable_when_legitimate,
        requirements: contract
            .requirements
            .iter()
            .map(|r| Requirement {
                from: format!("{prefix}{}", r.from),
                targets: r.targets.iter().map(|s| format!("{prefix}{s}")).collect(),
                labels: r.labels.iter().map(|s| format!("{prefix}{s}")).collect(),
                legitimate_labels: r
                    .legitimate_labels
                    .iter()
                    .map(|s| format!("{prefix}{s}"))
                    .collect(),
            })
            .collect(),
    }
}
