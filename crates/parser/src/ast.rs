use crate::Span;
use graph_core::{Binding, Role, Sort};

/// An authored name or reference, never an allocated AtomKey.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located<T> {
    pub value: T,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Declaration {
    Atom(AtomDeclaration),
    Rule(RuleDeclaration),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomDeclaration {
    pub name: Name,
    pub kind: Located<AtomSyntax>,
    /// Vector order is field-slot order; labels are not fields.
    pub fields: Vec<Name>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomSyntax {
    Constructor(Located<String>),
    PatternVariable {
        /// Retain occurrences and spans; resolution may diagnose duplicate sorts.
        admits: Vec<Located<Sort>>,
        binding: Located<Binding>,
    },
    RuleHead,
    Nat,
    Record {
        role: Located<Role>,
        label: Option<Located<String>>,
    },
}

/// Declares a head and named representation records, with no metadata defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDeclaration {
    pub name: Name,
    pub items: Vec<RuleItem>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleItem {
    Atom(AtomDeclaration),
    Header(Located<Header>),
    Side(Side),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Header {
    Rank { record: Name, root: Name },
    Mode { record: Name, role: Located<Role> },
    Revision { record: Name },
    Active { record: Name },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub role: Located<Role>,
    pub items: Vec<SideItem>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideItem {
    Membership {
        record: Name,
        item: Name,
        span: Span,
    },
    /// RHS only. Lowering must quote every expanded atom in this package.
    Package {
        declarations: Vec<Declaration>,
        span: Span,
    },
}
