//! Closed carrier observations and bounded exact isomorphism. These are not
//! executable configurations, and revision identity continuity is not tested.
use graph_core::{
    Atom, AtomKey, Diagnostic, ErrorKind, Graph, Kind, LimitKind, Result, ValidatedGraph,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct ObservationLimits {
    pub max_work: usize,
    pub max_comparison_work: usize,
    pub max_history_units: usize,
}
impl Default for ObservationLimits {
    fn default() -> Self {
        Self {
            max_work: 1_000_000,
            max_comparison_work: 1_000_000,
            max_history_units: 1_000_000,
        }
    }
}
pub(crate) struct Budget {
    remaining: usize,
    kind: LimitKind,
}
impl Budget {
    pub(crate) fn new(remaining: usize, kind: LimitKind) -> Self {
        Self { remaining, kind }
    }
    pub(crate) fn charge(&mut self, n: usize) -> Result<()> {
        self.remaining = self.remaining.checked_sub(n).ok_or_else(|| {
            Diagnostic::exhausted(self.kind, "observation/comparison budget exhausted")
        })?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Observation {
    atoms: BTreeMap<AtomKey, Atom>,
}
impl Observation {
    pub fn atoms(&self) -> &BTreeMap<AtomKey, Atom> {
        &self.atoms
    }
    pub fn retention_units(&self) -> usize {
        self.atoms.values().fold(1usize, |n, a| {
            n.saturating_add(1 + a.fields.len())
                .saturating_add(match &a.kind {
                    Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => s.len(),
                    Kind::PatternVariable { admits, .. } => admits.len(),
                    _ => 0,
                })
        })
    }
    /// Example criterion: distinct endpoints of non-self value links. This is
    /// an externally chosen measure, not an intrinsic definition of meaning.
    pub fn value_payload(&self) -> usize {
        self.atoms.values().filter(|a| matches!(&a.kind, Kind::Record { role: graph_core::Role::ObjectLink, label: Some(s) } if s == "value") && a.fields.len()==2 && a.fields[0]!=a.fields[1])
            .flat_map(|a| a.fields.iter().copied()).collect::<BTreeSet<_>>().len()
    }
}

#[derive(Clone, Debug)]
pub struct Observer {
    canonical: String,
    all: bool,
    data: bool,
    clauses: BTreeSet<(String, String)>,
    inputs: BTreeMap<String, AtomKey>,
}
fn malformed(message: &str) -> Diagnostic {
    Diagnostic::new(ErrorKind::MalformedInput, message)
}
fn decode_hex(s: &str) -> Result<String> {
    if s.len() % 2 != 0
        || !s
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(malformed("invalid lowercase UTF-8 hex label"));
    }
    let bytes = s
        .as_bytes()
        .chunks_exact(2)
        .map(|p| {
            let digit = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
            digit(p[0]) * 16 + digit(p[1])
        })
        .collect();
    String::from_utf8(bytes).map_err(|_| malformed("label is not UTF-8"))
}
impl Observer {
    pub fn all() -> Self {
        Self {
            canonical: "observer-v1\nall\n".into(),
            all: true,
            data: false,
            clauses: BTreeSet::new(),
            inputs: BTreeMap::new(),
        }
    }
    pub fn parse(
        text: &str,
        mut resolve: impl FnMut(&str) -> Option<AtomKey>,
        max_bytes: usize,
    ) -> Result<Self> {
        if text.len() > max_bytes {
            return Err(Diagnostic::exhausted(
                LimitKind::InputBytes,
                "observer declaration exceeds byte bound",
            ));
        }
        if !text.ends_with('\n') || text.contains('\r') {
            return Err(malformed("observer requires LF and a final LF"));
        }
        let mut lines = text[..text.len() - 1].split('\n');
        if lines.next() != Some("observer-v1") {
            return Err(malformed("unsupported observer schema"));
        }
        let scope = lines
            .next()
            .ok_or_else(|| malformed("missing observer scope"))?;
        if scope == "all" {
            if lines.next().is_some() {
                return Err(malformed("all takes no clauses"));
            }
            return Ok(Self::all());
        }
        if scope != "select carrier" && scope != "select data" {
            return Err(malformed("invalid observer scope"));
        }
        let mut result = Self {
            canonical: String::new(),
            all: false,
            data: scope == "select data",
            clauses: BTreeSet::new(),
            inputs: BTreeMap::new(),
        };
        for line in lines {
            let (selector, value) = line
                .split_once(' ')
                .ok_or_else(|| malformed("invalid observer clause"))?;
            match selector {
                "kind"
                    if [
                        "Constructor",
                        "PatternVariable",
                        "RuleHead",
                        "Nat",
                        "Record",
                    ]
                    .contains(&value) => {}
                "role"
                    if [
                        "ObjectLink",
                        "Lhs",
                        "Rhs",
                        "Active",
                        "Rank",
                        "ModeObject",
                        "ModeMeta",
                        "Revision",
                    ]
                    .contains(&value) => {}
                "constructor" | "link" => {
                    decode_hex(value)?;
                }
                "input" => {
                    if value.is_empty()
                        || !value.bytes().enumerate().all(|(i, c)| {
                            c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                        })
                    {
                        return Err(malformed("invalid input name"));
                    }
                    result.inputs.insert(
                        value.into(),
                        resolve(value).ok_or_else(|| malformed("unresolved observer input"))?,
                    );
                }
                _ => return Err(malformed("unknown observer selector or value")),
            }
            if !result.clauses.insert((selector.into(), value.into())) {
                return Err(malformed("duplicate observer clause"));
            }
        }
        result.canonical = format!("observer-v1\n{scope}\n");
        for (selector, value) in &result.clauses {
            result.canonical += &format!("{selector} {value}\n");
        }
        Ok(result)
    }
    pub fn canonical(&self) -> &str {
        &self.canonical
    }
    /// Caller-supplied initial input identity plus this canonical declaration
    /// and frozen resolution map identify an experiment's observer.
    pub fn identity(&self) -> String {
        format!("{:?}; inputs={:?}", self.canonical, self.inputs)
    }
    pub fn project(
        &self,
        view: &ValidatedGraph<'_>,
        limits: ObservationLimits,
    ) -> Result<Observation> {
        let graph = view.graph();
        let mut budget = Budget::new(limits.max_work, LimitKind::ObservationWork);
        budget.charge(1)?;
        let mut hidden = BTreeSet::new();
        if self.data {
            for rule in view.live_rules() {
                for key in &rule.representation {
                    budget.charge(1)?;
                    hidden.insert(*key);
                }
            }
        }
        let mut seeds = BTreeSet::new();
        for (key, atom) in graph.iter() {
            budget.charge(1)?;
            if self.all {
                seeds.insert(key);
                continue;
            }
            if hidden.contains(&key) {
                continue;
            }
            for (selector, value) in &self.clauses {
                budget.charge(1 + value.len())?;
                let matches = match selector.as_str() {
                    "kind" => match &atom.kind {
                        Kind::Constructor(_) => value == "Constructor",
                        Kind::PatternVariable { .. } => value == "PatternVariable",
                        Kind::RuleHead => value == "RuleHead",
                        Kind::Nat => value == "Nat",
                        Kind::Record { .. } => value == "Record",
                    },
                    "role" => atom.kind.role().is_some_and(|r| format!("{r:?}") == *value),
                    "constructor" => {
                        matches!(&atom.kind,Kind::Constructor(s) if *s==decode_hex(value)?)
                    }
                    "link" => {
                        matches!(&atom.kind,Kind::Record{role:graph_core::Role::ObjectLink,label:Some(s)} if *s==decode_hex(value)?)
                    }
                    "input" => self.inputs.get(value) == Some(&key),
                    _ => false,
                };
                if matches {
                    seeds.insert(key);
                    break;
                }
            }
        }
        let mut pending: Vec<_> = seeds.iter().copied().collect();
        while let Some(key) = pending.pop() {
            for target in &graph.atom(key)?.fields {
                budget.charge(1)?;
                if seeds.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        let mut atoms = BTreeMap::new();
        for key in seeds {
            let atom = graph.atom(key)?;
            budget.charge(
                1 + atom.fields.len()
                    + match &atom.kind {
                        Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => s.len(),
                        Kind::PatternVariable { admits, .. } => admits.len(),
                        _ => 0,
                    },
            )?;
            atoms.insert(key, atom.clone());
        }
        Ok(Observation { atoms })
    }
}

/// Exact iterative bijection search; no hash equality is accepted as evidence.
/// Attribute scans, pair trials and field consistency checks consume one shared
/// budget. This deliberately simple implementation has exponential worst case.
pub fn isomorphic(left: &Observation, right: &Observation, max_work: usize) -> Result<bool> {
    let mut budget = Budget::new(max_work, LimitKind::ComparisonWork);
    isomorphic_budget(left, right, &mut budget)
}
pub(crate) fn isomorphic_budget(
    left: &Observation,
    right: &Observation,
    budget: &mut Budget,
) -> Result<bool> {
    budget.charge(1)?;
    if left.atoms.len() != right.atoms.len() {
        return Ok(false);
    }
    budget.charge(left.atoms.len())?;
    budget.charge(right.atoms.len())?;
    budget.charge(left.retention_units())?;
    budget.charge(right.retention_units())?;
    let mut l: Vec<_> = left.atoms.iter().collect();
    // Field-bearing atoms constrain several identities at once. Precharge a
    // conservative n log2(n) sorting allowance; this is not a timing estimate.
    budget.charge(
        l.len()
            .saturating_mul(usize::BITS as usize - l.len().leading_zeros() as usize),
    )?;
    l.sort_by_key(|(key, atom)| (std::cmp::Reverse(atom.fields.len()), **key));
    let r: Vec<_> = right.atoms.iter().collect();
    let n = l.len();
    if n == 0 {
        return Ok(true);
    }
    let li: BTreeMap<_, _> = l.iter().enumerate().map(|(i, (k, _))| (**k, i)).collect();
    let ri: BTreeMap<_, _> = r.iter().enumerate().map(|(i, (k, _))| (**k, i)).collect();
    let mut map = vec![None; n];
    let mut used = vec![false; n];
    let mut next = vec![0; n];
    let mut depth = 0;
    loop {
        if depth == n {
            return Ok(true);
        }
        if next[depth] == n {
            next[depth] = 0;
            if depth == 0 {
                return Ok(false);
            }
            depth -= 1;
            used[map[depth].take().unwrap()] = false;
            continue;
        }
        let candidate = next[depth];
        next[depth] += 1;
        budget.charge(1)?;
        if used[candidate] {
            continue;
        }
        let a = l[depth].1;
        let b = r[candidate].1;
        budget.charge(
            1 + match &a.kind {
                Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => s.len(),
                Kind::PatternVariable { admits, .. } => admits.len(),
                _ => 0,
            },
        )?;
        if a.kind != b.kind || a.fields.len() != b.fields.len() {
            continue;
        }
        map[depth] = Some(candidate);
        let mut valid = true;
        let mut forward = BTreeMap::new();
        let mut backward = BTreeMap::new();
        for i in 0..=depth {
            budget.charge(1)?;
            let j = map[i].unwrap();
            if forward.insert(i, j).is_some_and(|old| old != j)
                || backward.insert(j, i).is_some_and(|old| old != i)
            {
                valid = false;
                break;
            }
            for (x, y) in l[i].1.fields.iter().zip(&r[j].1.fields) {
                budget.charge(1)?;
                let x = li[x];
                let y = ri[y];
                if map[x].is_some_and(|target| target != y)
                    || forward.insert(x, y).is_some_and(|old| old != y)
                    || backward.insert(y, x).is_some_and(|old| old != x)
                {
                    valid = false;
                    break;
                }
            }
            if !valid {
                break;
            }
        }
        if valid {
            used[candidate] = true;
            depth += 1;
        } else {
            map[depth] = None;
        }
    }
}

/// Observe a validated whole graph without imposing configuration validity on
/// the resulting projection.
pub fn all(
    graph: &Graph,
    limits: graph_core::GraphLimits,
    observation: ObservationLimits,
) -> Result<Observation> {
    Observer::all().project(&ValidatedGraph::new(graph, &limits)?, observation)
}
