use crate::{lex_with_limits, ParseLimits};
use graph_core::{
    validate_with_limits, Binding, Diagnostic, Graph, Kind, LimitKind, Result, Role, Sort,
};
use std::collections::BTreeMap;
use std::fmt::{self, Write};

/// Serialise the complete validated carrier as flat, explicit atom declarations.
/// Names a0, a1, ... follow current identity order but are not semantic identities
/// or canonical labels. Comments, original names and source formatting are lost.
/// Loading the result preserves the carrier up to typed attributed isomorphism,
/// not execution history, source provenance or allocator state.
pub fn print(graph: &Graph) -> Result<String> {
    print_with_limits(graph, &ParseLimits::default())
}

/// Validate before printing; bound output bytes before each string append and
/// check the resulting token budget. No partial output is returned on failure.
pub fn print_with_limits(graph: &Graph, limits: &ParseLimits) -> Result<String> {
    validate_with_limits(graph, &limits.graph)?;
    let names: BTreeMap<_, _> = graph
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (id, i))
        .collect();
    let mut output = Output {
        text: String::new(),
        maximum: limits.graph.max_input_bytes,
    };
    render(graph, &names, &mut output).map_err(|_| {
        Diagnostic::exhausted(
            LimitKind::InputBytes,
            "printed source exceeds output byte limit",
        )
    })?;
    if let Some(error) = lex_with_limits(&output.text, limits)
        .diagnostics
        .into_iter()
        .next()
    {
        return Err(error.diagnostic);
    }
    Ok(output.text)
}

struct Output {
    text: String,
    maximum: usize,
}
impl Write for Output {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self
            .text
            .len()
            .checked_add(text.len())
            .is_none_or(|n| n > self.maximum)
        {
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::ObjectLink => "ObjectLink",
        Role::Lhs => "Lhs",
        Role::Rhs => "Rhs",
        Role::Active => "Active",
        Role::Rank => "Rank",
        Role::ModeObject => "ModeObject",
        Role::ModeMeta => "ModeMeta",
        Role::Revision => "Revision",
    }
}
fn sort(out: &mut Output, sort: Sort) -> fmt::Result {
    match sort {
        Sort::Constructor => out.write_str("Constructor"),
        Sort::PatternVariable => out.write_str("PatternVariable"),
        Sort::RuleHead => out.write_str("RuleHead"),
        Sort::Nat => out.write_str("Nat"),
        Sort::Record(role) => write!(out, "Record[{}]", role_name(role)),
    }
}
fn label(out: &mut Output, text: &str) -> fmt::Result {
    out.write_char('"')?;
    for ch in text.chars() {
        match ch {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\n' => out.write_str("\\n")?,
            '\r' => out.write_str("\\r")?,
            '\t' => out.write_str("\\t")?,
            ch if ch.is_control() => write!(out, "\\u{{{:x}}}", ch as u32)?,
            ch => out.write_char(ch)?,
        }
    }
    out.write_char('"')
}
fn render(
    graph: &Graph,
    names: &BTreeMap<graph_core::AtomKey, usize>,
    out: &mut Output,
) -> fmt::Result {
    out.write_str("program {\n")?;
    for (id, atom) in graph.iter() {
        write!(out, "  atom a{} = ", names[&id])?;
        match &atom.kind {
            Kind::Constructor(text) => {
                out.write_str("Constructor(")?;
                label(out, text)?;
            }
            Kind::RuleHead => {
                out.write_str("RuleHead;\n")?;
                continue;
            }
            Kind::Nat => out.write_str("Nat(")?,
            Kind::PatternVariable { admits, binding } => {
                out.write_str("PatternVariable[")?;
                for (i, admitted) in admits.iter().enumerate() {
                    if i > 0 {
                        out.write_str(", ")?;
                    }
                    sort(out, *admitted)?;
                }
                out.write_str("] ")?;
                out.write_str(match binding {
                    Binding::Free => "Free",
                    Binding::AppliedRule => "AppliedRule",
                })?;
                out.write_str(";\n")?;
                continue;
            }
            Kind::Record { role, label: text } => {
                write!(out, "Record[{}](", role_name(*role))?;
                if let Some(text) = text {
                    label(out, text)?;
                    out.write_str(", ")?;
                }
            }
        }
        for (i, target) in atom.fields.iter().enumerate() {
            if i > 0 {
                out.write_str(", ")?;
            }
            write!(out, "a{}", names[target])?;
        }
        out.write_str(");\n")?;
    }
    out.write_str("}\n")
}
