use crate::*;
use graph_core::{Diagnostic, ErrorKind, LimitKind, Role, Sort};

type Result<T> = std::result::Result<T, SourceDiagnostic>;

/// Parse an entire program. No partial AST is returned when diagnostics exist.
pub fn parse(source: &str) -> std::result::Result<Program, Vec<SourceDiagnostic>> {
    parse_with_limits(source, &ParseLimits::default())
}

pub fn parse_with_limits(
    source: &str,
    limits: &ParseLimits,
) -> std::result::Result<Program, Vec<SourceDiagnostic>> {
    let lexed = lex_with_limits(source, limits);
    if lexed
        .diagnostics
        .iter()
        .any(|e| e.diagnostic.kind() == ErrorKind::ResourceExhaustion)
    {
        return Err(lexed.diagnostics);
    }
    let mut parser = Parser {
        tokens: lexed.tokens,
        position: 0,
        errors: lexed.diagnostics,
        nesting: limits.max_nesting.min(MAX_SYNTAX_NESTING),
        max_diagnostics: limits.max_diagnostics,
        exhausted: false,
    };
    let result = parser.program();
    if let Err(error) = &result {
        parser.report(error.clone());
    }
    parser.errors.sort_by_key(|e| (e.span.start, e.span.end));
    if parser.errors.is_empty() {
        Ok(result.expect("successful parse without diagnostics"))
    } else {
        Err(parser.errors)
    }
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    errors: Vec<SourceDiagnostic>,
    nesting: usize,
    max_diagnostics: usize,
    exhausted: bool,
}

const DECLARATIONS: &[TokenKind] = &[TokenKind::Atom, TokenKind::Rule];
const RULE_ITEMS: &[TokenKind] = &[
    TokenKind::Atom,
    TokenKind::Rank,
    TokenKind::Mode,
    TokenKind::Revision,
    TokenKind::Active,
    TokenKind::Lhs,
    TokenKind::Rhs,
];
const SIDE_ITEMS: &[TokenKind] = &[TokenKind::Identifier, TokenKind::Package];

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.position]
    }
    fn at(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }
    fn take(&mut self) -> Token {
        let token = self.peek().clone();
        if token.kind != TokenKind::Eof {
            self.position += 1;
        }
        token
    }
    fn expect(&mut self, kind: TokenKind) -> Result<Token> {
        if self.at(kind) {
            Ok(self.take())
        } else {
            Err(self.error(format!("expected {kind:?}, found {:?}", self.peek().kind)))
        }
    }
    fn error(&self, message: impl Into<String>) -> SourceDiagnostic {
        SourceDiagnostic::syntax(self.peek().span, message)
    }
    fn span_from(&self, start: usize) -> Span {
        Span {
            start,
            end: self.tokens[self.position.saturating_sub(1)].span.end,
        }
    }
    fn name(&mut self) -> Result<Name> {
        let token = self.expect(TokenKind::Identifier)?;
        Ok(Name {
            text: token.value.expect("identifier value"),
            span: token.span,
        })
    }
    fn label(&mut self) -> Result<Located<String>> {
        let token = self.expect(TokenKind::String)?;
        Ok(Located {
            value: token.value.expect("string value"),
            span: token.span,
        })
    }
    fn role(&mut self) -> Result<Located<Role>> {
        if let TokenKind::Role(role) = self.peek().kind {
            Ok(Located {
                value: role,
                span: self.take().span,
            })
        } else {
            Err(self.error("expected a record role"))
        }
    }
    fn report(&mut self, error: SourceDiagnostic) {
        if error.diagnostic.kind() == ErrorKind::ResourceExhaustion {
            self.errors.push(error);
        } else if !self.exhausted {
            if self.errors.len() >= self.max_diagnostics {
                self.errors.push(SourceDiagnostic {
                    span: error.span,
                    diagnostic: Diagnostic::exhausted(
                        LimitKind::SyntaxDiagnostics,
                        "syntax diagnostic limit exceeded",
                    ),
                });
                self.exhausted = true;
                self.position = self.tokens.len() - 1;
            } else {
                self.errors.push(error);
            }
        }
    }
    fn enter(&mut self, depth: usize) -> Result<()> {
        if depth <= self.nesting {
            return Ok(());
        }
        let span = self.peek().span;
        self.exhausted = true;
        self.position = self.tokens.len() - 1;
        Err(SourceDiagnostic {
            span,
            diagnostic: Diagnostic::exhausted(
                LimitKind::SyntaxNesting,
                "syntax nesting limit exceeded",
            ),
        })
    }
    // Brace-balanced synchronisation is iterative, including malformed blocks
    // that exceed the recursive parser's limit. Semicolons end declarations even
    // when parentheses/brackets were omitted; a parent closing brace is not eaten.
    fn recover(&mut self, start: usize, starters: &[TokenKind]) {
        let mut braces = 0usize;
        for token in &self.tokens[start..self.position] {
            match token.kind {
                TokenKind::OpenBrace => braces += 1,
                TokenKind::CloseBrace => braces = braces.saturating_sub(1),
                _ => {}
            }
        }
        if braces == 0
            && self.position > start
            && self.tokens[self.position - 1].kind == TokenKind::Semicolon
        {
            return;
        }
        while !self.at(TokenKind::Eof) {
            let kind = self.peek().kind;
            let starter = starters.contains(&kind)
                && (kind != TokenKind::Identifier
                    || self
                        .tokens
                        .get(self.position + 1)
                        .is_some_and(|t| t.kind == TokenKind::Colon));
            if braces == 0 && (kind == TokenKind::CloseBrace || (self.position > start && starter))
            {
                return;
            }
            self.take();
            match kind {
                TokenKind::OpenBrace => braces += 1,
                TokenKind::CloseBrace => braces = braces.saturating_sub(1),
                TokenKind::Semicolon if braces == 0 => return,
                _ => {}
            }
        }
    }
    fn program(&mut self) -> Result<Program> {
        let start = self.expect(TokenKind::Program)?.span.start;
        self.expect(TokenKind::OpenBrace)?;
        let declarations = self.declarations(0);
        self.expect(TokenKind::CloseBrace)?;
        let span = self.span_from(start);
        self.expect(TokenKind::Eof)?;
        Ok(Program { declarations, span })
    }
    fn declarations(&mut self, depth: usize) -> Vec<Declaration> {
        let mut declarations = Vec::new();
        while !self.at(TokenKind::CloseBrace) && !self.at(TokenKind::Eof) {
            let start = self.position;
            let result = match self.peek().kind {
                TokenKind::Atom => self.atom().map(Declaration::Atom),
                TokenKind::Rule => self.rule(depth + 1).map(Declaration::Rule),
                _ => Err(self.error("expected atom or rule declaration")),
            };
            match result {
                Ok(declaration) => declarations.push(declaration),
                Err(error) => {
                    self.report(error);
                    self.recover(start, DECLARATIONS);
                }
            }
        }
        declarations
    }
    fn sort(&mut self) -> Result<Located<Sort>> {
        let start = self.peek().span.start;
        let value = match self.peek().kind {
            TokenKind::Constructor => {
                self.take();
                Sort::Constructor
            }
            TokenKind::PatternVariable => {
                self.take();
                Sort::PatternVariable
            }
            TokenKind::RuleHead => {
                self.take();
                Sort::RuleHead
            }
            TokenKind::Nat => {
                self.take();
                Sort::Nat
            }
            TokenKind::Record => {
                self.take();
                self.expect(TokenKind::OpenBracket)?;
                let role = self.role()?.value;
                self.expect(TokenKind::CloseBracket)?;
                Sort::Record(role)
            }
            _ => return Err(self.error("expected a refined semantic sort")),
        };
        Ok(Located {
            value,
            span: self.span_from(start),
        })
    }
    fn fields(&mut self) -> Result<Vec<Name>> {
        let mut fields = Vec::new();
        if !self.at(TokenKind::CloseParen) {
            fields.push(self.name()?);
            while self.at(TokenKind::Comma) {
                self.take();
                fields.push(self.name()?);
            }
        }
        self.expect(TokenKind::CloseParen)?;
        Ok(fields)
    }
    fn atom(&mut self) -> Result<AtomDeclaration> {
        let start = self.expect(TokenKind::Atom)?.span.start;
        let name = self.name()?;
        self.expect(TokenKind::Equals)?;
        let kind_start = self.peek().span.start;
        let mut fields = Vec::new();
        let value = match self.peek().kind {
            TokenKind::Constructor => {
                self.take();
                self.expect(TokenKind::OpenParen)?;
                let label = self.label()?;
                self.expect(TokenKind::CloseParen)?;
                AtomSyntax::Constructor(label)
            }
            TokenKind::RuleHead => {
                self.take();
                AtomSyntax::RuleHead
            }
            TokenKind::Nat => {
                self.take();
                self.expect(TokenKind::OpenParen)?;
                fields = self.fields()?;
                if fields.len() > 1 {
                    return Err(SourceDiagnostic::syntax(
                        self.span_from(kind_start),
                        "Nat requires zero or one predecessor field",
                    ));
                }
                AtomSyntax::Nat
            }
            TokenKind::PatternVariable => {
                self.take();
                self.expect(TokenKind::OpenBracket)?;
                let mut admits = vec![self.sort()?];
                while self.at(TokenKind::Comma) {
                    self.take();
                    admits.push(self.sort()?);
                }
                self.expect(TokenKind::CloseBracket)?;
                let TokenKind::Binding(binding) = self.peek().kind else {
                    return Err(self.error("expected explicit Free or AppliedRule binding"));
                };
                let binding = Located {
                    value: binding,
                    span: self.take().span,
                };
                AtomSyntax::PatternVariable { admits, binding }
            }
            TokenKind::Record => {
                self.take();
                self.expect(TokenKind::OpenBracket)?;
                let role = self.role()?;
                self.expect(TokenKind::CloseBracket)?;
                self.expect(TokenKind::OpenParen)?;
                let label = if role.value == Role::ObjectLink {
                    let label = self.label()?;
                    self.expect(TokenKind::Comma)?;
                    Some(label)
                } else {
                    None
                };
                fields = self.fields()?;
                if fields.len() != role.value.arity() {
                    return Err(SourceDiagnostic::syntax(
                        self.span_from(kind_start),
                        format!(
                            "{:?} requires {} fields, found {}",
                            role.value,
                            role.value.arity(),
                            fields.len()
                        ),
                    ));
                }
                AtomSyntax::Record { role, label }
            }
            _ => return Err(self.error("expected an atom kind")),
        };
        let kind = Located {
            value,
            span: self.span_from(kind_start),
        };
        self.expect(TokenKind::Semicolon)?;
        Ok(AtomDeclaration {
            name,
            kind,
            fields,
            span: self.span_from(start),
        })
    }
    fn rule(&mut self, depth: usize) -> Result<RuleDeclaration> {
        self.enter(depth)?;
        let start = self.expect(TokenKind::Rule)?.span.start;
        let name = self.name()?;
        self.expect(TokenKind::OpenBrace)?;
        let mut items = Vec::new();
        while !self.at(TokenKind::CloseBrace) && !self.at(TokenKind::Eof) {
            let checkpoint = self.position;
            let result = match self.peek().kind {
                TokenKind::Atom => self.atom().map(RuleItem::Atom),
                TokenKind::Lhs | TokenKind::Rhs => self.side(depth).map(RuleItem::Side),
                TokenKind::Rank | TokenKind::Mode | TokenKind::Revision | TokenKind::Active => {
                    self.header().map(RuleItem::Header)
                }
                _ => Err(self.error("expected atom, rule header or lhs/rhs block")),
            };
            match result {
                Ok(item) => items.push(item),
                Err(error) => {
                    self.report(error);
                    self.recover(checkpoint, RULE_ITEMS);
                }
            }
        }
        self.expect(TokenKind::CloseBrace)?;
        Ok(RuleDeclaration {
            name,
            items,
            span: self.span_from(start),
        })
    }
    fn header(&mut self) -> Result<Located<Header>> {
        let token = self.take();
        let record = self.name()?;
        let value = match token.kind {
            TokenKind::Rank => {
                self.expect(TokenKind::Equals)?;
                Header::Rank {
                    record,
                    root: self.name()?,
                }
            }
            TokenKind::Mode => {
                self.expect(TokenKind::Equals)?;
                let role = self.role()?;
                if !matches!(role.value, Role::ModeObject | Role::ModeMeta) {
                    return Err(SourceDiagnostic::syntax(
                        role.span,
                        "mode requires ModeObject or ModeMeta",
                    ));
                }
                Header::Mode { record, role }
            }
            TokenKind::Revision => Header::Revision { record },
            TokenKind::Active => Header::Active { record },
            _ => unreachable!("header dispatch"),
        };
        self.expect(TokenKind::Semicolon)?;
        Ok(Located {
            value,
            span: self.span_from(token.span.start),
        })
    }
    fn side(&mut self, depth: usize) -> Result<Side> {
        let token = self.take();
        let role = Located {
            value: if token.kind == TokenKind::Lhs {
                Role::Lhs
            } else {
                Role::Rhs
            },
            span: token.span,
        };
        self.expect(TokenKind::OpenBrace)?;
        let mut items = Vec::new();
        while !self.at(TokenKind::CloseBrace) && !self.at(TokenKind::Eof) {
            let start = self.position;
            match self.side_item(role.value, depth) {
                Ok(item) => items.push(item),
                Err(error) => {
                    self.report(error);
                    self.recover(start, SIDE_ITEMS);
                }
            }
        }
        self.expect(TokenKind::CloseBrace)?;
        Ok(Side {
            role,
            items,
            span: self.span_from(token.span.start),
        })
    }
    fn side_item(&mut self, role: Role, depth: usize) -> Result<SideItem> {
        if self.at(TokenKind::Package) {
            if role != Role::Rhs {
                return Err(self.error("package is permitted only in rhs"));
            }
            self.enter(depth + 1)?;
            let start = self.take().span.start;
            self.expect(TokenKind::OpenBrace)?;
            let declarations = self.declarations(depth + 1);
            self.expect(TokenKind::CloseBrace)?;
            return Ok(SideItem::Package {
                declarations,
                span: self.span_from(start),
            });
        }
        let record = self.name()?;
        let start = record.span.start;
        self.expect(TokenKind::Colon)?;
        let item = self.name()?;
        self.expect(TokenKind::Semicolon)?;
        Ok(SideItem::Membership {
            record,
            item,
            span: self.span_from(start),
        })
    }
}
