use crate::{Diagnostic, Graph, Kind, NewAtom, Result};

/// Which configurable or machine bound prevented completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind {
    InputBytes,
    Atoms,
    Fields,
    LabelBytes,
    PackageDepth,
    SyntaxNesting,
    SyntaxTokens,
    SyntaxDiagnostics,
    ValidationWork,
    Candidates,
    SearchWork,
    Identities,
    ExecutionSteps,
    ObservationWork,
    ComparisonWork,
    ObservationHistory,
    TraceStorage,
    AnalysisWork,
    AnalysisStorage,
}

/// Operational bounds, not restrictions on the mathematical formalism.
/// Zero is meaningful: for example, zero package depth permits no nested package.
/// These bounds do not promise recovery from operating-system allocation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphLimits {
    pub max_input_bytes: usize,
    pub max_atoms: usize,
    pub max_fields: usize,
    /// Total UTF-8 bytes in constructor and object-link labels.
    pub max_label_bytes: usize,
    /// Maximum number of recursive package-validation descents.
    pub max_package_depth: usize,
    /// Conservative scan/traversal units shared across all recursive validation.
    pub max_validation_work: usize,
}

impl Default for GraphLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 4 * 1024 * 1024,
            max_atoms: 10_000,
            max_fields: 20_000,
            max_label_bytes: 1024 * 1024,
            max_package_depth: 32,
            max_validation_work: 10_000_000,
        }
    }
}

fn label_bytes(kind: &Kind) -> usize {
    match kind {
        Kind::Constructor(label)
        | Kind::Record {
            label: Some(label), ..
        } => label.len(),
        _ => 0,
    }
}

pub(crate) fn check(value: usize, maximum: usize, kind: LimitKind) -> Result<()> {
    if value > maximum {
        Err(Diagnostic::exhausted(
            kind,
            format!("{kind:?} limit exceeded: {value} > {maximum}"),
        ))
    } else {
        Ok(())
    }
}

fn add(value: usize, extra: usize, maximum: usize, kind: LimitKind) -> Result<usize> {
    let sum = value
        .checked_add(extra)
        .ok_or_else(|| Diagnostic::exhausted(kind, format!("{kind:?} count overflow")))?;
    check(sum, maximum, kind)?;
    Ok(sum)
}

impl GraphLimits {
    /// Check before tokenisation or source copying. This does not parse the input.
    pub fn check_input(&self, text: &str) -> Result<()> {
        check(text.len(), self.max_input_bytes, LimitKind::InputBytes)
    }
    pub fn check_graph(&self, graph: &Graph) -> Result<()> {
        self.check_batch(graph, &[])
    }
    pub(crate) fn check_batch(&self, graph: &Graph, batch: &[NewAtom]) -> Result<()> {
        add(graph.len(), batch.len(), self.max_atoms, LimitKind::Atoms)?;
        let mut fields = 0;
        let mut labels = 0;
        for (id, atom) in graph.iter() {
            fields = add(
                fields,
                atom.fields.len(),
                self.max_fields,
                LimitKind::Fields,
            )
            .map_err(|e| e.at_atom(id).at_role(atom.kind.role()))?;
            labels = add(
                labels,
                label_bytes(&atom.kind),
                self.max_label_bytes,
                LimitKind::LabelBytes,
            )
            .map_err(|e| e.at_atom(id).at_role(atom.kind.role()))?;
        }
        for (index, atom) in batch.iter().enumerate() {
            fields = add(
                fields,
                atom.fields.len(),
                self.max_fields,
                LimitKind::Fields,
            )
            .map_err(|e| e.at_batch(index).at_role(atom.kind.role()))?;
            labels = add(
                labels,
                label_bytes(&atom.kind),
                self.max_label_bytes,
                LimitKind::LabelBytes,
            )
            .map_err(|e| e.at_batch(index).at_role(atom.kind.role()))?;
        }
        Ok(())
    }
}

pub(crate) struct ValidationBudget {
    remaining: usize,
}
impl ValidationBudget {
    pub fn new(limits: &GraphLimits) -> Self {
        Self {
            remaining: limits.max_validation_work,
        }
    }
    pub fn charge(&mut self, amount: usize) -> Result<()> {
        self.remaining = self.remaining.checked_sub(amount).ok_or_else(|| {
            Diagnostic::exhausted(LimitKind::ValidationWork, "validation work limit exceeded")
        })?;
        Ok(())
    }
    /// Reserve an upper bound for a complete outgoing closure traversal.
    pub fn closure(&mut self, graph: &Graph) -> Result<()> {
        self.charge(graph.len())?;
        self.charge(graph.field_count())
    }
}
