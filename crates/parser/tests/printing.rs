mod support;
use graph_core::{fixture, Binding, Graph, Kind, NewAtom, Role, Sort, Target};
use parser::{load, print, print_with_limits, ParseLimits};
use std::collections::BTreeSet;
use support::{isomorphic, reversed};

fn round_trip(graph: &Graph) {
    let text = print(graph).unwrap();
    let loaded = load(&text).unwrap();
    assert_eq!(isomorphic(graph, loaded.graph(), 2_000_000), Ok(true));
    let permuted = reversed(graph);
    assert_eq!(isomorphic(graph, &permuted, 2_000_000), Ok(true));
    assert_eq!(isomorphic(&permuted, loaded.graph(), 2_000_000), Ok(true));
    assert_eq!(print(loaded.graph()).unwrap(), text);
}

#[test]
fn counted_ledgers_round_trip_as_complete_carriers() {
    for text in [
        include_str!("../../../examples/fixtures/a2.atoms"),
        include_str!("../../../examples/fixtures/a4.atoms"),
        include_str!("../../../examples/fixtures/b2.atoms"),
        include_str!("../../../examples/fixtures/c1.atoms"),
    ] {
        round_trip(&fixture::load(text).unwrap().graph);
    }
}

#[test]
fn authored_packages_and_membership_modification_round_trip() {
    for text in [
        include_str!("../../../examples/syntax/install-wrap.graph"),
        include_str!("../../../examples/syntax/modify-output.graph"),
    ] {
        round_trip(load(text).unwrap().graph());
    }
}

#[test]
fn readable_snapshot_preserves_parallel_links_unattached_atoms_and_cycles() {
    let expected = include_str!("../../../examples/syntax/objects.expected.graph");
    let graph = load(expected).unwrap();
    assert_eq!(print(graph.graph()).unwrap(), expected);
    round_trip(graph.graph());
    let inactive = include_str!("../../../examples/syntax/inactive.expected.graph");
    assert_eq!(print(load(inactive).unwrap().graph()).unwrap(), inactive);
}

// Compact builders transcribe the remaining worked-example schemas. They build
// authored text, not a second interpreter, and round trips make no execution claim.
fn schema(
    name: &str,
    rank: usize,
    mode: &str,
    atoms: &[(&str, &str)],
    lhs: &[&str],
    rhs: &[&str],
) -> String {
    let mut text = format!("rule {name} {{ atom {name}_rank0 = Nat();");
    for i in 1..=rank {
        text += &format!("atom {name}_rank{i} = Nat({name}_rank{});", i - 1);
    }
    text += &format!("rank {name}_rank = {name}_rank{rank}; mode {name}_mode = {mode}; revision {name}_rev; active {name}_active;");
    // Each example uses local names in kind expressions; prefix lexer identifiers
    // that are declared atoms, leaving kind names and string labels untouched.
    for (local, kind) in atoms {
        let mut resolved = kind.to_string();
        let tokens = parser::lex(kind);
        for token in tokens
            .tokens
            .iter()
            .rev()
            .filter(|t| t.kind == parser::TokenKind::Identifier)
        {
            let ident = token.value.as_ref().unwrap();
            if atoms.iter().any(|(name, _)| name == ident) {
                resolved
                    .replace_range(token.span.start..token.span.end, &format!("{name}_{ident}"));
            }
        }
        text += &format!("atom {name}_{local} = {resolved};");
    }
    for (side, items) in [("lhs", lhs), ("rhs", rhs)] {
        text += &format!("{side} {{");
        for (i, item) in items.iter().enumerate() {
            text += &format!("{name}_{side}{i}: {name}_{item};");
        }
        text += "}";
    }
    text + "}"
}

#[test]
fn remaining_worked_example_families_round_trip() {
    let promote = schema(
        "promote",
        2,
        "ModeMeta",
        &[
            ("h", "PatternVariable[RuleHead] Free"),
            ("z", "Nat()"),
            ("old", "Record[Rank](h,z)"),
            ("v", "Record[Revision](h)"),
            ("one", "Nat(z)"),
            ("new", "Record[Rank](h,one)"),
            ("nv", "Record[Revision](h)"),
        ],
        &["h", "z", "old", "v"],
        &["h", "z", "one", "new", "nv"],
    );
    let tick = schema(
        "tick",
        0,
        "ModeObject",
        &[
            ("c", "Constructor(\"Counter\")"),
            ("n", "PatternVariable[Constructor] Free"),
            ("old", "Record[ObjectLink](\"val\",c,n)"),
            ("s", "Constructor(\"Succ\")"),
            ("now", "Record[ObjectLink](\"val\",c,s)"),
            ("pred", "Record[ObjectLink](\"pred\",s,n)"),
        ],
        &["c", "n", "old"],
        &["c", "n", "s", "now", "pred"],
    );
    let mode = schema(
        "mode_change",
        2,
        "ModeMeta",
        &[
            ("h", "PatternVariable[RuleHead] Free"),
            ("old", "Record[ModeObject](h)"),
            ("v", "Record[Revision](h)"),
            ("new", "Record[ModeMeta](h)"),
            ("nv", "Record[Revision](h)"),
        ],
        &["h", "old", "v"],
        &["h", "new", "nv"],
    );
    let annotate = schema(
        "annotate",
        0,
        "ModeMeta",
        &[
            ("h", "PatternVariable[RuleHead] Free"),
            ("m", "Record[ModeObject](h)"),
            ("v", "Record[Revision](h)"),
            ("note", "Constructor(\"Audited\")"),
            ("add", "Record[Rhs](h,note)"),
            ("nv", "Record[Revision](h)"),
        ],
        &["h", "m", "v"],
        &["h", "m", "note", "add", "nv"],
    );
    let lower = |name: &str, note: &str, own: &str| {
        schema(
            name,
            1,
            "ModeMeta",
            &[
                ("h", "PatternVariable[RuleHead] Free"),
                ("note", note),
                ("nl", "Record[Lhs](h,note)"),
                ("nr", "Record[Rhs](h,note)"),
                ("z", "Nat()"),
                ("one", "Nat(z)"),
                ("old", "Record[Rank](h,one)"),
                ("v", "Record[Revision](h)"),
                ("own", own),
                ("low", "Record[Rank](h,z)"),
                ("fresh", note),
                ("fl", "Record[Lhs](h,fresh)"),
                ("fr", "Record[Rhs](h,fresh)"),
                ("nv", "Record[Revision](h)"),
            ],
            &["h", "note", "nl", "nr", "z", "one", "old", "v", "own"],
            &["h", "z", "own", "low", "fresh", "fl", "fr", "nv"],
        )
    };
    let p = lower("p", "Constructor(\"NoteByP\")", "Constructor(\"NoteByQ\")");
    let q = lower("q", "Constructor(\"NoteByQ\")", "Constructor(\"NoteByP\")");
    // A1, A5, B1, B3, C2 and C3. A2/A3, A4/C4, B2 and C1 use counted ledgers above.
    for body in ["atom z = Constructor(\"Zero\"); atom s1 = Constructor(\"Succ\"); atom s2 = Constructor(\"Succ\"); atom p1 = Record[ObjectLink](\"pred\",s1,z); atom p2 = Record[ObjectLink](\"pred\",s2,s1);".to_string(), tick, mode, annotate, promote.clone(), format!("{p} {q} {promote}")] {
        round_trip(load(&format!("program {{ {body} }}")).unwrap().graph());
    }
}

#[test]
fn empty_inactive_shared_and_nested_states_round_trip() {
    for source in ["program {}", "program { rule h { atom z = Nat(); rank r = z; mode m = ModeMeta; revision v; atom x = Constructor(\"X\"); lhs { l: x; } rhs { rr: x; } } }",
        "program { rule a { atom az = Nat(); rank ar = az; mode am = ModeMeta; revision av; rhs { package { rule b { atom bz = Nat(); rank br = bz; mode bm = ModeMeta; revision bv; rhs { package { rule c { atom cz = Nat(); rank cr = cz; mode cm = ModeObject; revision cv; } } } } } } } }"] {
        round_trip(load(source).unwrap().graph());
    }
}

#[test]
fn all_variable_sorts_bindings_and_escaped_labels_are_preserved() {
    let mut graph = Graph::new();
    let sorts = BTreeSet::from([
        Sort::Constructor,
        Sort::PatternVariable,
        Sort::RuleHead,
        Sort::Nat,
        Sort::Record(Role::ObjectLink),
        Sort::Record(Role::Lhs),
        Sort::Record(Role::Rhs),
        Sort::Record(Role::Active),
        Sort::Record(Role::Rank),
        Sort::Record(Role::ModeObject),
        Sort::Record(Role::ModeMeta),
        Sort::Record(Role::Revision),
    ]);
    graph
        .insert(
            Kind::PatternVariable {
                admits: sorts,
                binding: Binding::Free,
            },
            &[],
        )
        .unwrap();
    graph
        .insert(Kind::variable(Sort::RuleHead, Binding::AppliedRule), &[])
        .unwrap();
    let text = "\"\\\n\r\t\0\u{7f}λ😀 // /* program a0";
    let x = graph.insert(Kind::Constructor(text.into()), &[]).unwrap();
    graph
        .insert(
            Kind::Record {
                role: Role::ObjectLink,
                label: Some(text.into()),
            },
            &[x, x],
        )
        .unwrap();
    round_trip(&graph);
}

#[test]
fn comments_names_and_formatting_are_not_serialised() {
    let a = load("program { // comment\n atom original = Constructor(\"x\"); }").unwrap();
    assert_eq!(
        print(a.graph()).unwrap(),
        "program {\n  atom a0 = Constructor(\"x\");\n}\n"
    );
}

#[test]
fn identity_gaps_and_allocator_history_are_not_serialised() {
    let mut graph = Graph::new();
    let removed = graph
        .insert(Kind::Constructor("discard".into()), &[])
        .unwrap();
    graph.remove_set(&BTreeSet::from([removed])).unwrap();
    let retained = graph.insert(Kind::Constructor("keep".into()), &[]).unwrap();
    let source = print(&graph).unwrap();
    assert_eq!(source, "program {\n  atom a0 = Constructor(\"keep\");\n}\n");
    let loaded = load(&source).unwrap();
    assert_ne!(loaded.name("a0").unwrap().number(), retained.number());
    assert_eq!(isomorphic(&graph, loaded.graph(), 100), Ok(true));
    assert_eq!(isomorphic(loaded.graph(), &graph, 100), Ok(true));
    round_trip(&graph);
}

#[test]
fn invalid_graph_and_resource_bounds_return_no_output() {
    let mut invalid = Graph::new();
    invalid.insert(Kind::RuleHead, &[]).unwrap();
    assert_eq!(
        print(&invalid).unwrap_err().kind(),
        graph_core::ErrorKind::InvalidConfiguration
    );
    let graph = load("program { atom x = Constructor(\"\\u{0}\"); }").unwrap();
    let output = print(graph.graph()).unwrap();
    let mut limits = ParseLimits::default();
    limits.graph.max_input_bytes = output.len();
    assert_eq!(print_with_limits(graph.graph(), &limits).unwrap(), output);
    limits.graph.max_input_bytes -= 1;
    assert_eq!(
        print_with_limits(graph.graph(), &limits)
            .unwrap_err()
            .context()
            .limit,
        Some(graph_core::LimitKind::InputBytes)
    );
    limits = ParseLimits::default();
    limits.max_tokens = 0;
    assert_eq!(
        print_with_limits(graph.graph(), &limits)
            .unwrap_err()
            .context()
            .limit,
        Some(graph_core::LimitKind::SyntaxTokens)
    );
    limits = ParseLimits::default();
    limits.graph.max_atoms = 0;
    assert!(print_with_limits(graph.graph(), &limits).is_err());
}

fn cycles(next: &[usize]) -> Graph {
    let mut graph = Graph::new();
    graph
        .insert_batch(
            &next
                .iter()
                .map(|i| NewAtom {
                    kind: Kind::Nat,
                    fields: vec![Target::Fresh(*i)],
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    graph
}

#[test]
fn isomorphism_is_not_just_counts_degrees_or_key_equality() {
    assert_eq!(
        isomorphic(&cycles(&[1, 2, 3, 0]), &cycles(&[1, 0, 3, 2]), 10000),
        Ok(false)
    );
    assert_eq!(
        isomorphic(&cycles(&[0, 1]), &cycles(&[1, 0]), 10000),
        Ok(false)
    );
    let a = load("program { atom a = Constructor(\"A\"); atom b = Constructor(\"B\"); atom e = Record[ObjectLink](\"x\",a,b); }").unwrap();
    for text in ["program { atom a = Constructor(\"A\"); atom b = Constructor(\"B\"); atom e = Record[ObjectLink](\"x\",b,a); }",
        "program { atom a = Constructor(\"A\"); atom b = Constructor(\"B\"); atom e = Record[ObjectLink](\"y\",a,b); }"] {
        assert_eq!(isomorphic(a.graph(), load(text).unwrap().graph(), 10000), Ok(false));
    }
    assert!(isomorphic(&cycles(&[0]), &cycles(&[0]), 0).is_err());
}

#[test]
fn isomorphism_preserves_variable_attributes_and_multiplicity() {
    let a = load("program { atom x = PatternVariable[RuleHead] Free; }").unwrap();
    let b = load("program { atom x = PatternVariable[RuleHead] AppliedRule; }").unwrap();
    let c = load("program { atom x = PatternVariable[Nat] Free; }").unwrap();
    assert_eq!(isomorphic(a.graph(), b.graph(), 100), Ok(false));
    assert_eq!(isomorphic(a.graph(), c.graph(), 100), Ok(false));
    assert_eq!(isomorphic(&cycles(&[0]), &cycles(&[0, 1]), 100), Ok(false));
}

#[test]
fn reproducible_generated_carriers_round_trip() {
    let mut seed = 0x5e_f2026_u64;
    for size in 1..=32 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed
        };
        let mut graph = Graph::new();
        let atoms: Vec<_> = (0..size)
            .map(|i| {
                graph
                    .insert(Kind::Constructor(format!("label{} λ", i % 3)), &[])
                    .unwrap()
            })
            .collect();
        for _ in 0..size {
            let a = (next() % size as u64) as usize;
            let b = (next() % size as u64) as usize;
            graph
                .insert(
                    Kind::Record {
                        role: Role::ObjectLink,
                        label: Some(format!("e{}", next() % 2)),
                    },
                    &[atoms[a], atoms[b]],
                )
                .unwrap();
        }
        round_trip(&graph);
    }
}

#[test]
fn exact_oracle_agrees_with_brute_force_on_small_functional_carriers() {
    fn brute(a: &[usize], b: &[usize], p: &mut [usize], at: usize) -> bool {
        if at == p.len() {
            return (0..p.len()).all(|i| p[a[i]] == b[p[i]]);
        }
        for j in at..p.len() {
            p.swap(at, j);
            let yes = brute(a, b, p, at + 1);
            p.swap(at, j);
            if yes {
                return true;
            }
        }
        false
    }
    let mut seed = 73_u64;
    for size in 1..=6 {
        for _ in 0..20 {
            let mut generate = || {
                (0..size)
                    .map(|_| {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        (seed % size as u64) as usize
                    })
                    .collect::<Vec<_>>()
            };
            let a = generate();
            let b = generate();
            let mut p: Vec<_> = (0..size).collect();
            assert_eq!(
                isomorphic(&cycles(&a), &cycles(&b), 100000).unwrap(),
                brute(&a, &b, &mut p, 0)
            );
        }
    }
}
