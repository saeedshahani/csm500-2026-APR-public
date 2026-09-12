//! Executable reference contract for S3.A, not the S3.E runtime observer.
use graph_core::{fixture, validate, AtomKey, Graph, Kind, Role};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct Spec {
    all: bool,
    data: bool,
    clauses: BTreeSet<(String, String)>,
}
fn decode_label(text: &str) -> Result<String, &'static str> {
    if text.len() % 2 != 0
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("noncanonical hex label");
    }
    String::from_utf8(
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect(),
    )
    .map_err(|_| "invalid UTF-8 label")
}
fn parse(text: &str) -> Result<Spec, &'static str> {
    if text == "observer-v1\nall\n" {
        return Ok(Spec {
            all: true,
            data: false,
            clauses: BTreeSet::new(),
        });
    }
    let mut lines = text
        .strip_suffix('\n')
        .ok_or("missing final newline")?
        .split('\n');
    if lines.next() != Some("observer-v1") {
        return Err("unsupported schema");
    }
    let data = match lines.next() {
        Some("select data") => true,
        Some("select carrier") => false,
        _ => return Err("invalid scope"),
    };
    let mut clauses = BTreeSet::new();
    for line in lines {
        let (key, value) = line.split_once(' ').ok_or("invalid clause")?;
        match key {
            "kind"
                if [
                    "Constructor",
                    "PatternVariable",
                    "RuleHead",
                    "Nat",
                    "Record",
                ]
                .contains(&value) => {}
            "role"
                if [
                    "ObjectLink",
                    "Lhs",
                    "Rhs",
                    "Active",
                    "Rank",
                    "ModeObject",
                    "ModeMeta",
                    "Revision",
                ]
                .contains(&value) => {}
            "constructor" | "link" => {
                decode_label(value)?;
            }
            "input"
                if !value.is_empty()
                    && value.bytes().enumerate().all(|(i, b)| {
                        b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                    }) => {}
            _ => return Err("unknown selector or value"),
        }
        if !clauses.insert((key.into(), value.into())) {
            return Err("duplicate clause");
        }
    }
    Ok(Spec {
        all: false,
        data,
        clauses,
    })
}
fn canonical(spec: &Spec) -> String {
    if spec.all {
        return "observer-v1\nall\n".into();
    }
    let mut out = format!(
        "observer-v1\nselect {}\n",
        if spec.data { "data" } else { "carrier" }
    );
    for (key, value) in &spec.clauses {
        out += &format!("{key} {value}\n");
    }
    out
}
fn observe(
    g: &Graph,
    spec: &Spec,
    inputs: &BTreeMap<String, AtomKey>,
) -> Result<BTreeSet<AtomKey>, &'static str> {
    let config = validate(g).map_err(|_| "invalid execution state")?;
    for (key, value) in &spec.clauses {
        if key == "input" && !inputs.contains_key(value) {
            return Err("unresolved designated input");
        }
    }
    let hidden: BTreeSet<_> = config
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    let mut seeds = BTreeSet::new();
    for (id, a) in g.iter() {
        if spec.all {
            seeds.insert(id);
            continue;
        }
        if spec.data && hidden.contains(&id) {
            continue;
        }
        let kind = match a.kind {
            Kind::Constructor(_) => "Constructor",
            Kind::PatternVariable { .. } => "PatternVariable",
            Kind::RuleHead => "RuleHead",
            Kind::Nat => "Nat",
            Kind::Record { .. } => "Record",
        };
        let selected = spec.clauses.iter().any(|(key,value)| match key.as_str() {
            "kind" => kind == value,
            "role" => a.kind.role().is_some_and(|r| format!("{r:?}") == *value),
            "constructor" => matches!(&a.kind, Kind::Constructor(label) if *label == decode_label(value).unwrap()),
            "link" => matches!(&a.kind, Kind::Record {role: Role::ObjectLink, label: Some(label)} if *label == decode_label(value).unwrap()),
            "input" => inputs.get(value) == Some(&id),
            _ => unreachable!(),
        });
        if selected {
            seeds.insert(id);
        }
    }
    g.closure(&seeds).map_err(|_| "invalid projection")
}

#[test]
fn observer_declarations_are_versioned_unambiguous_and_canonical() {
    let a = parse("observer-v1\nselect carrier\nrole Rank\nkind Constructor\n").unwrap();
    let b = parse("observer-v1\nselect carrier\nkind Constructor\nrole Rank\n").unwrap();
    assert_eq!(canonical(&a), canonical(&b));
    for invalid in [
        "observer-v2\nall\n",
        "observer-v1\nall\nkind Nat\n",
        "observer-v1\nselect data\nrole Unknown\n",
        "observer-v1\nselect data\nlink FF\n",
        "observer-v1\nselect data\nlink ff\n",
        "observer-v1\nselect data\nkind Nat\nkind Nat\n",
        "observer-v1\nselect data\ninput 1bad\n",
        "observer-v1\nselect data",
    ] {
        assert!(parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert_eq!(decode_label("76616c7565").unwrap(), "value");
    assert_eq!(decode_label("").unwrap(), "");
}

#[test]
fn rank_projection_expands_to_targets_without_becoming_an_executable_configuration() {
    let f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let spec = parse("observer-v1\nselect carrier\nrole Rank\n").unwrap();
    let observed = observe(&f.graph, &spec, &BTreeMap::new()).unwrap();
    // Carrier scope also selects quoted Rank templates, not just the live rank.
    let expected = [
        "sl",
        "sl_r0",
        "sl_r1",
        "sl_rank",
        "b_self",
        "b_rest",
        "b_top",
        "b_old_rank",
        "b_new_rank",
    ]
    .into_iter()
    .map(|name| f.names[name])
    .collect();
    assert_eq!(observed, expected);
    assert!(observed.contains(&f.names["sl"]));
    assert!(!observed
        .iter()
        .any(|id| f.graph.get(*id).unwrap().kind.role() == Some(Role::Revision)));
    // The observed head lacks execution metadata, intentionally. It is only a carrier view.
}

#[test]
fn all_retains_unattached_atoms_and_designated_inputs_never_rebind_to_replacements() {
    let mut g = Graph::new();
    let a = g.insert(Kind::Constructor("value".into()), &[]).unwrap();
    let inputs = BTreeMap::from([("root".into(), a)]);
    let spec = parse("observer-v1\nselect carrier\ninput root\n").unwrap();
    assert_eq!(observe(&g, &spec, &inputs).unwrap(), BTreeSet::from([a]));
    assert!(observe(&g, &spec, &BTreeMap::new()).is_err());
    g.remove_set(&BTreeSet::from([a])).unwrap();
    let b = g.insert(Kind::Constructor("value".into()), &[]).unwrap();
    assert_ne!(a, b);
    assert!(observe(&g, &spec, &inputs).unwrap().is_empty());
    assert_eq!(
        observe(&g, &parse("observer-v1\nall\n").unwrap(), &inputs).unwrap(),
        BTreeSet::from([b])
    );
}

#[test]
fn data_scope_excludes_templates_before_closure_and_empty_selection_is_explicit() {
    let mut f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let stray = f
        .graph
        .insert(Kind::Constructor("stray".into()), &[])
        .unwrap();
    let spec = parse("observer-v1\nselect data\nkind Nat\nkind Constructor\n").unwrap();
    assert_eq!(
        observe(&f.graph, &spec, &BTreeMap::new()).unwrap(),
        BTreeSet::from([stray])
    );
    assert!(observe(
        &f.graph,
        &parse("observer-v1\nselect carrier\n").unwrap(),
        &BTreeMap::new()
    )
    .unwrap()
    .is_empty());
}

#[test]
fn nontriviality_requires_a_relation_not_nonempty_litter_or_parallel_inflation() {
    let mut g = Graph::new();
    let a = g.insert(Kind::Constructor("A".into()), &[]).unwrap();
    let b = g.insert(Kind::Constructor("B".into()), &[]).unwrap();
    let spec = parse("observer-v1\nselect data\nlink 76616c7565\n").unwrap();
    let measure = |g: &Graph| -> usize {
        let observed = observe(g, &spec, &BTreeMap::new()).unwrap();
        let mut vertices: BTreeSet<AtomKey> = BTreeSet::new();
        for id in observed {
            let atom = g.get(id).unwrap();
            if atom.kind.role() == Some(Role::ObjectLink) && atom.fields[0] != atom.fields[1] {
                vertices.extend(atom.fields.iter().copied());
            }
        }
        vertices.len()
    };
    assert_eq!(measure(&g), 0);
    let kind = Kind::Record {
        role: Role::ObjectLink,
        label: Some("value".into()),
    };
    let self_link = g.insert(kind.clone(), &[a, a]).unwrap();
    assert_eq!(measure(&g), 0);
    let first = g.insert(kind.clone(), &[a, b]).unwrap();
    let second = g.insert(kind, &[a, b]).unwrap();
    assert_eq!(measure(&g), 2);
    g.remove_set(&BTreeSet::from([self_link, first, second]))
        .unwrap();
    assert!(!g.is_empty());
    assert_eq!(measure(&g), 0);
}
