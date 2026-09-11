use graph_core::{Binding, ErrorKind, GraphLimits, LimitKind, Role, Sort};
use parser::*;

fn program(body: &str) -> Program {
    parse(&format!("program {{ {body} }}")).unwrap()
}
fn atom(declaration: &Declaration) -> &AtomDeclaration {
    let Declaration::Atom(atom) = declaration else {
        panic!("expected atom")
    };
    atom
}
fn rule(declaration: &Declaration) -> &RuleDeclaration {
    let Declaration::Rule(rule) = declaration else {
        panic!("expected rule")
    };
    rule
}

#[test]
fn empty_program_and_comments_are_consumed_completely() {
    assert!(parse("/* start */ program { // empty\r\n } // end")
        .unwrap()
        .declarations
        .is_empty());
    assert!(parse("").is_err());
    assert!(parse("program {}").is_ok());
    assert!(parse("program {} program {}").is_err());
    assert!(parse("program {} stray").is_err());
    assert!(parse("program {}; ").is_err());
}

#[test]
fn all_five_kinds_retain_ordered_fields_and_attributes() {
    let p = program(
        r#"
        atom c = Constructor("label");
        atom v = PatternVariable[Constructor, Nat, Record[ObjectLink]] Free;
        atom h = RuleHead;
        atom z = Nat(); atom n = Nat(z);
        atom e = Record[ObjectLink]("link", c, n);
    "#,
    );
    assert_eq!(p.declarations.len(), 6);
    assert!(
        matches!(&atom(&p.declarations[0]).kind.value, AtomSyntax::Constructor(label) if label.value == "label")
    );
    let AtomSyntax::PatternVariable { admits, binding } = &atom(&p.declarations[1]).kind.value
    else {
        panic!()
    };
    assert_eq!(
        admits.iter().map(|s| s.value).collect::<Vec<_>>(),
        vec![Sort::Constructor, Sort::Nat, Sort::Record(Role::ObjectLink)]
    );
    assert_eq!(binding.value, Binding::Free);
    assert_eq!(atom(&p.declarations[2]).kind.value, AtomSyntax::RuleHead);
    assert!(atom(&p.declarations[3]).fields.is_empty());
    assert_eq!(atom(&p.declarations[4]).fields[0].text, "z");
    assert_eq!(
        atom(&p.declarations[5])
            .fields
            .iter()
            .map(|n| n.text.as_str())
            .collect::<Vec<_>>(),
        vec!["c", "n"]
    );
}

#[test]
fn every_record_role_has_its_exact_positional_arity() {
    for role in [
        Role::ObjectLink,
        Role::Lhs,
        Role::Rhs,
        Role::Rank,
        Role::Active,
        Role::ModeObject,
        Role::ModeMeta,
        Role::Revision,
    ] {
        let label = if role == Role::ObjectLink {
            "\"x\", "
        } else {
            ""
        };
        let fields = if role.arity() == 2 { "a, b" } else { "a" };
        let p = program(&format!("atom r = Record[{role:?}]({label}{fields});"));
        assert_eq!(atom(&p.declarations[0]).fields.len(), role.arity());
        for count in 0..=3 {
            if count == role.arity() {
                continue;
            }
            let fields = vec!["a"; count].join(", ");
            assert!(parse(&format!(
                "program {{ atom r = Record[{role:?}]({label}{fields}); }}"
            ))
            .is_err());
        }
    }
}

#[test]
fn refined_sorts_and_both_binding_modes_are_explicit() {
    let p = program("atom v = PatternVariable[RuleHead] AppliedRule; atom w = PatternVariable[PatternVariable, Record[Rank]] Free;");
    assert!(
        matches!(&atom(&p.declarations[0]).kind.value, AtomSyntax::PatternVariable { binding, .. } if binding.value == Binding::AppliedRule)
    );
    for bad in [
        "PatternVariable[] Free",
        "PatternVariable[Constructor]",
        "PatternVariable[Record] Free",
        "PatternVariable[Constructor,] Free",
    ] {
        assert!(
            parse(&format!("program {{ atom x = {bad}; }}")).is_err(),
            "{bad}"
        );
    }
    // Semantic admissibility belongs to lowering/validation, not syntax.
    assert!(parse("program { atom x = PatternVariable[Nat] AppliedRule; }").is_ok());
}

#[test]
fn nat_is_structural_and_has_no_numeric_shorthand() {
    for bad in ["Nat(a,b)", "Nat", "Nat(0)", "Nat(1)", "Nat(a,)"] {
        assert!(parse(&format!("program {{ atom n = {bad}; }}")).is_err());
    }
    assert_eq!(
        atom(&program("atom n = Nat(n);").declarations[0]).fields[0].text,
        "n"
    );
}

#[test]
fn rule_headers_and_shared_sides_have_named_record_identities() {
    let p = program("rule h { rank rk = zero; mode md = ModeMeta; revision rv; active ac; atom self_var = PatternVariable[RuleHead] AppliedRule; lhs { l: self_var; } rhs { r: self_var; } } atom zero = Nat();");
    let h = rule(&p.declarations[0]);
    assert_eq!(h.name.text, "h");
    assert!(
        matches!(&h.items[0], RuleItem::Header(Located { value: Header::Rank { record, root }, .. }) if record.text == "rk" && root.text == "zero")
    );
    assert!(
        matches!(&h.items[1], RuleItem::Header(Located { value: Header::Mode { role, .. }, .. }) if role.value == Role::ModeMeta)
    );
    let RuleItem::Side(lhs) = &h.items[5] else {
        panic!()
    };
    let RuleItem::Side(rhs) = &h.items[6] else {
        panic!()
    };
    let SideItem::Membership {
        record: l, item: x, ..
    } = &lhs.items[0]
    else {
        panic!()
    };
    let SideItem::Membership {
        record: r, item: y, ..
    } = &rhs.items[0]
    else {
        panic!()
    };
    assert_eq!(x.text, y.text);
    assert_ne!(l.text, r.text);
}

#[test]
fn incomplete_inactive_rules_and_duplicate_headers_remain_syntax_not_validity() {
    let p =
        program("rule h { rank r = z; mode m = ModeObject; revision v; lhs {} } atom z = Nat();");
    assert_eq!(rule(&p.declarations[0]).items.len(), 4);
    assert!(parse("program { rule h {} }").is_ok());
    assert!(parse("program { rule h { revision a; revision b; } }").is_ok());
    assert!(parse("program { rule h { mode m = Rank; } }").is_err());
}

#[test]
fn forward_cycles_parallel_links_and_duplicates_remain_distinct_in_ast() {
    let p = program(
        r#"atom a = Record[ObjectLink]("x", a, b); atom b = Constructor("b"); atom c = Record[ObjectLink]("x", a, b); atom b = Constructor("duplicate");"#,
    );
    assert_eq!(p.declarations.len(), 4);
    assert_eq!(atom(&p.declarations[0]).fields[0].text, "a");
    assert_ne!(
        atom(&p.declarations[0]).name.text,
        atom(&p.declarations[2]).name.text
    );
    assert_ne!(
        atom(&p.declarations[1]).name.span,
        atom(&p.declarations[3]).name.span
    );
}

#[test]
fn package_keeps_inner_rule_variables_and_memberships_structured() {
    let p = parse(include_str!("../../../examples/syntax/install-wrap.graph")).unwrap();
    let outer = rule(&p.declarations[0]);
    let RuleItem::Side(rhs) = &outer.items[7] else {
        panic!()
    };
    let SideItem::Package { declarations, .. } = &rhs.items[0] else {
        panic!()
    };
    let inner = rule(&declarations[0]);
    assert_eq!(inner.name.text, "wrap");
    assert!(inner.items.iter().any(|i| matches!(i, RuleItem::Atom(a) if a.name.text == "x" && matches!(a.kind.value, AtomSyntax::PatternVariable { .. }))));
    assert!(matches!(&inner.items[8], RuleItem::Side(_)));
}

#[test]
fn package_is_rhs_only_and_not_an_independent_atom_kind() {
    for source in [
        "program { package {} }",
        "program { rule h { package {} } }",
        "program { rule h { lhs { package { atom a = Nat(); } } } }",
    ] {
        assert!(parse(source).is_err());
    }
    assert!(parse("program { rule h { rhs { package {} } } }").is_ok());
}

#[test]
fn spans_cover_names_references_attributes_and_blocks() {
    let source = "// π\nprogram { atom x = Record[ObjectLink](\"λ\", a, b); }";
    let p = parse(source).unwrap();
    let a = atom(&p.declarations[0]);
    assert_eq!(&source[a.name.span.start..a.name.span.end], "x");
    assert_eq!(&source[a.fields[1].span.start..a.fields[1].span.end], "b");
    let AtomSyntax::Record {
        role,
        label: Some(label),
    } = &a.kind.value
    else {
        panic!()
    };
    assert_eq!(&source[role.span.start..role.span.end], "ObjectLink");
    assert_eq!(&source[label.span.start..label.span.end], "\"λ\"");
    assert_eq!(
        &source[a.span.start..a.span.end],
        "atom x = Record[ObjectLink](\"λ\", a, b);"
    );
    assert_eq!(&source[p.span.start..p.span.end], &source[6..]);
}

#[test]
fn missing_punctuation_wrong_arity_and_eof_are_diagnosed() {
    for bad in [
        "program { atom x Constructor(\"x\"); }",
        "program { atom x = Constructor(\"x\") }",
        "program { atom x = Nat(a,b); }",
        "program { rule h { rank r z; } }",
        "program { rule h { rhs { m x; } } }",
        "program {",
        "program { rule h {",
        "program { atom x = Nat(",
    ] {
        assert!(!parse(bad).unwrap_err().is_empty(), "{bad}");
    }
    let source = "program {";
    assert_eq!(
        parse(source).unwrap_err()[0].span,
        Span {
            start: source.len(),
            end: source.len()
        }
    );
}

#[test]
fn recovery_reports_independent_errors_inside_and_after_rules() {
    let source = "program { atom a = Nat(x,y); rule h { rank r zero; mode m = Rank; rhs { bad x; good: x; } } atom b = Nat(x,y,z); }";
    let errors = parse(source).unwrap_err();
    assert_eq!(errors.len(), 5, "{errors:#?}");
    assert!(errors
        .windows(2)
        .all(|e| e[0].span.start <= e[1].span.start));
    assert!(errors.last().unwrap().span.start > source.find("atom b").unwrap());
}

#[test]
fn recovery_balances_invalid_nested_blocks_and_keeps_outer_boundary() {
    let source = "program { rule a { lhs { package { rule b { rhs { package {} } } } } } atom x = Nat(a,b); }";
    let errors = parse(source).unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:#?}");
    assert!(errors[1].diagnostic.0.contains("predecessor"));
}

fn nested(levels: usize) -> String {
    format!(
        "program {{ {} {} }}",
        "rule h { rhs { package { ".repeat(levels),
        "} } }".repeat(levels)
    )
}

#[test]
fn exact_nesting_bounds_and_zero_are_enforced_without_recursing_unboundedly() {
    let limits = ParseLimits {
        max_nesting: 4,
        ..ParseLimits::default()
    };
    assert!(parse_with_limits(&nested(2), &limits).is_ok());
    let errors = parse_with_limits(&nested(3), &limits).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].diagnostic.context().limit,
        Some(LimitKind::SyntaxNesting)
    );
    let zero = ParseLimits {
        max_nesting: 0,
        ..limits
    };
    assert!(parse_with_limits("program { atom n = Nat(); }", &zero).is_ok());
    assert!(parse_with_limits("program { rule h {} }", &zero).is_err());
    assert_eq!(
        parse_with_limits(
            &nested(10_000),
            &ParseLimits {
                max_nesting: usize::MAX,
                ..limits
            }
        )
        .unwrap_err()
        .len(),
        1
    );
}

#[test]
fn input_byte_preflight_is_shared_with_core_and_distinct_from_syntax_failure() {
    let source = "program {} // λ";
    let limits = ParseLimits {
        graph: GraphLimits {
            max_input_bytes: source.len(),
            ..GraphLimits::default()
        },
        ..ParseLimits::default()
    };
    assert!(parse_with_limits(source, &limits).is_ok());
    let too_small = ParseLimits {
        graph: GraphLimits {
            max_input_bytes: source.len() - 1,
            ..limits.graph
        },
        ..limits
    };
    let errors = parse_with_limits(source, &too_small).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].diagnostic.kind(), ErrorKind::ResourceExhaustion);
    assert_eq!(
        errors[0].diagnostic.context().limit,
        Some(LimitKind::InputBytes)
    );
}

#[test]
fn token_limits_exclude_comments_and_eof_and_report_truncation() {
    let limits = ParseLimits {
        max_tokens: 3,
        ..ParseLimits::default()
    };
    assert!(parse_with_limits("program { /* comment */ } // end", &limits).is_ok());
    let errors = parse_with_limits("program {} extra", &limits).unwrap_err();
    assert_eq!(
        errors[0].diagnostic.context().limit,
        Some(LimitKind::SyntaxTokens)
    );
    let zero = ParseLimits {
        max_tokens: 0,
        ..limits
    };
    assert!(lex_with_limits("/* empty */", &zero).diagnostics.is_empty());
    assert_eq!(lex_with_limits("x", &zero).tokens.len(), 1);
}

#[test]
fn diagnostic_limits_apply_to_lexing_and_parsing_without_returning_a_partial_ast() {
    let limits = ParseLimits {
        max_diagnostics: 2,
        ..ParseLimits::default()
    };
    for source in [
        "@ @ @ @ @",
        "program { atom a = Nat(x,y); atom b = Nat(x,y); atom c = Nat(x,y); }",
    ] {
        let errors = parse_with_limits(source, &limits).unwrap_err();
        assert_eq!(errors.len(), 3);
        assert_eq!(
            errors[2].diagnostic.context().limit,
            Some(LimitKind::SyntaxDiagnostics)
        );
    }
    let zero = ParseLimits {
        max_diagnostics: 0,
        ..limits
    };
    assert!(parse_with_limits("program {}", &zero).is_ok());
    let errors = parse_with_limits("program { atom x = ; }", &zero).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].diagnostic.kind(), ErrorKind::ResourceExhaustion);
}

#[test]
fn missing_semicolons_recover_at_the_next_complete_declaration_start() {
    let errors =
        parse("program { atom a = Nat() atom b = Nat(x,y); rule h { rhs { m: a n: b; } } }")
            .unwrap_err();
    assert_eq!(errors.len(), 3, "{errors:#?}");
}

#[test]
fn reserved_names_and_wrong_label_positions_are_syntax_errors() {
    for body in [
        "atom rule = Nat();",
        "rule atom {}",
        "atom x = Constructor(label);",
        "atom x = Record[ObjectLink](a,b);",
        "atom x = Record[Revision](\"label\", h);",
        "rule h { rhs { lhs: x; } }",
    ] {
        assert!(parse(&format!("program {{ {body} }}")).is_err(), "{body}");
    }
}

#[test]
fn documented_graph_examples_are_accepted_by_the_actual_parser() {
    // Test the tracked architecture rather than requiring the working paper or
    // locally ignored explainers in repository builds.
    let architecture = include_str!("../../../docs/architecture.md");
    let mut count = 0;
    for section in architecture.split("```graph\n").skip(1) {
        let source = section.split("```").next().unwrap();
        parse(source).unwrap();
        parser::load(source).unwrap();
        count += 1;
    }
    assert!(
        count > 0,
        "architecture must contain a runnable syntax specimen"
    );
}
