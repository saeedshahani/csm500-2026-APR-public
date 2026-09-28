//! Closed carrier observations and bounded exact isomorphism. These are not
//! executable configurations, and revision identity continuity is not tested.
use graph_core::{
    Atom, AtomKey, Diagnostic, ErrorKind, Graph, Kind, LimitKind, Result, ValidatedGraph,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub(crate) fn signature(&self) -> (usize, usize, u64) {
        (
            self.atoms.len(),
            self.atoms.values().map(|a| a.fields.len()).sum(),
            self.atoms.values().fold(0u64, |h, a| {
                h.wrapping_add(graph_core::shape_contribution(a))
            }),
        )
    }
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
    pub(crate) fn is_all(&self) -> bool {
        self.all
    }
    pub fn initial_inputs(&self) -> &BTreeMap<String, AtomKey> {
        &self.inputs
    }
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
    compare_budget(left, right, budget, false)
}

/// Exact comparison with a checked identity witness and incremental, reversible
/// field propagation. Work accounting is deliberately separate from the legacy
/// comparator; callers must not substitute this inside a legacy trace replay.
pub fn isomorphic_indexed(
    left: &Observation,
    right: &Observation,
    max_work: usize,
) -> Result<bool> {
    let mut budget = Budget::new(max_work, LimitKind::ComparisonWork);
    isomorphic_indexed_budget(left, right, &mut budget)
}

pub(crate) fn isomorphic_indexed_budget(
    left: &Observation,
    right: &Observation,
    budget: &mut Budget,
) -> Result<bool> {
    compare_budget(left, right, budget, true)
}

fn compare_budget(
    left: &Observation,
    right: &Observation,
    budget: &mut Budget,
    indexed: bool,
) -> Result<bool> {
    budget.charge(1)?;
    if left.atoms.len() != right.atoms.len() {
        return Ok(false);
    }
    if indexed {
        // Keys alone are NOT a witness. Check all kinds, attributes and ordered
        // endpoints; exact map equality certifies the identity bijection.
        budget.charge(left.retention_units())?;
        budget.charge(right.retention_units())?;
        if left.atoms == right.atoms {
            return Ok(true);
        }
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
    let mut initial_classes = BTreeMap::new();
    let mut colours = Vec::new();
    let mut fields = Vec::new();
    for (offset, atoms, indices) in [(0, &l, &li), (n, &r, &ri)] {
        for (_, atom) in atoms {
            budget.charge(1 + atom.fields.len())?;
            let label = format!("{:?}", atom.kind);
            budget.charge(label.len())?;
            let next = initial_classes.len();
            colours.push(
                *initial_classes
                    .entry((label, atom.fields.len()))
                    .or_insert(next),
            );
            fields.push(
                atom.fields
                    .iter()
                    .map(|k| indices[k] + offset)
                    .collect::<Vec<_>>(),
            );
        }
    }
    let Some(colours) = refine(colours, &fields, n, budget)? else {
        return Ok(false);
    };
    if indexed {
        return propagated(&l, &r, &fields, &colours, budget);
    }
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
        if used[candidate] || colours[depth] != colours[n + candidate] {
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

/// Each assigned pair forces its ordered field pairs. The trail records only
/// newly assigned pairs, so undo restores the exact preceding partial bijection.
/// Refinement only restricts domains; success still requires a total bijection.
fn propagated(
    left: &[(&AtomKey, &Atom)],
    right: &[(&AtomKey, &Atom)],
    fields: &[Vec<usize>],
    colours: &[usize],
    budget: &mut Budget,
) -> Result<bool> {
    use std::ops::Bound::{Excluded, Unbounded};
    let n = left.len();
    budget.charge(n.saturating_mul(4))?;
    let mut forward = vec![None; n];
    let mut backward = vec![None; n];
    let mut free: BTreeSet<_> = (0..n).collect();
    let mut available: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for i in 0..n {
        available.entry(colours[n + i]).or_default().insert(i);
    }
    struct Frame {
        node: usize,
        last: Option<usize>,
        start: usize,
    }
    let mut stack = vec![Frame {
        node: 0,
        last: None,
        start: 0,
    }];
    let mut trail: Vec<(usize, usize)> = Vec::new();
    while let Some(frame) = stack.last_mut() {
        while trail.len() > frame.start {
            budget.charge(1)?;
            let (a, b) = trail.pop().unwrap();
            forward[a] = None;
            backward[b] = None;
            free.insert(a);
            available.get_mut(&colours[n + b]).unwrap().insert(b);
        }
        budget.charge(1)?;
        let domain = &available[&colours[frame.node]];
        let candidate = match frame.last {
            None => domain.first().copied(),
            Some(last) => domain.range((Excluded(last), Unbounded)).next().copied(),
        };
        let Some(candidate) = candidate else {
            stack.pop();
            continue;
        };
        frame.last = Some(candidate);
        let mut pending = vec![(frame.node, candidate)];
        let mut valid = true;
        while let Some((a, b)) = pending.pop() {
            budget.charge(1)?;
            if let Some(old) = forward[a] {
                if old != b {
                    valid = false;
                    break;
                }
                continue;
            }
            budget.charge(
                1 + match &left[a].1.kind {
                    Kind::Constructor(s) | Kind::Record { label: Some(s), .. } => s.len(),
                    Kind::PatternVariable { admits, .. } => admits.len(),
                    _ => 0,
                },
            )?;
            if backward[b].is_some()
                || colours[a] != colours[n + b]
                || left[a].1.kind != right[b].1.kind
                || fields[a].len() != fields[n + b].len()
            {
                valid = false;
                break;
            }
            forward[a] = Some(b);
            backward[b] = Some(a);
            free.remove(&a);
            available.get_mut(&colours[n + b]).unwrap().remove(&b);
            trail.push((a, b));
            budget.charge(fields[a].len())?;
            pending.extend(
                fields[a]
                    .iter()
                    .zip(&fields[n + b])
                    .map(|(x, y)| (*x, *y - n)),
            );
        }
        if valid {
            let Some(node) = free.first().copied() else {
                return Ok(true);
            };
            stack.push(Frame {
                node,
                last: None,
                start: trail.len(),
            });
        }
    }
    Ok(false)
}

/// Necessary-condition partition refinement only. Incoming signatures are
/// accumulated in one incidence pass, independently of the test oracle's
/// all-pairs construction. Equal partitions still require exact bijection search.
fn refine(
    mut colours: Vec<usize>,
    fields: &[Vec<usize>],
    n: usize,
    budget: &mut Budget,
) -> Result<Option<Vec<usize>>> {
    loop {
        budget.charge(colours.len())?;
        let mut incoming = vec![Vec::new(); colours.len()];
        for (source, targets) in fields.iter().enumerate() {
            budget.charge(1)?;
            for (slot, target) in targets.iter().enumerate() {
                budget.charge(1)?;
                incoming[*target].push((slot, colours[source]));
            }
        }
        let mut classes = BTreeMap::new();
        let mut refined = Vec::new();
        for i in 0..colours.len() {
            let size = incoming[i].len();
            budget.charge(
                1 + fields[i].len()
                    + size.saturating_mul(usize::BITS as usize - size.leading_zeros() as usize),
            )?;
            incoming[i].sort_unstable();
            let signature = (
                colours[i],
                fields[i].iter().map(|j| colours[*j]).collect::<Vec<_>>(),
                std::mem::take(&mut incoming[i]),
            );
            let next = classes.len();
            refined.push(*classes.entry(signature).or_insert(next));
        }
        let mut balance = vec![0isize; classes.len()];
        for (i, c) in refined.iter().enumerate() {
            budget.charge(1)?;
            balance[*c] += if i < n { 1 } else { -1 };
        }
        if balance.iter().any(|v| *v != 0) {
            return Ok(None);
        }
        // Refinement cannot merge existing classes because their old colour is
        // part of every signature. An unchanged class count is a fixed partition.
        let old_count = colours.iter().copied().max().map_or(0, |c| c + 1);
        if classes.len() == old_count {
            return Ok(Some(refined));
        }
        colours = refined;
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
