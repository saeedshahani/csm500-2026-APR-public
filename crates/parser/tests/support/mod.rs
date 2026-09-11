//! Independent, exhaustive small-carrier oracle. Not a production observer.
use graph_core::{Graph, NewAtom, Target};
use std::collections::BTreeMap;

pub fn reversed(graph: &Graph) -> Graph {
    let mut atoms: Vec<_> = graph.iter().collect();
    atoms.reverse();
    rebuild(atoms)
}

fn rebuild(atoms: Vec<(graph_core::AtomKey, &graph_core::Atom)>) -> Graph {
    let indices: BTreeMap<_, _> = atoms
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (*k, i))
        .collect();
    let batch: Vec<_> = atoms
        .iter()
        .map(|(_, a)| NewAtom {
            kind: a.kind.clone(),
            fields: a.fields.iter().map(|k| Target::Fresh(indices[k])).collect(),
        })
        .collect();
    let mut result = Graph::new();
    result.insert_batch(&batch).unwrap();
    result
}

/// Enumerate injective kind/attribute/arity-compatible bijections and test every
/// positional field. Return an error, never false, if the search is unfinished.
/// Limits count candidate assignments and bound stack/matrix memory for tests.
pub fn isomorphic(left: &Graph, right: &Graph, budget: usize) -> Result<bool, &'static str> {
    if left.len() != right.len() || left.field_count() != right.field_count() {
        return Ok(false);
    }
    let n = left.len();
    if n > 256 {
        return Err("isomorphism test atom limit");
    }
    let l: Vec<_> = left.iter().collect();
    let r: Vec<_> = right.iter().collect();
    let li: BTreeMap<_, _> = l.iter().enumerate().map(|(i, (k, _))| (*k, i)).collect();
    let ri: BTreeMap<_, _> = r.iter().enumerate().map(|(i, (k, _))| (*k, i)).collect();
    let lf: Vec<Vec<_>> = l
        .iter()
        .map(|(_, a)| a.fields.iter().map(|k| li[k]).collect())
        .collect();
    let rf: Vec<Vec<_>> = r
        .iter()
        .map(|(_, a)| a.fields.iter().map(|k| ri[k]).collect())
        .collect();
    // Joint colour refinement is only a necessary-condition filter. Exhaustive
    // bijection search below, not colour equality, decides isomorphism.
    let all: Vec<_> = l.iter().chain(&r).collect();
    let mut colours: Vec<_> = all
        .iter()
        .map(|(_, a)| all.iter().position(|(_, b)| a.kind == b.kind).unwrap())
        .collect();
    let fields: Vec<Vec<_>> = lf
        .iter()
        .cloned()
        .chain(rf.iter().map(|v| v.iter().map(|i| i + n).collect()))
        .collect();
    loop {
        let mut classes = BTreeMap::new();
        let next_colours: Vec<_> = (0..2 * n)
            .map(|i| {
                let outgoing: Vec<_> = fields[i].iter().map(|j| colours[*j]).collect();
                let mut incoming = Vec::new();
                for (j, targets) in fields.iter().enumerate() {
                    for (slot, target) in targets.iter().enumerate() {
                        if *target == i {
                            incoming.push((slot, colours[j]));
                        }
                    }
                }
                incoming.sort();
                let next = classes.len();
                *classes
                    .entry((colours[i], outgoing, incoming))
                    .or_insert(next)
            })
            .collect();
        if next_colours == colours {
            break;
        }
        colours = next_colours;
    }
    let candidates: Vec<Vec<_>> = l
        .iter()
        .enumerate()
        .map(|(i, (_, a))| {
            r.iter()
                .enumerate()
                .filter(|(j, (_, b))| {
                    a.kind == b.kind
                        && a.fields.len() == b.fields.len()
                        && colours[i] == colours[n + *j]
                })
                .map(|(j, _)| j)
                .collect()
        })
        .collect();
    if candidates.iter().any(Vec::is_empty) {
        return Ok(false);
    }
    let mut order: Vec<_> = (0..n).collect();
    order.sort_by_key(|i| (candidates[*i].len(), *i));
    let mut mapping = vec![None; n];
    let mut used = vec![false; n];
    let mut next = vec![0; n];
    let mut depth = 0;
    let mut attempts = 0;
    loop {
        if depth == n {
            return Ok(true);
        }
        let i = order[depth];
        if let Some(j) = mapping[i].take() {
            used[j] = false;
        }
        let mut advance = false;
        while next[depth] < candidates[i].len() {
            if attempts == budget {
                return Err("isomorphism test search exhausted");
            }
            attempts += 1;
            let j = candidates[i][next[depth]];
            next[depth] += 1;
            if used[j] {
                continue;
            }
            mapping[i] = Some(j);
            let consistent = (0..n).all(|a| {
                mapping[a].is_none_or(|b| {
                    lf[a]
                        .iter()
                        .zip(&rf[b])
                        .all(|(x, y)| mapping[*x].is_none_or(|z| z == *y))
                })
            });
            if consistent {
                used[j] = true;
                advance = true;
                break;
            }
            mapping[i] = None;
        }
        if advance {
            depth += 1;
        } else {
            next[depth] = 0;
            if depth == 0 {
                return Ok(false);
            }
            depth -= 1;
        }
    }
}
