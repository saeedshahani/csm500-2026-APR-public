//! Source-located concrete syntax for graph-resident rewrite programs.
//!
//! Parsing checks syntax, not graph validity. Names are not runtime identities;
//! resolution, graph construction and semantic validation are separate operations.
//! [`load`] performs that complete pipeline into a separate validated buffer;
//! it never mutates a running execution.
//!
//! ```
//! let program = parser::parse(r#"program { atom hello = Constructor("Hello"); }"#)?;
//! assert_eq!(program.declarations.len(), 1);
//! # Ok::<(), Vec<parser::SourceDiagnostic>>(())
//! ```

mod ast;
pub mod lambda;
mod lexer;
mod lowering;
mod printer;
mod syntax;

pub use ast::*;
pub use lexer::{lex, lex_with_limits, Lexed, Token, TokenKind};
pub use lowering::{load, load_with_limits, AtomOrigin, LoadedProgram};
pub use printer::{print, print_with_limits};
pub use syntax::{parse, parse_with_limits};

use graph_core::{Diagnostic, ErrorKind, GraphLimits};

/// Half-open UTF-8 byte offsets into the original source. EOF has an empty span.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// One-based Unicode-scalar position, not a terminal display-cell measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// Returns None for an out-of-range offset or an offset inside a UTF-8 character.
/// CRLF is one newline; a lone CR is also a newline. Tabs count as one scalar.
pub fn source_position(source: &str, offset: usize) -> Option<Position> {
    let prefix = source.get(..offset)?;
    let (mut line, mut column, mut cr) = (1, 1, false);
    for ch in prefix.chars() {
        match ch {
            '\r' => {
                line += 1;
                column = 1;
            }
            '\n' if cr => {}
            '\n' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        cr = ch == '\r';
    }
    Some(Position { line, column })
}

/// Source attribution wraps, rather than parses or replaces, core diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceDiagnostic {
    pub diagnostic: Diagnostic,
    pub span: Span,
    pub related: Box<[RelatedLocation]>,
}

/// A second source location explaining a duplicate, reference or graph failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedLocation {
    pub span: Span,
    pub message: String,
}

impl SourceDiagnostic {
    pub fn new(diagnostic: Diagnostic, span: Span) -> Self {
        Self {
            diagnostic,
            span,
            related: Box::default(),
        }
    }
    pub fn with_related(mut self, span: Span, message: impl Into<String>) -> Self {
        let mut related = self.related.into_vec();
        related.push(RelatedLocation {
            span,
            message: message.into(),
        });
        self.related = related.into_boxed_slice();
        self
    }
    fn syntax(span: Span, message: impl Into<String>) -> Self {
        Self {
            diagnostic: Diagnostic::new(ErrorKind::MalformedInput, message),
            span,
            related: Box::default(),
        }
    }
}

/// Syntax resource settings. Input preflight reuses the graph-core contract.
/// Nesting counts rule and package blocks, not graph package-validation descents.
/// The hard ceiling bounds recursive parsing and recursive AST destruction even
/// when callers request a larger limit. These are not semantic rank bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseLimits {
    pub graph: GraphLimits,
    pub max_nesting: usize,
    /// Non-trivia tokens, excluding the mandatory EOF marker.
    pub max_tokens: usize,
    /// Ordinary errors; one additional resource diagnostic reports truncation.
    pub max_diagnostics: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            graph: GraphLimits::default(),
            max_nesting: 64,
            max_tokens: 200_000,
            max_diagnostics: 100,
        }
    }
}

pub const MAX_SYNTAX_NESTING: usize = 64;
