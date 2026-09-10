use graph_core::{Binding, Role};
use parser::*;

#[test]
fn reserved_keywords_are_exact_case_sensitive_words() {
    let lexed = lex("program program2 Program atom rule rank mode revision active lhs rhs package Constructor PatternVariable RuleHead Nat Record ObjectLink Lhs Rhs Active Rank ModeObject ModeMeta Revision Free AppliedRule");
    assert!(lexed.diagnostics.is_empty());
    assert_eq!(lexed.tokens[0].kind, TokenKind::Program);
    assert_eq!(lexed.tokens[1].kind, TokenKind::Identifier);
    assert_eq!(lexed.tokens[2].kind, TokenKind::Identifier);
    assert_eq!(lexed.tokens[17].kind, TokenKind::Role(Role::ObjectLink));
    assert_eq!(
        lexed.tokens[26].kind,
        TokenKind::Binding(Binding::AppliedRule)
    );
    assert_eq!(lexed.tokens.last().unwrap().kind, TokenKind::Eof);
}

#[test]
fn identifiers_punctuation_and_eof_retain_byte_spans() {
    let source = "_x x2 {}[]()=:;,";
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    assert_eq!(lexed.tokens[0].value.as_deref(), Some("_x"));
    for token in &lexed.tokens[..lexed.tokens.len() - 1] {
        assert!(token.span.start < token.span.end);
        assert!(source.get(token.span.start..token.span.end).is_some());
    }
    assert_eq!(
        lexed.tokens.last().unwrap().span,
        Span {
            start: source.len(),
            end: source.len()
        }
    );
}

#[test]
fn strings_decode_supported_escapes_without_unicode_normalisation() {
    let lexed = lex(r#""a\n\r\t\0\\\"\u{1F600}λ" "" "é" "e\u{301}""#);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    assert_eq!(lexed.tokens[0].value.as_deref(), Some("a\n\r\t\0\\\"😀λ"));
    assert_eq!(lexed.tokens[1].value.as_deref(), Some(""));
    assert_ne!(lexed.tokens[2].value, lexed.tokens[3].value);
}

#[test]
fn invalid_escapes_and_unicode_scalars_produce_one_error_per_literal() {
    for source in [
        r#""\q""#,
        r#""\u{}""#,
        r#""\u{D800}""#,
        r#""\u{110000}""#,
        r#""\u{0000000}""#,
        r#""\u{xyz}""#,
        r#""\u1234""#,
        r#""\u{12""#,
        "\"raw\tcontrol\"",
    ] {
        let lexed = lex(source);
        assert_eq!(lexed.diagnostics.len(), 1, "{source}");
        assert_eq!(lexed.tokens[0].kind, TokenKind::Invalid);
        assert_eq!(
            lexed.diagnostics[0].span,
            Span {
                start: 0,
                end: source.len()
            }
        );
    }
}

#[test]
fn unterminated_strings_and_comments_have_bounded_spans() {
    for source in ["\"abc", "\"abc\\", "/* abc", "\"abc\n", "\"abc\\\n"] {
        let lexed = lex(source);
        assert_eq!(lexed.diagnostics.len(), 1, "{source}");
        assert!(lexed.diagnostics[0].diagnostic.0.contains("unterminated"));
        assert!(lexed.diagnostics[0].span.end <= source.len());
    }
}

#[test]
fn comment_delimiters_inside_strings_are_literal_and_blocks_are_non_nested() {
    let lexed = lex("// first\r\n /* α /* inner */ \"// /* */\" // eof");
    assert!(lexed.diagnostics.is_empty());
    assert_eq!(lexed.tokens.len(), 2);
    assert_eq!(lexed.tokens[0].value.as_deref(), Some("// /* */"));
}

#[test]
fn unknown_unicode_characters_are_not_identifiers_and_positions_count_scalars() {
    let source = "// λ\r\n\"é😀\" @ λ";
    let lexed = lex(source);
    assert_eq!(lexed.diagnostics.len(), 2);
    let at = source.find('@').unwrap();
    assert_eq!(
        source_position(source, at),
        Some(Position { line: 2, column: 6 })
    );
    assert_eq!(
        source_position(source, source.len()),
        Some(Position { line: 2, column: 9 })
    );
    assert_eq!(
        source_position(source, source.find('😀').unwrap() + 1),
        None
    );
    assert_eq!(source_position(source, source.len() + 1), None);
    assert_eq!(
        source_position("a\rb\r\nc\nd\t!", 10),
        Some(Position { line: 4, column: 4 })
    );
    for error in lexed.diagnostics {
        assert!(source.get(error.span.start..error.span.end).is_some());
    }
}

#[test]
fn lexical_and_independent_syntax_errors_are_both_reported() {
    let errors =
        parse("program { atom a = Constructor(\"bad\\q\"); atom b = Nat(a,b); } @").unwrap_err();
    assert!(errors.iter().any(|e| e.diagnostic.0.contains("escape")));
    assert!(errors
        .iter()
        .any(|e| e.diagnostic.0.contains("predecessor")));
    assert!(errors.iter().any(|e| e.diagnostic.0.contains("unknown")));
}

#[test]
fn generated_short_malformed_inputs_terminate_with_valid_diagnostic_spans() {
    let alphabet = [
        '{', '}', '(', ')', '[', ']', ';', '"', '\\', '/', '*', '\n', 'λ', 'a', '@',
    ];
    let mut state = 41u64;
    for length in 0..300 {
        let mut source = "program { ".to_owned();
        for _ in 0..length {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            source.push(alphabet[(state as usize) % alphabet.len()]);
        }
        source.push('}');
        if let Err(errors) = parse(&source) {
            for error in errors {
                assert!(source.get(error.span.start..error.span.end).is_some());
            }
        }
    }
}
