use graph_core::{validate, Binding, ErrorKind, GraphLimits, Kind, LimitKind, Role, Sort};
use parser::{load, load_with_limits, LoadedProgram, ParseLimits, SourceDiagnostic};
use std::collections::{BTreeMap, BTreeSet};

fn rule(name: &str, body: &str, meta: bool, active: bool) -> String {
    format!("rule {name} {{ atom {name}_zero = Nat(); rank {name}_rank = {name}_zero; mode {name}_mode = {}; revision {name}_rev; {} {body} }}", if meta { "ModeMeta" } else { "ModeObject" }, if active { format!("active {name}_active;") } else { String::new() })
}
fn source(body: &str) -> String {
    format!("program {{ {body} }}")
}
fn rejected(body: &str, fragment: &str) -> Vec<SourceDiagnostic> {
    let errors = load(&source(body)).unwrap_err();
    assert!(
        errors.iter().any(|e| e.diagnostic.0.contains(fragment)),
        "{errors:#?}"
    );
    errors
}
fn named_records(loaded: &LoadedProgram) -> BTreeMap<String, (Kind, Vec<String>)> {
    let inverse: BTreeMap<_, _> = loaded
        .names()
        .iter()
        .map(|(n, k)| (*k, n.clone()))
        .collect();
    loaded
        .names()
        .iter()
        .map(|(name, id)| {
            let atom = loaded.graph().get(*id).unwrap();
            (
                name.clone(),
                (
                    atom.kind.clone(),
                    atom.fields.iter().map(|id| inverse[id].clone()).collect(),
                ),
            )
        })
        .collect()
}

#[test]
fn empty_input_graph_and_equality_only_unicode_labels_load() {
    assert!(load("program {}").unwrap().graph().is_empty());
    let loaded = load(r#"program { atom a = Constructor("Add"); atom b = Constructor("λ\u{1f600}\n"); atom e = Record[ObjectLink]("+", a, b); }"#).unwrap();
    assert_eq!(loaded.graph().len(), 3);
    assert_eq!(
        loaded.graph().get(loaded.name("b").unwrap()).unwrap().kind,
        Kind::Constructor("λ😀\n".to_owned())
    );
    assert_eq!(
        loaded
            .graph()
            .get(loaded.name("e").unwrap())
            .unwrap()
            .fields,
        vec![loaded.name("a").unwrap(), loaded.name("b").unwrap()]
    );
}

#[test]
fn forward_references_logical_cycles_self_links_and_parallel_links_are_preserved() {
    let loaded = load(&source(r#"atom ab = Record[ObjectLink]("edge", a,b); atom ba = Record[ObjectLink]("edge", b,a); atom aa = Record[ObjectLink]("edge", a,a); atom ab2 = Record[ObjectLink]("edge", a,b); atom b = Constructor("B"); atom a = Constructor("A"); atom n = Nat(n);"#)).unwrap();
    assert_eq!(loaded.graph().len(), 7);
    let key = |n| loaded.name(n).unwrap();
    assert_ne!(key("ab"), key("ab2"));
    assert_eq!(
        loaded.graph().get(key("ab")),
        loaded.graph().get(key("ab2"))
    );
    assert_eq!(loaded.graph().get(key("n")).unwrap().fields, vec![key("n")]);
    assert!(validate(loaded.graph()).is_ok());
}

#[test]
fn duplicate_names_point_to_both_declarations_and_do_not_resolve_ambiguously() {
    let text = source("atom a = Nat(); atom a = Nat(); atom x = Nat(a); atom y = Nat(missing);");
    let errors = load(&text).unwrap_err();
    assert_eq!(errors.len(), 2);
    let duplicate = &errors[0];
    assert!(duplicate.diagnostic.0.contains("duplicate"));
    assert_eq!(&text[duplicate.span.start..duplicate.span.end], "a");
    assert_eq!(
        &text[duplicate.related[0].span.start..duplicate.related[0].span.end],
        "a"
    );
    assert!(duplicate.related[0].span.start < duplicate.span.start);
    assert!(errors[1].diagnostic.0.contains("missing"));
}

#[test]
fn file_wide_namespace_includes_heads_headers_memberships_and_packages() {
    for text in [
        source("rule h { revision h; }"),
        source(&rule("h", "atom h_rank = Nat();", true, false)),
        source(&rule(
            "h",
            "atom x = Nat(); lhs { same: x; } rhs { same: x; }",
            true,
            false,
        )),
        source(&format!(
            "{} {}",
            rule("h", "atom x = Nat();", true, false),
            rule("q", "rhs { package { atom x = Nat(); } }", true, false)
        )),
    ] {
        assert!(load(&text)
            .unwrap_err()
            .iter()
            .any(|e| e.diagnostic.0.contains("duplicate declaration")));
    }
}

#[test]
fn unresolved_fields_memberships_and_rank_roots_report_use_and_container() {
    let text = source(
        "atom n = Nat(missing_n); rule h { rank rk = missing_rank; lhs { m: missing_item; } }",
    );
    let errors = load(&text).unwrap_err();
    assert_eq!(
        errors.len(),
        3,
        "resolution failures must suppress metadata cascades"
    );
    for error in errors {
        assert_eq!(error.diagnostic.kind(), ErrorKind::MalformedInput);
        assert!(text[error.span.start..error.span.end].starts_with("missing_"));
        assert!(!error.related.is_empty());
    }
}

#[test]
fn headers_expand_to_named_records_without_defaults() {
    let loaded = load(&source(&rule("h", "", true, true))).unwrap();
    assert_eq!(loaded.graph().len(), 6);
    assert_eq!(loaded.graph().field_count(), 5);
    let h = loaded.name("h").unwrap();
    for (name, role) in [
        ("h_rank", Role::Rank),
        ("h_mode", Role::ModeMeta),
        ("h_rev", Role::Revision),
        ("h_active", Role::Active),
    ] {
        let atom = loaded.graph().get(loaded.name(name).unwrap()).unwrap();
        assert_eq!(atom.kind, Kind::record(role));
        assert_eq!(atom.fields[0], h);
    }
    let config = validate(loaded.graph()).unwrap();
    assert!(config.rules[&h].active);
    assert_eq!(config.rules[&h].rank, 0);
}

#[test]
fn shared_side_identity_and_literal_rhs_atoms_survive_lowering() {
    let loaded = load(&source(&rule("h", r#"atom x = PatternVariable[Constructor] Free; atom fresh = Constructor("F"); lhs { lx: x; } rhs { rx: x; rf: fresh; }"#, false, true))).unwrap();
    let config = validate(loaded.graph()).unwrap();
    let view = &config.rules[&loaded.name("h").unwrap()];
    assert_eq!(
        view.lhs
            .intersection(&view.rhs)
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([loaded.name("x").unwrap()])
    );
    assert!(view.rhs.contains(&loaded.name("fresh").unwrap()));
    assert_ne!(loaded.name("lx"), loaded.name("rx"));
}

#[test]
fn binding_modes_and_refined_sort_sets_are_not_defaulted_or_erased() {
    let loaded = load(&source(&rule("h", "atom me = PatternVariable[RuleHead] AppliedRule; atom any = PatternVariable[Constructor, Record[ObjectLink]] Free; lhs { lm: me; la: any; } rhs { rm: me; ra: any; }", true, true))).unwrap();
    assert_eq!(
        loaded.graph().get(loaded.name("me").unwrap()).unwrap().kind,
        Kind::variable(Sort::RuleHead, Binding::AppliedRule)
    );
    let errors = rejected(
        "atom x = PatternVariable[Nat] AppliedRule;",
        "variable declaration",
    );
    assert_eq!(errors[0].diagnostic.context().batch_index, Some(0));
    let duplicates = rejected(
        "atom x = PatternVariable[Nat,Nat] Free;",
        "duplicate admitted sort",
    );
    assert_eq!(duplicates[0].related.len(), 1);
    assert!(load("program { atom x = PatternVariable[Nat]; }").is_err());
}

#[test]
fn installer_package_matches_hand_derived_counts_and_literal_variable_ownership() {
    let loaded = load(include_str!("../../../examples/syntax/install-wrap.graph")).unwrap();
    assert_eq!(
        (loaded.graph().len(), loaded.graph().field_count()),
        (33, 47)
    );
    assert_eq!(loaded.names().len(), 20);
    let config = validate(loaded.graph()).unwrap();
    let install = loaded.name("install").unwrap();
    let wrap = loaded.name("wrap").unwrap();
    assert_eq!(config.rules.len(), 1);
    assert!(!config.rules.contains_key(&wrap));
    assert_eq!(config.rules[&install].package_atoms.len(), 13);
    assert!(config.rules[&install].lhs.is_empty());
    assert_eq!(config.quoted[&loaded.name("x").unwrap()], install);
    assert_eq!(
        loaded
            .graph()
            .iter()
            .filter(|(k, _)| loaded.origin(*k).unwrap().generated)
            .count(),
        13
    );
}

#[test]
fn nested_packages_include_inner_generated_memberships_but_not_their_own() {
    let bottom = rule("bottom", "", true, true);
    let middle = rule(
        "middle",
        &format!("rhs {{ package {{ {bottom} }} }}"),
        true,
        true,
    );
    let outer = rule(
        "outer",
        &format!("rhs {{ package {{ {middle} }} }}"),
        true,
        true,
    );
    let loaded = load(&source(&outer)).unwrap();
    // All rank zero is structurally valid; future installation fails GC.
    // bottom=6, middle=6+6+6, outer=6+18+18 atoms.
    assert_eq!(loaded.graph().len(), 42);
    let config = validate(loaded.graph()).unwrap();
    let head = loaded.name("outer").unwrap();
    assert_eq!(config.rules[&head].rhs.len(), 18);
    assert_eq!(config.rules[&head].package_atoms.len(), 18);
}

#[test]
fn generated_memberships_cannot_collide_with_authored_names() {
    let body = format!(
        "atom _generated_0 = Nat(); {}",
        rule(
            "h",
            "rhs { package { atom _generated_1 = Nat(); } }",
            true,
            false
        )
    );
    let loaded = load(&source(&body)).unwrap();
    assert_eq!(loaded.names().len() + 1, loaded.graph().len());
    assert!(loaded.name("_generated_0").is_some());
    assert!(loaded.name("_generated_1").is_some());
}

#[test]
fn field_type_errors_select_reference_spans_and_preserve_core_context() {
    let text = source(r#"atom c = Constructor("C"); atom n = Nat(c);"#);
    let errors = load(&text).unwrap_err();
    let error = &errors[0];
    assert_eq!(&text[error.span.start..error.span.end], "c");
    assert_eq!(error.diagnostic.kind(), ErrorKind::InvalidConfiguration);
    assert_eq!(error.diagnostic.context().slot, Some(0));
    assert_eq!(error.diagnostic.context().atoms.len(), 2);
    assert_eq!(error.related.len(), 2);
}

#[test]
fn missing_metadata_maps_to_head_and_nested_failure_maps_inside_package() {
    let text = source("rule h {}");
    let errors = load(&text).unwrap_err();
    assert_eq!(&text[errors[0].span.start..errors[0].span.end], "h");
    assert_eq!(errors[0].diagnostic.context().role, Some(Role::Rank));
    let nested = source(&rule("h", "rhs { package { rule broken {} } }", true, true));
    let errors = load(&nested).unwrap_err();
    assert_eq!(&nested[errors[0].span.start..errors[0].span.end], "broken");
    assert!(errors[0].related.len() >= 2);
}

#[test]
fn cross_owner_quotation_duplicate_membership_and_literal_lhs_heads_reject() {
    let first = rule("h", "atom x = Nat(); lhs { hx: x; }", true, false);
    let second = rule("q", "lhs { qx: x; }", true, false);
    rejected(&format!("{first} {second}"), "shared quoted syntax");
    rejected(
        &rule("h", "atom x = Nat(); lhs { a: x; b: x; }", true, false),
        "duplicate operative membership",
    );
    rejected(
        &rule(
            "h",
            "atom literal = RuleHead; lhs { a: literal; }",
            true,
            false,
        ),
        "literal LHS head",
    );
}

#[test]
fn missing_and_duplicate_rank_mode_revision_and_activation_reject() {
    let valid = rule("h", "", true, true);
    for missing in [
        "rank h_rank = h_zero;",
        "mode h_mode = ModeMeta;",
        "revision h_rev;",
    ] {
        rejected(
            &valid.replace(missing, ""),
            "unique rank/mode/revision/activity",
        );
    }
    for extra in [
        "rank another = h_zero;",
        "mode another = ModeMeta;",
        "mode another = ModeObject;",
        "revision another;",
        "active another;",
    ] {
        rejected(
            &rule("h", extra, true, true),
            "unique rank/mode/revision/activity",
        );
    }
    assert!(load(&source(&valid.replace("active h_active;", ""))).is_ok());
}

#[test]
fn rank_cycles_shared_suffixes_wrong_arity_and_wrong_root_sort_reject() {
    rejected(
        &rule("h", "", true, false).replace("Nat()", "Nat(h_zero)"),
        "cyclic rank",
    );
    let second = rule("q", "", true, false).replace("rank q_rank = q_zero", "rank q_rank = h_zero");
    rejected(
        &format!("{} {second}", rule("h", "", true, false)),
        "shared rank chain",
    );
    rejected(
        &rule("h", "", true, false).replace("Nat()", "Constructor(\"x\")"),
        "field typing",
    );
    for bad in ["Nat(a,b)", "Nat(0)"] {
        assert!(load(&source(&format!("atom z = {bad};"))).is_err());
    }
}

#[test]
fn unbound_rhs_and_incompatible_admits_and_object_metadata_reject() {
    rejected(
        &rule(
            "h",
            "atom x = PatternVariable[Nat] Free; rhs { m: x; }",
            true,
            true,
        ),
        "unbound RHS variable",
    );
    rejected(&rule("h", "atom x = PatternVariable[Nat, Constructor] Free; atom n = Nat(x); lhs { lx: x; ln: n; } rhs { rx: x; rn: n; }", true, true), "field typing");
    rejected(
        &rule(
            "h",
            "atom x = PatternVariable[Nat] Free; lhs { lx: x; } rhs { rx: x; }",
            false,
            true,
        ),
        "meta variable in object rule",
    );
    rejected(
        &rule(
            "h",
            "atom x = PatternVariable[RuleHead] AppliedRule; lhs { lx: x; } rhs { rx: x; }",
            false,
            true,
        ),
        "AppliedRule in object rule",
    );
}

#[test]
fn inactive_incomplete_sides_are_distinct_from_incomplete_packages() {
    let body = r#"atom a = Constructor("A"); atom b = Constructor("B"); atom e = Record[ObjectLink]("edge", a,b); lhs { le: e; } rhs { re: e; ra: a; rb: b; }"#;
    assert!(load(&source(&rule("h", body, true, false))).is_ok());
    rejected(&rule("h", body, true, true), "unclosed rule side");
    rejected(
        &rule("h", "rhs { package { rule broken {} } }", true, false),
        "unique rank/mode/revision/activity",
    );
}

#[test]
fn external_package_capture_and_membership_self_cycle_reject() {
    rejected(
        &format!(
            "atom external = Nat(); {}",
            rule(
                "h",
                "rhs { package { atom inside = Nat(external); } }",
                true,
                false
            )
        ),
        "package field references an atom outside its block",
    );
    rejected(
        &rule("h", "lhs { m: m; }", true, false),
        "rootless or indirectly owned membership",
    );
}

#[test]
fn declaration_permutations_agree_under_the_explicit_authored_name_bijection() {
    let declarations = [
        "atom z = Nat();",
        "atom h = RuleHead;",
        "atom rk = Record[Rank](h,z);",
        "atom md = Record[ModeMeta](h);",
        "atom rv = Record[Revision](h);",
        "atom ac = Record[Active](h);",
        "atom x = PatternVariable[Nat] Free;",
        "atom lx = Record[Lhs](h,x);",
        "atom rx = Record[Rhs](h,x);",
    ];
    let first = load(&source(&declarations.join(" "))).unwrap();
    let second = load(&source(
        &declarations.into_iter().rev().collect::<Vec<_>>().join(" "),
    ))
    .unwrap();
    assert_eq!(named_records(&first), named_records(&second));
    assert_ne!(first.name("h"), second.name("h"));
}

#[test]
fn limits_count_expanded_atoms_fields_and_utf8_labels_before_graph_allocation() {
    let text = source(&rule(
        "h",
        "rhs { package { atom x = Constructor(\"λ\"); } }",
        true,
        false,
    ));
    let expected = load(&text).unwrap();
    assert_eq!(
        (expected.graph().len(), expected.graph().field_count()),
        (7, 6)
    );
    let graph = GraphLimits {
        max_atoms: 7,
        max_fields: 6,
        max_label_bytes: 2,
        ..GraphLimits::default()
    };
    assert!(load_with_limits(
        &text,
        &ParseLimits {
            graph,
            ..ParseLimits::default()
        }
    )
    .is_ok());
    for (graph, kind) in [
        (
            GraphLimits {
                max_atoms: 6,
                ..graph
            },
            LimitKind::Atoms,
        ),
        (
            GraphLimits {
                max_fields: 5,
                ..graph
            },
            LimitKind::Fields,
        ),
        (
            GraphLimits {
                max_label_bytes: 1,
                ..graph
            },
            LimitKind::LabelBytes,
        ),
    ] {
        let errors = load_with_limits(
            &text,
            &ParseLimits {
                graph,
                ..ParseLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].diagnostic.context().limit, Some(kind));
        assert!(errors[0].span.end <= text.len());
    }
}

#[test]
fn input_validation_and_package_depth_limits_remain_resource_failures() {
    let text = include_str!("../../../examples/syntax/install-wrap.graph");
    for (graph, kind) in [
        (
            GraphLimits {
                max_input_bytes: text.len() - 1,
                ..GraphLimits::default()
            },
            LimitKind::InputBytes,
        ),
        (
            GraphLimits {
                max_validation_work: 0,
                ..GraphLimits::default()
            },
            LimitKind::ValidationWork,
        ),
        (
            GraphLimits {
                max_package_depth: 0,
                ..GraphLimits::default()
            },
            LimitKind::PackageDepth,
        ),
    ] {
        let errors = load_with_limits(
            text,
            &ParseLimits {
                graph,
                ..ParseLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(errors[0].diagnostic.kind(), ErrorKind::ResourceExhaustion);
        assert_eq!(errors[0].diagnostic.context().limit, Some(kind));
    }
}

#[test]
fn resolution_diagnostic_budget_truncates_without_publishing_a_graph() {
    let text = source("atom a = Nat(x); atom b = Nat(y); atom c = Nat(z);");
    let limits = ParseLimits {
        max_diagnostics: 1,
        ..ParseLimits::default()
    };
    let errors = load_with_limits(&text, &limits).unwrap_err();
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors[1].diagnostic.context().limit,
        Some(LimitKind::SyntaxDiagnostics)
    );
    let zero = ParseLimits {
        max_diagnostics: 0,
        ..limits
    };
    assert_eq!(load_with_limits(&text, &zero).unwrap_err().len(), 1);
}

#[test]
fn generated_expansion_is_bounded_even_when_source_nesting_is_small() {
    let mut body = "atom x = Nat();".to_owned();
    for level in 0..15 {
        body = rule(
            &format!("h{level}"),
            &format!("rhs {{ package {{ {body} }} }}"),
            true,
            false,
        );
    }
    let errors = load(&source(&body)).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].diagnostic.context().limit, Some(LimitKind::Atoms));
}

#[test]
fn origin_fields_and_generated_membership_provenance_are_complete() {
    let text = include_str!("../../../examples/syntax/install-wrap.graph");
    let loaded = load(text).unwrap();
    for (id, atom) in loaded.graph().iter() {
        let origin = loaded.origin(id).unwrap();
        assert_eq!(origin.fields.len(), atom.fields.len());
        for span in [origin.declaration, origin.kind]
            .into_iter()
            .chain(origin.fields.iter().copied())
        {
            assert!(text.get(span.start..span.end).is_some());
        }
        if origin.generated {
            assert!(text[origin.declaration.start..origin.declaration.end].starts_with("package"));
        }
    }
    assert!(!loaded.origin(loaded.name("x").unwrap()).unwrap().generated);
}

#[test]
fn package_blocks_cannot_capture_other_rhs_items_or_preserved_items() {
    let captured = rule(
        "h",
        "atom outside = Nat(); rhs { ro: outside; package { atom inside = Nat(outside); } }",
        true,
        false,
    );
    rejected(
        &captured,
        "package field references an atom outside its block",
    );
    let preserved = rule(
        "h",
        "lhs { lx: x; } rhs { package { atom x = Nat(); } }",
        true,
        false,
    );
    rejected(&preserved, "package item also occurs on its enclosing lhs");
    // This is a restriction of the fresh-package shorthand, not a restriction
    // on raw graph descriptions with ordinary shared L/R syntax.
    assert!(load(&source(&rule(
        "h",
        "atom x = Nat(); lhs { lx: x; } rhs { rx: x; }",
        true,
        false
    )))
    .is_ok());
}

#[test]
fn zero_error_budget_applies_to_core_construction_and_validation_failures() {
    let limits = ParseLimits {
        max_diagnostics: 0,
        ..ParseLimits::default()
    };
    for text in [
        "program { atom x = PatternVariable[Nat] AppliedRule; }",
        "program { rule h {} }",
    ] {
        let errors = load_with_limits(text, &limits).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].diagnostic.context().limit,
            Some(LimitKind::SyntaxDiagnostics)
        );
    }
    assert!(load_with_limits("program {}", &limits).is_ok());
}

#[test]
fn empty_packages_and_zero_construction_limits_have_defined_behaviour() {
    let empty = "program {}";
    let limits = ParseLimits {
        graph: GraphLimits {
            max_atoms: 0,
            max_fields: 0,
            max_label_bytes: 0,
            max_input_bytes: empty.len(),
            ..GraphLimits::default()
        },
        ..ParseLimits::default()
    };
    assert!(load_with_limits(empty, &limits).unwrap().graph().is_empty());
    assert!(load_with_limits("program {} ", &limits).is_err());
    let loaded = load(&source(&rule("h", "rhs { package {} }", true, false))).unwrap();
    assert_eq!(loaded.graph().len(), 5);
    assert!(loaded
        .graph()
        .iter()
        .all(|(key, _)| !loaded.origin(key).unwrap().generated));
}
