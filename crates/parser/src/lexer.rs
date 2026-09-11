use crate::{ParseLimits, SourceDiagnostic, Span};
use graph_core::{Binding, Diagnostic, LimitKind, Role};
use logos::Logos;

#[derive(Logos, Clone, Copy, Debug, PartialEq, Eq)]
enum RawToken {
    #[regex(r"[ \t\r\n]+")]
    Whitespace,
    #[token("//")]
    LineComment,
    #[token("/*")]
    BlockComment,
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Identifier,
    #[token("\"")]
    Quote,
    #[token("{")]
    OpenBrace,
    #[token("}")]
    CloseBrace,
    #[token("[")]
    OpenBracket,
    #[token("]")]
    CloseBracket,
    #[token("(")]
    OpenParen,
    #[token(")")]
    CloseParen,
    #[token("=")]
    Equals,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,
    #[token(",")]
    Comma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Program,
    Atom,
    Rule,
    Rank,
    Mode,
    Revision,
    Active,
    Lhs,
    Rhs,
    Package,
    Constructor,
    PatternVariable,
    RuleHead,
    Nat,
    Record,
    Role(Role),
    Binding(Binding),
    Identifier,
    String,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    Equals,
    Colon,
    Semicolon,
    Comma,
    Invalid,
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// Identifier spelling or decoded label; absent for punctuation and keywords.
    pub value: Option<String>,
}

/// Includes one EOF token even on malformed input. Invalid tokens retain their
/// positions so parser recovery cannot silently join text across lexical errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

pub fn lex(source: &str) -> Lexed {
    lex_with_limits(source, &ParseLimits::default())
}

pub fn lex_with_limits(source: &str, limits: &ParseLimits) -> Lexed {
    let eof = Token {
        kind: TokenKind::Eof,
        span: Span {
            start: source.len(),
            end: source.len(),
        },
        value: None,
    };
    if let Err(diagnostic) = limits.graph.check_input(source) {
        return Lexed {
            tokens: vec![eof],
            diagnostics: vec![SourceDiagnostic {
                related: Box::default(),
                diagnostic,
                span: Span { start: 0, end: 0 },
            }],
        };
    }
    let mut result = Lexed {
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut lexer = RawToken::lexer(source);
    while let Some(raw) = lexer.next() {
        let mut value = None;
        let mut error = None;
        let kind = match raw {
            Ok(RawToken::Whitespace) => continue,
            Ok(RawToken::LineComment) => {
                let end = lexer
                    .remainder()
                    .find(['\r', '\n'])
                    .unwrap_or(lexer.remainder().len());
                lexer.bump(end);
                continue;
            }
            Ok(RawToken::BlockComment) => {
                if let Some(end) = lexer.remainder().find("*/") {
                    lexer.bump(end + 2);
                    continue;
                }
                lexer.bump(lexer.remainder().len());
                error = Some("unterminated block comment".to_owned());
                TokenKind::Invalid
            }
            Ok(RawToken::Identifier) => {
                let kind = word(lexer.slice());
                if kind == TokenKind::Identifier {
                    value = Some(lexer.slice().to_owned());
                }
                kind
            }
            Ok(RawToken::Quote) => match string(&mut lexer) {
                Ok(label) => {
                    value = Some(label);
                    TokenKind::String
                }
                Err(message) => {
                    error = Some(message);
                    TokenKind::Invalid
                }
            },
            Ok(RawToken::OpenBrace) => TokenKind::OpenBrace,
            Ok(RawToken::CloseBrace) => TokenKind::CloseBrace,
            Ok(RawToken::OpenBracket) => TokenKind::OpenBracket,
            Ok(RawToken::CloseBracket) => TokenKind::CloseBracket,
            Ok(RawToken::OpenParen) => TokenKind::OpenParen,
            Ok(RawToken::CloseParen) => TokenKind::CloseParen,
            Ok(RawToken::Equals) => TokenKind::Equals,
            Ok(RawToken::Colon) => TokenKind::Colon,
            Ok(RawToken::Semicolon) => TokenKind::Semicolon,
            Ok(RawToken::Comma) => TokenKind::Comma,
            Err(()) => {
                error = Some("unknown source character".to_owned());
                TokenKind::Invalid
            }
        };
        let span = Span {
            start: lexer.span().start,
            end: lexer.span().end,
        };
        if let Some(message) = error {
            if result.diagnostics.len() >= limits.max_diagnostics {
                result.diagnostics.push(SourceDiagnostic {
                    related: Box::default(),
                    span,
                    diagnostic: Diagnostic::exhausted(
                        LimitKind::SyntaxDiagnostics,
                        "syntax diagnostic limit exceeded",
                    ),
                });
                break;
            }
            result
                .diagnostics
                .push(SourceDiagnostic::syntax(span, message));
        }
        if result.tokens.len() >= limits.max_tokens {
            result.diagnostics.push(SourceDiagnostic {
                related: Box::default(),
                span,
                diagnostic: Diagnostic::exhausted(
                    LimitKind::SyntaxTokens,
                    "syntax token limit exceeded",
                ),
            });
            break;
        }
        result.tokens.push(Token { kind, span, value });
    }
    result.tokens.push(eof);
    result
}

fn word(text: &str) -> TokenKind {
    match text {
        "program" => TokenKind::Program,
        "atom" => TokenKind::Atom,
        "rule" => TokenKind::Rule,
        "rank" => TokenKind::Rank,
        "mode" => TokenKind::Mode,
        "revision" => TokenKind::Revision,
        "active" => TokenKind::Active,
        "lhs" => TokenKind::Lhs,
        "rhs" => TokenKind::Rhs,
        "package" => TokenKind::Package,
        "Constructor" => TokenKind::Constructor,
        "PatternVariable" => TokenKind::PatternVariable,
        "RuleHead" => TokenKind::RuleHead,
        "Nat" => TokenKind::Nat,
        "Record" => TokenKind::Record,
        "ObjectLink" => TokenKind::Role(Role::ObjectLink),
        "Lhs" => TokenKind::Role(Role::Lhs),
        "Rhs" => TokenKind::Role(Role::Rhs),
        "Active" => TokenKind::Role(Role::Active),
        "Rank" => TokenKind::Role(Role::Rank),
        "ModeObject" => TokenKind::Role(Role::ModeObject),
        "ModeMeta" => TokenKind::Role(Role::ModeMeta),
        "Revision" => TokenKind::Role(Role::Revision),
        "Free" => TokenKind::Binding(Binding::Free),
        "AppliedRule" => TokenKind::Binding(Binding::AppliedRule),
        _ => TokenKind::Identifier,
    }
}

fn string(lexer: &mut logos::Lexer<'_, RawToken>) -> Result<String, String> {
    let rest = lexer.remainder();
    let mut escaped = false;
    for (index, ch) in rest.char_indices() {
        if ch == '\n' || ch == '\r' {
            lexer.bump(index);
            return Err("unterminated string before line ending".to_owned());
        }
        if ch == '"' && !escaped {
            let decoded = decode(&rest[..index]);
            lexer.bump(index + 1);
            return decoded;
        }
        escaped = ch == '\\' && !escaped;
    }
    lexer.bump(rest.len());
    Err("unterminated string at end of input".to_owned())
}

fn decode(raw: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch.is_control() {
            return Err("raw control character in string; use an escape".to_owned());
        }
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let escaped = match chars.next() {
            Some('"') => '"',
            Some('\\') => '\\',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('0') => '\0',
            Some('u') => {
                if chars.next() != Some('{') {
                    return Err("Unicode escape requires \\u{hex}".to_owned());
                }
                let (mut value, mut digits, mut closed) = (0u32, 0, false);
                for digit in chars.by_ref() {
                    if digit == '}' {
                        closed = true;
                        break;
                    }
                    digits += 1;
                    if digits > 6 || !digit.is_ascii_hexdigit() {
                        return Err("Unicode escape requires one to six hex digits".to_owned());
                    }
                    value = value * 16 + digit.to_digit(16).expect("checked hex digit");
                }
                if !closed || digits == 0 {
                    return Err("incomplete Unicode escape".to_owned());
                }
                char::from_u32(value)
                    .ok_or_else(|| "Unicode escape is not a Unicode scalar value".to_owned())?
            }
            _ => return Err("invalid string escape".to_owned()),
        };
        out.push(escaped);
    }
    Ok(out)
}
