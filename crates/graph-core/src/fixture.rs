//! Small explicit atom-ledger reader for tests; not the planned surface language.
use crate::{
    ensure, AtomKey, Binding, Diagnostic, Graph, Kind, NewAtom, Result, Role, Sort, Target,
};
use std::collections::{BTreeMap, BTreeSet};

pub struct Fixture {
    pub graph: Graph,
    pub names: BTreeMap<String, AtomKey>,
}

pub fn load(text: &str) -> Result<Fixture> {
    let rows: Vec<Vec<&str>> = text
        .lines()
        .map(|l| {
            l.split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect()
        })
        .filter(|r: &Vec<_>| !r.is_empty())
        .collect();
    let mut indices = BTreeMap::new();
    for (i, row) in rows.iter().enumerate() {
        ensure(row.len() >= 2, "expected name and kind")?;
        ensure(indices.insert(row[0], i).is_none(), "duplicate atom name")?;
    }
    let mut batch = Vec::new();
    for row in &rows {
        let mut fields = BTreeMap::new();
        let mut attrs = BTreeMap::new();
        for item in &row[2..] {
            let (key, value) = item
                .split_once('=')
                .ok_or_else(|| Diagnostic("expected key=value".into()))?;
            if let Some(slot) = key.strip_prefix("slot").or_else(|| key.strip_prefix('s')) {
                let slot: usize = slot
                    .parse()
                    .map_err(|_| Diagnostic("invalid slot".into()))?;
                let index = indices
                    .get(value)
                    .ok_or_else(|| Diagnostic(format!("unknown target {value}")))?;
                ensure(
                    fields.insert(slot, Target::Fresh(*index)).is_none(),
                    "duplicate slot",
                )?;
            } else {
                ensure(attrs.insert(key, value).is_none(), "duplicate attribute")?;
            }
        }
        ensure(
            fields.keys().copied().eq(0..fields.len()),
            "non-contiguous slots",
        )?;
        let kind = match row[1] {
            "RuleHead" => Kind::RuleHead,
            "Nat" => Kind::Nat,
            "PatternVariable" => {
                let admits = attrs
                    .remove("admits")
                    .ok_or_else(|| Diagnostic("missing admits".into()))?;
                let admits = admits
                    .strip_prefix('{')
                    .and_then(|s| s.strip_suffix('}'))
                    .ok_or_else(|| Diagnostic("expected admits={sort,...}".into()))?;
                let admits = admits
                    .split(',')
                    .map(sort)
                    .collect::<Result<BTreeSet<_>>>()?;
                let binding = match attrs.remove("binding") {
                    Some("Free") => Binding::Free,
                    Some("AppliedRule") => Binding::AppliedRule,
                    _ => return Err(Diagnostic("invalid binding".into())),
                };
                Kind::PatternVariable { admits, binding }
            }
            token if token.starts_with("Constructor[") && token.ends_with(']') => {
                Kind::Constructor(token[12..token.len() - 1].into())
            }
            token if token.starts_with("Record[") && token.ends_with(']') => {
                let (role_name, label) = token[7..token.len() - 1]
                    .split_once(',')
                    .map_or((&token[7..token.len() - 1], None), |(r, l)| {
                        (r, Some(l.to_string()))
                    });
                Kind::Record {
                    role: role(role_name)?,
                    label,
                }
            }
            _ => return Err(Diagnostic(format!("unknown kind {}", row[1]))),
        };
        ensure(attrs.is_empty(), "unknown attribute")?;
        batch.push(NewAtom {
            kind,
            fields: fields.into_values().collect(),
        });
    }
    let mut graph = Graph::new();
    let ids = graph.insert_batch(&batch)?;
    let names = indices
        .into_iter()
        .map(|(name, i)| (name.to_string(), ids[i]))
        .collect();
    Ok(Fixture { graph, names })
}

fn role(s: &str) -> Result<Role> {
    match s {
        "ObjectLink" => Ok(Role::ObjectLink),
        "Lhs" => Ok(Role::Lhs),
        "Rhs" => Ok(Role::Rhs),
        "Active" => Ok(Role::Active),
        "Rank" => Ok(Role::Rank),
        "ModeObject" => Ok(Role::ModeObject),
        "ModeMeta" => Ok(Role::ModeMeta),
        "Revision" => Ok(Role::Revision),
        _ => Err(Diagnostic(format!("unknown role {s}"))),
    }
}
fn sort(s: &str) -> Result<Sort> {
    match s {
        "Constructor" => Ok(Sort::Constructor),
        "PatternVariable" => Ok(Sort::PatternVariable),
        "RuleHead" => Ok(Sort::RuleHead),
        "Nat" => Ok(Sort::Nat),
        _ => s
            .strip_prefix("Record[")
            .and_then(|s| s.strip_suffix(']'))
            .map(|s| role(s).map(Sort::Record))
            .unwrap_or_else(|| Err(Diagnostic(format!("unknown sort {s}")))),
    }
}
