use crate::*;
use graph_core::{
    validate_with_limits, AtomKey, Diagnostic, ErrorKind, Graph, Kind, LimitKind, NewAtom, Role,
    Target,
};
use std::collections::BTreeMap;

/// Source provenance for an atom in the loaded baseline, not a semantic label.
/// Generated package memberships point to their package block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomOrigin {
    pub declaration: Span,
    pub kind: Span,
    pub fields: Vec<Span>,
    pub generated: bool,
}

/// A completely validated, read-only loaded graph with authored names and source
/// provenance. No partially built graph is published on failure. Consuming this
/// value releases an ordinary construction buffer, not a perpetual certificate.
///
/// ```compile_fail
/// let loaded = parser::load("program {}").unwrap();
/// loaded.graph().insert(graph_core::Kind::Nat, &[]).unwrap();
/// ```
#[derive(Debug)]
pub struct LoadedProgram {
    graph: Graph,
    names: BTreeMap<String, AtomKey>,
    origins: BTreeMap<AtomKey, AtomOrigin>,
}
impl LoadedProgram {
    pub fn graph(&self) -> &Graph {
        &self.graph
    }
    pub fn names(&self) -> &BTreeMap<String, AtomKey> {
        &self.names
    }
    pub fn name(&self, name: &str) -> Option<AtomKey> {
        self.names.get(name).copied()
    }
    pub fn origin(&self, atom: AtomKey) -> Option<&AtomOrigin> {
        self.origins.get(&atom)
    }
    pub fn into_graph(self) -> Graph {
        self.graph
    }
}

/// Parse, resolve, expand and validate a complete program in a separate buffer.
///
/// ```
/// let loaded = parser::load(r#"program { atom x = Constructor("hello"); }"#)?;
/// let x = loaded.name("x").unwrap();
/// assert_eq!(loaded.graph().len(), 1);
/// assert!(loaded.origin(x).is_some());
/// # Ok::<(), Vec<parser::SourceDiagnostic>>(())
/// ```
pub fn load(source: &str) -> std::result::Result<LoadedProgram, Vec<SourceDiagnostic>> {
    load_with_limits(source, &ParseLimits::default())
}

pub fn load_with_limits(
    source: &str,
    limits: &ParseLimits,
) -> std::result::Result<LoadedProgram, Vec<SourceDiagnostic>> {
    let program = parse_with_limits(source, limits)?;
    // Only the bounded parser can feed this private lowering path. Public,
    // editable AST values are descriptive data, not trusted construction input.
    let mut lowering = Lowering {
        limits,
        plans: Vec::new(),
        symbols: BTreeMap::new(),
        packages: Vec::new(),
        errors: Vec::new(),
        stopped: false,
        fields: 0,
        labels: 0,
    };
    lowering.declarations(&program.declarations);
    lowering.finish(program.span)
}

struct Symbol {
    index: usize,
    span: Span,
    ambiguous: bool,
}

enum Reference<'a> {
    Named(&'a Name),
    Local(usize, Span),
}
impl Reference<'_> {
    fn span(&self) -> Span {
        match self {
            Self::Named(n) => n.span,
            Self::Local(_, span) => *span,
        }
    }
}

struct Plan<'a> {
    kind: Kind,
    fields: Vec<Reference<'a>>,
    origin: AtomOrigin,
}

#[derive(Clone, Copy)]
struct Package {
    start: usize,
    end: usize,
    owner: usize,
    span: Span,
}

struct Lowering<'a> {
    limits: &'a ParseLimits,
    plans: Vec<Plan<'a>>,
    symbols: BTreeMap<&'a str, Symbol>,
    packages: Vec<Package>,
    errors: Vec<SourceDiagnostic>,
    stopped: bool,
    fields: usize,
    labels: usize,
}

impl<'a> Lowering<'a> {
    fn report(&mut self, error: SourceDiagnostic) {
        if self.stopped {
            return;
        }
        if error.diagnostic.kind() == ErrorKind::ResourceExhaustion {
            self.stopped = true;
            self.errors.push(error);
        } else if self.errors.len() >= self.limits.max_diagnostics {
            self.stopped = true;
            self.errors.push(SourceDiagnostic::new(
                Diagnostic::exhausted(
                    LimitKind::SyntaxDiagnostics,
                    "lowering diagnostic limit exceeded",
                ),
                error.span,
            ));
        } else {
            self.errors.push(error);
        }
    }

    fn count(
        &mut self,
        old: usize,
        extra: usize,
        maximum: usize,
        limit: LimitKind,
        span: Span,
    ) -> Option<usize> {
        match old.checked_add(extra).filter(|sum| *sum <= maximum) {
            Some(sum) => Some(sum),
            None => {
                self.report(SourceDiagnostic::new(
                    Diagnostic::exhausted(limit, format!("expanded {limit:?} limit exceeded")),
                    span,
                ));
                None
            }
        }
    }

    fn add(
        &mut self,
        name: Option<&'a Name>,
        kind: Kind,
        fields: Vec<Reference<'a>>,
        declaration: Span,
        kind_span: Span,
    ) -> Option<usize> {
        if self.stopped {
            return None;
        }
        let label_bytes = match &kind {
            Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => s.len(),
            _ => 0,
        };
        let limits = self.limits.graph;
        self.count(
            self.plans.len(),
            1,
            limits.max_atoms,
            LimitKind::Atoms,
            declaration,
        )?;
        let field_total = self.count(
            self.fields,
            fields.len(),
            limits.max_fields,
            LimitKind::Fields,
            declaration,
        )?;
        let label_total = self.count(
            self.labels,
            label_bytes,
            limits.max_label_bytes,
            LimitKind::LabelBytes,
            kind_span,
        )?;
        let index = self.plans.len();
        if let Some(name) = name {
            if let Some(previous) = self.symbols.get_mut(name.text.as_str()) {
                previous.ambiguous = true;
                let first = previous.span;
                self.report(
                    SourceDiagnostic::syntax(
                        name.span,
                        format!("duplicate declaration: {}", name.text),
                    )
                    .with_related(first, "first declaration of this name"),
                );
            } else {
                self.symbols.insert(
                    &name.text,
                    Symbol {
                        index,
                        span: name.span,
                        ambiguous: false,
                    },
                );
            }
        }
        if self.stopped {
            return None;
        }
        self.fields = field_total;
        self.labels = label_total;
        let origin = AtomOrigin {
            declaration,
            kind: kind_span,
            fields: fields.iter().map(Reference::span).collect(),
            generated: name.is_none(),
        };
        self.plans.push(Plan {
            kind,
            fields,
            origin,
        });
        Some(index)
    }

    fn declarations(&mut self, declarations: &'a [Declaration]) {
        for declaration in declarations {
            if self.stopped {
                break;
            }
            match declaration {
                Declaration::Atom(atom) => self.atom(atom),
                Declaration::Rule(rule) => self.rule(rule),
            }
        }
    }

    fn atom(&mut self, atom: &'a AtomDeclaration) {
        let kind = match &atom.kind.value {
            AtomSyntax::Constructor(label) => Kind::Constructor(label.value.clone()),
            AtomSyntax::RuleHead => Kind::RuleHead,
            AtomSyntax::Nat => Kind::Nat,
            AtomSyntax::Record { role, label } => Kind::Record {
                role: role.value,
                label: label.as_ref().map(|s| s.value.clone()),
            },
            AtomSyntax::PatternVariable { admits, binding } => {
                let mut seen = BTreeMap::new();
                for sort in admits {
                    if let Some(first) = seen.get(&sort.value) {
                        self.report(
                            SourceDiagnostic::syntax(sort.span, "duplicate admitted sort")
                                .with_related(*first, "first occurrence of this sort"),
                        );
                    } else {
                        seen.insert(sort.value, sort.span);
                    }
                    if self.stopped {
                        return;
                    }
                }
                Kind::PatternVariable {
                    admits: seen.into_keys().collect(),
                    binding: binding.value,
                }
            }
        };
        self.add(
            Some(&atom.name),
            kind,
            atom.fields.iter().map(Reference::Named).collect(),
            atom.span,
            atom.kind.span,
        );
    }

    fn rule(&mut self, rule: &'a RuleDeclaration) {
        let Some(head) = self.add(
            Some(&rule.name),
            Kind::RuleHead,
            vec![],
            rule.span,
            rule.name.span,
        ) else {
            return;
        };
        for item in &rule.items {
            if self.stopped {
                break;
            }
            match item {
                RuleItem::Atom(atom) => self.atom(atom),
                RuleItem::Header(header) => {
                    let mut fields = vec![Reference::Local(head, rule.name.span)];
                    let (name, role) = match &header.value {
                        Header::Rank { record, root } => {
                            fields.push(Reference::Named(root));
                            (record, Role::Rank)
                        }
                        Header::Mode { record, role } => (record, role.value),
                        Header::Revision { record } => (record, Role::Revision),
                        Header::Active { record } => (record, Role::Active),
                    };
                    self.add(
                        Some(name),
                        Kind::record(role),
                        fields,
                        header.span,
                        header.span,
                    );
                }
                RuleItem::Side(side) => {
                    for item in &side.items {
                        if self.stopped {
                            break;
                        }
                        match item {
                            SideItem::Membership { record, item, span } => {
                                self.add(
                                    Some(record),
                                    Kind::record(side.role.value),
                                    vec![
                                        Reference::Local(head, rule.name.span),
                                        Reference::Named(item),
                                    ],
                                    *span,
                                    side.role.span,
                                );
                            }
                            SideItem::Package { declarations, span } => {
                                let start = self.plans.len();
                                self.declarations(declarations);
                                let end = self.plans.len();
                                self.packages.push(Package {
                                    start,
                                    end,
                                    owner: head,
                                    span: *span,
                                });
                                // Include inner generated memberships, but never
                                // include these new enclosing memberships themselves.
                                for index in start..end {
                                    let item_span = self.plans[index].origin.declaration;
                                    if self
                                        .add(
                                            None,
                                            Kind::record(Role::Rhs),
                                            vec![
                                                Reference::Local(head, rule.name.span),
                                                Reference::Local(index, item_span),
                                            ],
                                            *span,
                                            *span,
                                        )
                                        .is_none()
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn finish(
        mut self,
        program_span: Span,
    ) -> std::result::Result<LoadedProgram, Vec<SourceDiagnostic>> {
        let mut batch = Vec::new();
        for index in 0..self.plans.len() {
            if self.stopped {
                break;
            }
            let plan = &self.plans[index];
            let mut fields = Vec::new();
            let mut errors = Vec::new();
            for reference in &plan.fields {
                match reference {
                    Reference::Local(index, _) => fields.push(Target::Fresh(*index)),
                    Reference::Named(name) => match self.symbols.get(name.text.as_str()) {
                        Some(symbol) if !symbol.ambiguous => {
                            fields.push(Target::Fresh(symbol.index))
                        }
                        Some(_) => {} // Already reported: do not select an arbitrary duplicate.
                        None => errors.push(
                            SourceDiagnostic::syntax(
                                name.span,
                                format!("unresolved reference: {}", name.text),
                            )
                            .with_related(
                                plan.origin.declaration,
                                "declaration containing this reference",
                            ),
                        ),
                    },
                }
            }
            batch.push(NewAtom {
                kind: plan.kind.clone(),
                fields,
            });
            for error in errors {
                self.report(error);
            }
        }
        if !self.errors.is_empty() {
            self.errors.sort_by_key(|e| (e.span.start, e.span.end));
            return Err(self.errors);
        }
        self.check_packages(&batch);
        if !self.errors.is_empty() {
            self.errors.sort_by_key(|e| (e.span.start, e.span.end));
            return Err(self.errors);
        }
        let mut graph = Graph::new();
        let keys = match graph.insert_batch_with_limits(&batch, &self.limits.graph) {
            Ok(keys) => keys,
            Err(error) => return Err(self.failure(error, &[], program_span)),
        };
        if let Err(error) = validate_with_limits(&graph, &self.limits.graph) {
            return Err(self.failure(error, &keys, program_span));
        }
        let names = self
            .symbols
            .into_iter()
            .map(|(name, symbol)| (name.to_owned(), keys[symbol.index]))
            .collect();
        let origins = keys
            .into_iter()
            .zip(self.plans.into_iter().map(|p| p.origin))
            .collect();
        Ok(LoadedProgram {
            graph,
            names,
            origins,
        })
    }

    fn check_packages(&mut self, batch: &[NewAtom]) {
        let index = |target: Target| match target {
            Target::Fresh(i) => i,
            Target::Existing(_) => unreachable!("source loading uses one fresh batch"),
        };
        let mut lhs = BTreeMap::<usize, Vec<(usize, usize)>>::new();
        for (i, atom) in batch.iter().enumerate() {
            if atom.kind.role() == Some(Role::Lhs) {
                lhs.entry(index(atom.fields[0]))
                    .or_default()
                    .push((index(atom.fields[1]), i));
            }
        }
        for p in 0..self.packages.len() {
            if self.stopped {
                break;
            }
            let package = self.packages[p];
            let range = package.start..package.end;
            for i in range.clone() {
                if self.stopped {
                    break;
                }
                for (slot, target) in batch[i].fields.iter().enumerate() {
                    if !range.contains(&index(*target)) {
                        let origin = &self.plans[i].origin;
                        self.report(
                            SourceDiagnostic::new(
                                Diagnostic::new(
                                    ErrorKind::MalformedInput,
                                    "package field references an atom outside its block",
                                )
                                .at_role(batch[i].kind.role())
                                .at_slot(slot),
                                origin.fields[slot],
                            )
                            .with_related(
                                package.span,
                                "package must be closed over its expanded declarations",
                            ),
                        );
                    }
                    if self.stopped {
                        break;
                    }
                }
            }
            if let Some(memberships) = lhs.get(&package.owner) {
                for (item, membership) in memberships {
                    if self.stopped {
                        break;
                    }
                    if range.contains(item) {
                        let span = self.plans[*membership].origin.fields[1];
                        self.report(
                            SourceDiagnostic::new(
                                Diagnostic::new(
                                    ErrorKind::MalformedInput,
                                    "package item also occurs on its enclosing lhs",
                                )
                                .at_role(Some(Role::Lhs)),
                                span,
                            )
                            .with_related(
                                package.span,
                                "package shorthand denotes RHS-only syntax",
                            ),
                        );
                    }
                }
            }
        }
    }

    fn failure(
        mut self,
        error: Diagnostic,
        keys: &[AtomKey],
        fallback: Span,
    ) -> Vec<SourceDiagnostic> {
        let error = self.locate(error, keys, fallback);
        self.report(error);
        self.errors
    }

    fn locate(&self, error: Diagnostic, keys: &[AtomKey], fallback: Span) -> SourceDiagnostic {
        let indices: BTreeMap<_, _> = keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
        let context = error.context();
        let index = context
            .batch_index
            .or_else(|| context.atoms.iter().find_map(|k| indices.get(k).copied()));
        let primary = index.and_then(|i| self.plans.get(i));
        let span = primary
            .map(|p| {
                context
                    .slot
                    .and_then(|slot| p.origin.fields.get(slot).copied())
                    .unwrap_or(p.origin.kind)
            })
            .unwrap_or(fallback);
        let mut related = Vec::new();
        if let Some(plan) = primary {
            related.push(RelatedLocation {
                span: plan.origin.declaration,
                message: if plan.origin.generated {
                    "membership generated by this package"
                } else {
                    "declaration of the offending atom"
                }
                .to_owned(),
            });
        }
        for key in &context.atoms {
            if let Some(i) = indices.get(key) {
                let location = self.plans[*i].origin.declaration;
                if !related.iter().any(|r| r.span == location) {
                    related.push(RelatedLocation {
                        span: location,
                        message: "related graph atom".to_owned(),
                    });
                }
            }
        }
        SourceDiagnostic {
            diagnostic: error,
            span,
            related: related.into_boxed_slice(),
        }
    }
}
