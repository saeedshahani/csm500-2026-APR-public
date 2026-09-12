//! Shared test-only graph recipes. No engine or parser dependency. Shrinking
//! acts on finite recipes, then rebuilds all references and private metadata.
#![allow(dead_code)]
use graph_core::{fixture, AtomKey, Binding, Graph, Kind, NewAtom, Role, Sort, Target};
use proptest::prelude::*;
use proptest::test_runner::{
    Config, FileFailurePersistence, RngAlgorithm, TestCaseResult, TestRng, TestRunner,
};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

pub fn check<S: Strategy>(
    name: &str,
    path: &'static str,
    strategy: S,
    test: impl Fn(S::Value) -> TestCaseResult,
) {
    let cases = std::env::var("S3_CASES")
        .map(|v| v.parse::<u32>().expect("S3_CASES must be u32"))
        .unwrap_or(64);
    assert!((1..=4096).contains(&cases), "S3_CASES must be in 1..=4096");
    let seed = std::env::var("S3_SEED")
        .map(|v| v.parse::<u64>().expect("S3_SEED must be u64"))
        .unwrap_or(0x533346);
    let config = Config {
        cases,
        max_local_rejects: 0,
        max_global_rejects: 0,
        max_shrink_iters: 4096,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(path))),
        ..Config::default()
    };
    let mut bytes = [0; 32];
    for chunk in bytes.chunks_exact_mut(8) {
        chunk.copy_from_slice(&seed.to_le_bytes());
    }
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &bytes));
    let calls = Cell::new(0u32);
    runner
        .run(&strategy, |value| {
            calls.set(calls.get() + 1);
            test(value)
        })
        .unwrap();
    println!("{name}: requested={cases}; evaluated={}; rejected=0; seed={seed}; generator=proptest-1.6.0/ChaCha/repeated-le-u64; no filters",calls.get());
}

pub fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> AtomKey {
    g.insert(Kind::record(role), fields).unwrap()
}
pub fn ctor(g: &mut Graph, label: &str) -> AtomKey {
    g.insert(Kind::Constructor(label.into()), &[]).unwrap()
}
pub fn link(g: &mut Graph, label: &str, a: AtomKey, b: AtomKey) -> AtomKey {
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(label.into()),
        },
        &[a, b],
    )
    .unwrap()
}
pub fn variable(g: &mut Graph, sort: Sort) -> AtomKey {
    g.insert(Kind::variable(sort, Binding::Free), &[]).unwrap()
}
pub fn rule(g: &mut Graph, rank: usize, meta: bool, active: bool) -> AtomKey {
    let h = g.insert(Kind::RuleHead, &[]).unwrap();
    let mut n = g.insert(Kind::Nat, &[]).unwrap();
    for _ in 0..rank {
        n = g.insert(Kind::Nat, &[n]).unwrap();
    }
    record(g, Role::Rank, &[h, n]);
    record(g, Role::Revision, &[h]);
    record(
        g,
        if meta {
            Role::ModeMeta
        } else {
            Role::ModeObject
        },
        &[h],
    );
    if active {
        record(g, Role::Active, &[h]);
    }
    h
}
pub fn sides(g: &mut Graph, h: AtomKey, l: &[AtomKey], r: &[AtomKey]) {
    for a in l {
        record(g, Role::Lhs, &[h, *a]);
    }
    for a in r {
        record(g, Role::Rhs, &[h, *a]);
    }
}
pub fn owned(g: &Graph, h: AtomKey, role: Role) -> AtomKey {
    g.iter()
        .find(|(_, a)| a.kind.role() == Some(role) && a.fields[0] == h)
        .unwrap()
        .0
}
pub fn append(g: &mut Graph, fragment: &Graph) -> BTreeMap<AtomKey, AtomKey> {
    let atoms: Vec<_> = fragment.iter().collect();
    let positions: BTreeMap<_, _> = atoms
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (*k, i))
        .collect();
    let batch: Vec<_> = atoms
        .iter()
        .map(|(_, a)| NewAtom {
            kind: a.kind.clone(),
            fields: a
                .fields
                .iter()
                .map(|k| Target::Fresh(positions[k]))
                .collect(),
        })
        .collect();
    let keys = g.insert_batch(&batch).unwrap();
    atoms
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (*k, keys[i]))
        .collect()
}
#[derive(Clone, Debug)]
pub struct ObjectRecipe {
    pub nodes: usize,
    pub edges: Vec<(u8, u8)>,
    pub label: String,
}
pub fn objects() -> impl Strategy<Value = ObjectRecipe> {
    (
        2usize..6,
        proptest::collection::vec((any::<u8>(), any::<u8>()), 0..7),
        prop_oneof![
            Just("value".to_owned()),
            Just("λ\n\0".to_owned()),
            "[a-z]{0,4}"
        ],
    )
        .prop_map(|(nodes, edges, label)| ObjectRecipe {
            nodes,
            edges,
            label,
        })
}
pub fn object_graph(recipe: &ObjectRecipe, nonself: bool) -> Graph {
    let mut g = Graph::new();
    let vertices: Vec<_> = (0..recipe.nodes)
        .map(|i| ctor(&mut g, &format!("v{i}")))
        .collect();
    for (a, b) in &recipe.edges {
        let a = *a as usize % vertices.len();
        let b = if nonself {
            (a + 1 + *b as usize % (vertices.len() - 1)) % vertices.len()
        } else {
            *b as usize % vertices.len()
        };
        link(&mut g, &recipe.label, vertices[a], vertices[b]);
    }
    g
}
pub fn relabel(g: &mut Graph, from: &str, to: &str) -> AtomKey {
    let h = rule(g, 0, false, true);
    let x = variable(g, Sort::Constructor);
    let y = variable(g, Sort::Constructor);
    let l = link(g, from, x, y);
    let r = link(g, to, x, y);
    sides(g, h, &[x, y, l], &[x, y, r]);
    h
}
pub fn ln(recipe: &ObjectRecipe) -> Graph {
    let mut recipe = recipe.clone();
    recipe.label = "pending".into();
    let mut g = object_graph(&recipe, true);
    relabel(&mut g, "pending", "done");
    g
}
pub fn lowerer(rank: usize, active: bool, partial: bool) -> Graph {
    let mut f = fixture::load(include_str!("../../../../examples/fixtures/b2.atoms")).unwrap();
    let h = f.names["sl"];
    f.graph
        .remove_set(&BTreeSet::from([f.names["sl_rank"], f.names["sl_r1"]]))
        .unwrap();
    let mut top = f.names["sl_r0"];
    for _ in 0..rank {
        top = f.graph.insert(Kind::Nat, &[top]).unwrap();
    }
    record(&mut f.graph, Role::Rank, &[h, top]);
    if !active {
        f.graph
            .remove_set(&BTreeSet::from([f.names["sl_active"]]))
            .unwrap();
        if partial {
            f.graph
                .remove_set(&BTreeSet::from([f.names["sl_L_rest"]]))
                .unwrap();
        }
    }
    f.graph
}
pub fn nested(depth: usize, active: bool) -> Graph {
    let mut g = Graph::new();
    let h = rule(&mut g, depth, depth > 0, active);
    if depth > 0 {
        let child = nested(depth - 1, active);
        let keys: Vec<_> = append(&mut g, &child).values().copied().collect();
        sides(&mut g, h, &[], &keys);
    }
    g
}
/// Two mode promotions in one event; each target retains rank, receives a fresh
/// revision and becomes meta-visible. Altered target sets are known beforehand.
pub fn promote(
    rank: usize,
    violate_gc: bool,
    violate_revision: bool,
) -> (
    Graph,
    AtomKey,
    BTreeMap<AtomKey, AtomKey>,
    BTreeSet<AtomKey>,
) {
    let mut g = Graph::new();
    let first = rule(&mut g, rank, false, false);
    let second = rule(&mut g, rank + usize::from(violate_gc), false, true);
    let source = rule(&mut g, rank + 1, true, true);
    let mut lhs = Vec::new();
    let mut rhs = Vec::new();
    let mut binding = BTreeMap::new();
    for target in [first, second] {
        let h = variable(&mut g, Sort::RuleHead);
        let old = record(&mut g, Role::ModeObject, &[h]);
        let new = record(&mut g, Role::ModeMeta, &[h]);
        let rev = record(&mut g, Role::Revision, &[h]);
        let fresh = if target == second && violate_revision {
            rev
        } else {
            record(&mut g, Role::Revision, &[h])
        };
        lhs.extend([h, old, rev]);
        rhs.extend([h, new, fresh]);
        binding.extend([
            (h, target),
            (old, owned(&g, target, Role::ModeObject)),
            (rev, owned(&g, target, Role::Revision)),
        ]);
    }
    sides(&mut g, source, &lhs, &rhs);
    (g, source, binding, BTreeSet::from([first, second]))
}
