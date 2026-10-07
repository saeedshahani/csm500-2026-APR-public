//! Closed lambda terms, encoded as a graph-resident call-by-name environment machine.
//!
//! This adapter constructs syntax and rules; it never executes beta reduction.
//! The source arena uses zero-based de Bruijn indices. See `docs/formalism.md`
//! for the invariant, alias cases, readback and correspondence argument.
//!
//! ```
//! use parser::lambda::{Term, Node, Limits, encode, readback};
//! let limits = Limits::default();
//! let identity = Term::new(vec![Node::Var(0), Node::Lam(0)], 1, limits)?;
//! let encoded = encode(&identity, limits)?;
//! let term = readback(&encoded.graph, encoded.root, limits)?;
//! assert_eq!(term.display(limits)?, "(λ.0)");
//! # Ok::<(), parser::lambda::Error>(())
//! ```
use graph_core::{AtomKey, Binding, Graph, Kind, Role, Sort};
use std::collections::{BTreeMap, BTreeSet};

pub const CLOSED_PROFILE: &str = "closed-cbn-v1";
pub const OPEN_PROFILE: &str = "open-cbn-v1";

/// Shared version-dispatched entry point for native and WebAssembly clients.
pub fn compile_profile(
    text: &str,
    profile: &str,
    free_names: Vec<String>,
    limits: Limits,
) -> Result<Encoded, Error> {
    match profile {
        CLOSED_PROFILE if free_names.is_empty() => encode(&parse(text, limits)?, limits),
        OPEN_PROFILE => encode_open(&parse_open(text, free_names, limits)?, limits),
        CLOSED_PROFILE => Err(Error::InvalidSource(
            "closed profile cannot declare free names",
        )),
        _ => Err(Error::InvalidSource("unknown lambda profile")),
    }
}

/// An open term in an explicit ordered context. Indices beyond local binders
/// address this context; names are symbols, never executable external values.
#[derive(Clone, Debug)]
pub struct OpenTerm {
    term: Term,
    free_names: Vec<String>,
}

fn check_names(names: &[String], limits: Limits) -> Result<(), Error> {
    if names.len() > limits.max_nodes
        || names
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.len()))
            .is_none_or(|n| n > limits.max_work)
    {
        return Err(Error::ResourceLimit);
    }
    let mut seen = BTreeSet::new();
    for name in names {
        if !name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || !seen.insert(name)
        {
            return Err(Error::InvalidSource("invalid or duplicate free name"));
        }
    }
    Ok(())
}

impl OpenTerm {
    pub fn new(
        nodes: Vec<Node>,
        root: usize,
        free_names: Vec<String>,
        limits: Limits,
    ) -> Result<Self, Error> {
        check_names(&free_names, limits)?;
        Ok(Self {
            term: Term::scoped(nodes, root, free_names.len(), limits)?,
            free_names,
        })
    }
    pub fn nodes(&self) -> &[Node] {
        self.term.nodes()
    }
    pub fn root(&self) -> usize {
        self.term.root()
    }
    pub fn free_names(&self) -> &[String] {
        &self.free_names
    }
    /// Bound occurrences use indices; unresolved inputs retain their names.
    pub fn display(&self, limits: Limits) -> Result<String, Error> {
        self.term.display_scoped(&self.free_names, limits)
    }
}

/// Source arena nodes; children must precede their parent. Shared syntax is allowed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Var(usize),
    Lam(usize),
    App(usize, usize),
}

/// A validated closed term. Arena identity is not source-term equality.
#[derive(Clone, Debug)]
pub struct Term {
    nodes: Vec<Node>,
    root: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidSource(&'static str),
    InvalidGraph(&'static str),
    ResourceLimit,
    Core(graph_core::Diagnostic),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSource(message) => write!(f, "invalid lambda source: {message}"),
            Self::InvalidGraph(message) => write!(f, "invalid lambda readback: {message}"),
            Self::ResourceLimit => f.write_str("lambda adapter resource limit"),
            Self::Core(error) => write!(f, "{error}"),
        }
    }
}
impl std::error::Error for Error {}

impl From<graph_core::Diagnostic> for Error {
    fn from(value: graph_core::Diagnostic) -> Self {
        Self::Core(value)
    }
}

/// Independent adapter bounds, not a mathematical bound on the source calculus.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_work: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_nodes: 4096,
            max_depth: 128,
            max_work: 200_000,
        }
    }
}

fn spend(work: &mut usize) -> Result<(), Error> {
    *work = work.checked_sub(1).ok_or(Error::ResourceLimit)?;
    Ok(())
}

/// Read fully parenthesised named terms: `x`, `(\\x. term)`, `(term term)`.
/// Names are resolved to the nearest binder; free names are rejected.
pub fn parse(text: &str, limits: Limits) -> Result<Term, Error> {
    parse_scoped(text, &[], limits)
}

pub fn parse_open(text: &str, free_names: Vec<String>, limits: Limits) -> Result<OpenTerm, Error> {
    check_names(&free_names, limits)?;
    Ok(OpenTerm {
        term: parse_scoped(text, &free_names, limits)?,
        free_names,
    })
}

fn parse_scoped(text: &str, free_names: &[String], limits: Limits) -> Result<Term, Error> {
    if text.len() > limits.max_work || limits.max_depth > 128 {
        return Err(Error::ResourceLimit);
    }
    struct Reader<'a> {
        text: &'a [u8],
        at: usize,
        nodes: Vec<Node>,
        binders: Vec<String>,
        free_names: &'a [String],
        limits: Limits,
    }
    impl Reader<'_> {
        fn space(&mut self) {
            while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
                self.at += 1;
            }
        }
        fn take(&mut self, token: u8) -> Result<(), Error> {
            self.space();
            if self.text.get(self.at) != Some(&token) {
                return Err(Error::InvalidSource(
                    "expected parenthesis, backslash or dot",
                ));
            }
            self.at += 1;
            Ok(())
        }
        fn name(&mut self) -> Result<String, Error> {
            self.space();
            let start = self.at;
            while self
                .text
                .get(self.at)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                self.at += 1;
            }
            if start == self.at || !self.text[start].is_ascii_alphabetic() {
                return Err(Error::InvalidSource("expected an ASCII variable name"));
            }
            Ok(String::from_utf8(self.text[start..self.at].to_vec()).unwrap())
        }
        fn term(&mut self, depth: usize) -> Result<usize, Error> {
            if depth > self.limits.max_depth || self.nodes.len() >= self.limits.max_nodes {
                return Err(Error::ResourceLimit);
            }
            self.space();
            let node = if self.text.get(self.at) == Some(&b'(') {
                self.at += 1;
                self.space();
                let node = if self.text.get(self.at) == Some(&b'\\') {
                    self.at += 1;
                    let name = self.name()?;
                    self.take(b'.')?;
                    self.binders.push(name);
                    let body = self.term(depth + 1)?;
                    self.binders.pop();
                    Node::Lam(body)
                } else {
                    let f = self.term(depth + 1)?;
                    let a = self.term(depth + 1)?;
                    Node::App(f, a)
                };
                self.take(b')')?;
                node
            } else {
                let name = self.name()?;
                Node::Var(
                    self.binders
                        .iter()
                        .rev()
                        .position(|n| n == &name)
                        .or_else(|| {
                            self.free_names
                                .iter()
                                .position(|n| n == &name)
                                .map(|i| self.binders.len() + i)
                        })
                        .ok_or(Error::InvalidSource("free variable"))?,
                )
            };
            if self.nodes.len() >= self.limits.max_nodes {
                return Err(Error::ResourceLimit);
            }
            let id = self.nodes.len();
            self.nodes.push(node);
            Ok(id)
        }
    }
    let mut reader = Reader {
        text: text.as_bytes(),
        at: 0,
        nodes: Vec::new(),
        binders: Vec::new(),
        free_names,
        limits,
    };
    let root = reader.term(0)?;
    reader.space();
    if reader.at != text.len() {
        return Err(Error::InvalidSource("trailing input"));
    }
    Term::scoped(reader.nodes, root, free_names.len(), limits)
}

/// Interpret a uniquely identified, unquoted machine at an inspected state.
/// This is bounded readback, not a certificate of the machine invariant.
pub fn interpret(graph: &Graph, limits: Limits) -> Result<Option<String>, Error> {
    let configuration = graph_core::validate(graph)?;
    let representation: BTreeSet<_> = configuration
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    let roots: Vec<_> = graph
        .iter()
        .filter(|(id, a)| {
            !representation.contains(id) && a.kind == Kind::Constructor("lc.Machine".into())
        })
        .map(|(id, _)| id)
        .collect();
    match roots.as_slice() {
        [] => Ok(None),
        [root] => {
            let open = graph.iter().any(|(id, a)| {
                !representation.contains(&id)
                    && a.kind
                        == Kind::Record {
                            role: Role::ObjectLink,
                            label: Some("lc.profile".into()),
                        }
                    && a.fields[0] == *root
            });
            Ok(Some(if open {
                readback_open(graph, *root, limits)?.display(limits)?
            } else {
                readback(graph, *root, limits)?.display(limits)?
            }))
        }
        _ => Err(Error::InvalidGraph("multiple lambda machines")),
    }
}

impl Term {
    pub fn new(nodes: Vec<Node>, root: usize, limits: Limits) -> Result<Self, Error> {
        Self::scoped(nodes, root, 0, limits)
    }
    fn scoped(
        nodes: Vec<Node>,
        root: usize,
        free_count: usize,
        limits: Limits,
    ) -> Result<Self, Error> {
        if nodes.len() > limits.max_nodes || limits.max_depth > 128 {
            return Err(Error::ResourceLimit);
        }
        if root >= nodes.len() {
            return Err(Error::InvalidSource("missing root"));
        }
        for (i, node) in nodes.iter().enumerate() {
            let valid = match node {
                Node::Var(_) => true,
                Node::Lam(b) => *b < i,
                Node::App(f, a) => *f < i && *a < i,
            };
            if !valid {
                return Err(Error::InvalidSource("children must precede parent"));
            }
        }
        let mut work = limits.max_work;
        let mut todo = vec![(root, 0, 0)];
        let mut reached = BTreeSet::new();
        while let Some((id, binders, depth)) = todo.pop() {
            spend(&mut work)?;
            if depth > limits.max_depth {
                return Err(Error::ResourceLimit);
            }
            reached.insert(id);
            match nodes[id] {
                Node::Var(n) if n >= binders + free_count => {
                    return Err(Error::InvalidSource("free variable"))
                }
                Node::Var(_) => (),
                Node::Lam(b) => todo.push((b, binders + 1, depth + 1)),
                Node::App(f, a) => {
                    todo.push((f, binders, depth + 1));
                    todo.push((a, binders, depth + 1));
                }
            }
        }
        if reached.len() != nodes.len() {
            return Err(Error::InvalidSource("unreachable source node"));
        }
        Ok(Self { nodes, root })
    }
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    pub fn root(&self) -> usize {
        self.root
    }

    /// Canonical nameless syntax, independent of arena sharing and allocation order.
    pub fn display(&self, limits: Limits) -> Result<String, Error> {
        self.display_scoped(&[], limits)
    }
    fn display_scoped(&self, names: &[String], limits: Limits) -> Result<String, Error> {
        fn go(
            t: &Term,
            id: usize,
            depth: usize,
            scope: (usize, &[String]),
            work: &mut usize,
            limits: Limits,
            out: &mut String,
        ) -> Result<(), Error> {
            let (binders, names) = scope;
            spend(work)?;
            if depth > limits.max_depth || depth > 128 {
                return Err(Error::ResourceLimit);
            }
            match t.nodes[id] {
                Node::Var(n) => {
                    if n < binders {
                        out.push_str(&n.to_string());
                    } else {
                        out.push_str(
                            names
                                .get(n - binders)
                                .ok_or(Error::InvalidSource("free variable"))?,
                        );
                    }
                }
                Node::Lam(b) => {
                    out.push_str("(λ.");
                    go(t, b, depth + 1, (binders + 1, names), work, limits, out)?;
                    out.push(')');
                }
                Node::App(f, a) => {
                    out.push('(');
                    go(t, f, depth + 1, scope, work, limits, out)?;
                    out.push(' ');
                    go(t, a, depth + 1, scope, work, limits, out)?;
                    out.push(')');
                }
            }
            Ok(())
        }
        let mut out = String::new();
        let mut work = limits.max_work;
        go(self, self.root, 0, (0, names), &mut work, limits, &mut out)?;
        Ok(out)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Push,
    Beta,
    LookupZero,
    LookupSucc,
}

pub struct Encoded {
    pub graph: Graph,
    pub root: AtomKey,
    /// Descriptive classification only: execution decodes rules from the graph.
    pub rules: BTreeMap<AtomKey, Transition>,
}

fn ctor(g: &mut Graph, name: &str) -> Result<AtomKey, Error> {
    Ok(g.insert(Kind::Constructor(format!("lc.{name}")), &[])?)
}
fn edge(g: &mut Graph, name: &str, from: AtomKey, to: AtomKey) -> Result<AtomKey, Error> {
    Ok(g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(format!("lc.{name}")),
        },
        &[from, to],
    )?)
}

/// Compile without source evaluation. Eleven fixed alias-specialised graph rules.
pub fn encode(term: &Term, limits: Limits) -> Result<Encoded, Error> {
    encode_scoped(term, None, limits)
}

/// Compile open weak-head call-by-name without changing the eleven resident rules.
/// The explicit context and encoding version remain in the graph and its trace.
pub fn encode_open(term: &OpenTerm, limits: Limits) -> Result<Encoded, Error> {
    check_names(&term.free_names, limits)?;
    encode_scoped(&term.term, Some(&term.free_names), limits)
}

fn encode_scoped(term: &Term, names: Option<&[String]>, limits: Limits) -> Result<Encoded, Error> {
    // Recheck caller bounds even if the Term was validated with larger allowances.
    if term.nodes.len() > limits.max_nodes {
        return Err(Error::ResourceLimit);
    }
    Term::scoped(
        term.nodes.clone(),
        term.root,
        names.map_or(0, <[String]>::len),
        limits,
    )?;
    let mut graph = Graph::new();
    let rules = machine_rules(&mut graph)?;
    let mut ids = Vec::new();
    let mut work = limits.max_work;
    for node in &term.nodes {
        spend(&mut work)?;
        let id = match *node {
            Node::Var(n) => {
                let mut id = ctor(&mut graph, "Z")?;
                for _ in 0..n {
                    spend(&mut work)?;
                    let next = ctor(&mut graph, "S")?;
                    edge(&mut graph, "pred", next, id)?;
                    id = next;
                }
                id
            }
            Node::Lam(b) => {
                let id = ctor(&mut graph, "Lam")?;
                edge(&mut graph, "body", id, ids[b])?;
                id
            }
            Node::App(f, a) => {
                let id = ctor(&mut graph, "App")?;
                edge(&mut graph, "fun", id, ids[f])?;
                edge(&mut graph, "arg", id, ids[a])?;
                id
            }
        };
        ids.push(id);
    }
    let root = ctor(&mut graph, "Machine")?;
    let nil_env = ctor(&mut graph, "NilEnv")?;
    let mut env = nil_env;
    let stack = ctor(&mut graph, "NilStack")?;
    if let Some(names) = names {
        let profile = ctor(
            &mut graph,
            &format!("Profile:{OPEN_PROFILE}:{}", names.join(",")),
        )?;
        edge(&mut graph, "profile", root, profile)?;
        for name in names.iter().rev() {
            spend(&mut work)?;
            let symbol = ctor(&mut graph, &format!("Free:{name}"))?;
            let next = ctor(&mut graph, "Env")?;
            edge(&mut graph, "term", next, symbol)?;
            edge(&mut graph, "env", next, nil_env)?;
            edge(&mut graph, "tail", next, env)?;
            env = next;
        }
    }
    edge(&mut graph, "term", root, ids[term.root])?;
    edge(&mut graph, "env", root, env)?;
    edge(&mut graph, "stack", root, stack)?;
    graph_core::validate(&graph)?;
    Ok(Encoded { graph, root, rules })
}

/// Template construction records every LHS atom explicitly; only control records
/// are consumed. Persistent syntax, frames and environments are never deleted.
struct Pattern<'a> {
    g: &'a mut Graph,
    lhs: BTreeSet<AtomKey>,
    removed: BTreeSet<AtomKey>,
    added: BTreeSet<AtomKey>,
}
impl<'a> Pattern<'a> {
    fn new(g: &'a mut Graph) -> Self {
        Self {
            g,
            lhs: BTreeSet::new(),
            removed: BTreeSet::new(),
            added: BTreeSet::new(),
        }
    }
    fn node(&mut self, label: Option<&str>) -> Result<AtomKey, Error> {
        let id = match label {
            Some(s) => ctor(self.g, s)?,
            None => self
                .g
                .insert(Kind::variable(Sort::Constructor, Binding::Free), &[])?,
        };
        self.lhs.insert(id);
        Ok(id)
    }
    fn link(&mut self, name: &str, a: AtomKey, b: AtomKey, remove: bool) -> Result<(), Error> {
        let id = edge(self.g, name, a, b)?;
        self.lhs.insert(id);
        if remove {
            self.removed.insert(id);
        }
        Ok(())
    }
    fn fresh(&mut self, label: &str) -> Result<AtomKey, Error> {
        let id = ctor(self.g, label)?;
        self.added.insert(id);
        Ok(id)
    }
    fn out(&mut self, name: &str, a: AtomKey, b: AtomKey) -> Result<(), Error> {
        let id = edge(self.g, name, a, b)?;
        self.added.insert(id);
        Ok(())
    }
    fn finish(self) -> Result<AtomKey, Error> {
        let h = self.g.insert(Kind::RuleHead, &[])?;
        let n = self.g.insert(Kind::Nat, &[])?;
        for (role, fields) in [
            (Role::Rank, vec![h, n]),
            (Role::Revision, vec![h]),
            (Role::ModeObject, vec![h]),
            (Role::Active, vec![h]),
        ] {
            self.g.insert(Kind::record(role), &fields)?;
        }
        for id in &self.lhs {
            self.g.insert(Kind::record(Role::Lhs), &[h, *id])?;
        }
        for id in self.lhs.difference(&self.removed).chain(&self.added) {
            self.g.insert(Kind::record(Role::Rhs), &[h, *id])?;
        }
        Ok(h)
    }
}

fn machine_rules(g: &mut Graph) -> Result<BTreeMap<AtomKey, Transition>, Error> {
    let mut rules = BTreeMap::new();
    // App's two children can share a syntax identity, but neither is its parent.
    for same in [false, true] {
        let mut p = Pattern::new(g);
        let m = p.node(Some("Machine"))?;
        let t = p.node(Some("App"))?;
        let f = p.node(None)?;
        let a = if same { f } else { p.node(None)? };
        let e = p.node(None)?;
        let s = p.node(None)?;
        p.link("term", m, t, true)?;
        p.link("env", m, e, false)?;
        p.link("stack", m, s, true)?;
        p.link("fun", t, f, false)?;
        p.link("arg", t, a, false)?;
        let k = p.fresh("Stack")?;
        p.out("term", k, a)?;
        p.out("env", k, e)?;
        p.out("tail", k, s)?;
        p.out("term", m, f)?;
        p.out("stack", m, k)?;
        rules.insert(p.finish()?, Transition::Push);
    }
    // Argument term can coincide with lambda/body; the captured environments can coincide.
    for alias in 0..3 {
        for same_env in [false, true] {
            let mut p = Pattern::new(g);
            let m = p.node(Some("Machine"))?;
            let t = p.node(Some("Lam"))?;
            let b = p.node(None)?;
            let a = match alias {
                1 => t,
                2 => b,
                _ => p.node(None)?,
            };
            let e = p.node(None)?;
            let f = if same_env { e } else { p.node(None)? };
            let k = p.node(Some("Stack"))?;
            let s = p.node(None)?;
            p.link("term", m, t, true)?;
            p.link("env", m, e, true)?;
            p.link("stack", m, k, true)?;
            p.link("body", t, b, false)?;
            p.link("term", k, a, false)?;
            p.link("env", k, f, false)?;
            p.link("tail", k, s, false)?;
            let next = p.fresh("Env")?;
            p.out("term", next, a)?;
            p.out("env", next, f)?;
            p.out("tail", next, e)?;
            p.out("term", m, b)?;
            p.out("env", m, next)?;
            p.out("stack", m, s)?;
            rules.insert(p.finish()?, Transition::Beta);
        }
    }
    for same_term in [false, true] {
        let mut p = Pattern::new(g);
        let m = p.node(Some("Machine"))?;
        let t = p.node(Some("Z"))?;
        let e = p.node(Some("Env"))?;
        let a = if same_term { t } else { p.node(None)? };
        let f = p.node(None)?;
        p.link("term", m, t, true)?;
        p.link("env", m, e, true)?;
        p.link("term", e, a, false)?;
        p.link("env", e, f, false)?;
        p.out("term", m, a)?;
        p.out("env", m, f)?;
        rules.insert(p.finish()?, Transition::LookupZero);
    }
    let mut p = Pattern::new(g);
    let m = p.node(Some("Machine"))?;
    let t = p.node(Some("S"))?;
    let n = p.node(None)?;
    let e = p.node(Some("Env"))?;
    let tail = p.node(None)?;
    p.link("term", m, t, true)?;
    p.link("env", m, e, true)?;
    p.link("pred", t, n, false)?;
    p.link("tail", e, tail, false)?;
    p.out("term", m, n)?;
    p.out("env", m, tail)?;
    rules.insert(p.finish()?, Transition::LookupSucc);
    Ok(rules)
}

struct Reader<'a> {
    graph: &'a Graph,
    links: BTreeMap<(AtomKey, String), AtomKey>,
    nodes: Vec<Node>,
    work: usize,
    limits: Limits,
    free_names: Vec<String>,
}
impl Reader<'_> {
    fn label(&mut self, id: AtomKey) -> Result<&str, Error> {
        spend(&mut self.work)?;
        match self.graph.get(id).map(|a| &a.kind) {
            Some(Kind::Constructor(s)) => Ok(s.as_str()),
            _ => Err(Error::InvalidGraph("expected constructor")),
        }
    }
    fn link(&mut self, id: AtomKey, name: &str) -> Result<AtomKey, Error> {
        spend(&mut self.work)?;
        self.links
            .get(&(id, format!("lc.{name}")))
            .copied()
            .ok_or(Error::InvalidGraph("missing field"))
    }
    fn add(&mut self, node: Node) -> Result<usize, Error> {
        if self.nodes.len() >= self.limits.max_nodes {
            return Err(Error::ResourceLimit);
        }
        let id = self.nodes.len();
        self.nodes.push(node);
        Ok(id)
    }
    fn term(
        &mut self,
        t: AtomKey,
        e: AtomKey,
        binders: usize,
        depth: usize,
        lift: usize,
    ) -> Result<usize, Error> {
        if depth > self.limits.max_depth || depth > 128 {
            return Err(Error::ResourceLimit);
        }
        match self.label(t)? {
            "lc.Lam" => {
                let b = self.link(t, "body")?;
                let b = self.term(b, e, binders + 1, depth + 1, lift)?;
                self.add(Node::Lam(b))
            }
            "lc.App" => {
                let f = self.link(t, "fun")?;
                let a = self.link(t, "arg")?;
                let f = self.term(f, e, binders, depth + 1, lift)?;
                let a = self.term(a, e, binders, depth + 1, lift)?;
                self.add(Node::App(f, a))
            }
            "lc.Z" | "lc.S" => {
                let mut n = 0;
                let mut v = t;
                loop {
                    match self.label(v)? {
                        "lc.Z" => break,
                        "lc.S" => {
                            n += 1;
                            v = self.link(v, "pred")?;
                        }
                        _ => return Err(Error::InvalidGraph("invalid index")),
                    }
                }
                if n < binders {
                    return self.add(Node::Var(n));
                }
                let mut env = e;
                for _ in 0..n - binders {
                    if self.label(env)? != "lc.Env" {
                        return Err(Error::InvalidGraph("unbound index"));
                    }
                    env = self.link(env, "tail")?;
                }
                if self.label(env)? != "lc.Env" {
                    return Err(Error::InvalidGraph("unbound index"));
                }
                let a = self.link(env, "term")?;
                let f = self.link(env, "env")?;
                // A captured closure is closed relative to the named context.
                // Lift only its free occurrences, not its locally bound indices.
                self.term(a, f, 0, depth + 1, lift + binders)
            }
            _ => {
                let label = self.label(t)?.to_owned();
                let index = label
                    .strip_prefix("lc.Free:")
                    .and_then(|name| self.free_names.iter().position(|n| n == name))
                    .ok_or(Error::InvalidGraph("not lambda syntax or declared symbol"))?;
                self.add(Node::Var(index + binders + lift))
            }
        }
    }
}

/// Reify the complete current weak-head expression, including pending arguments.
/// Not a beta evaluator, a normal-form test, or validation of the simulation invariant.
/// The correspondence applies to reachable encoded states; arbitrary graphs can fail.
pub fn readback(graph: &Graph, root: AtomKey, limits: Limits) -> Result<Term, Error> {
    Ok(readback_scoped(graph, root, limits, false)?.term)
}

pub fn readback_open(graph: &Graph, root: AtomKey, limits: Limits) -> Result<OpenTerm, Error> {
    readback_scoped(graph, root, limits, true)
}

fn readback_scoped(
    graph: &Graph,
    root: AtomKey,
    limits: Limits,
    open: bool,
) -> Result<OpenTerm, Error> {
    if limits.max_depth > 128 {
        return Err(Error::ResourceLimit);
    }
    let mut reader = Reader {
        graph,
        links: BTreeMap::new(),
        nodes: Vec::new(),
        work: limits.max_work,
        limits,
        free_names: Vec::new(),
    };
    // Rule descriptions contain both old and new control fields. They are not
    // live machine records and must not participate in source readback.
    if graph.len() > limits.max_work {
        return Err(Error::ResourceLimit);
    }
    let validation = graph_core::validate(graph)?;
    let hidden: BTreeSet<_> = validation
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    for (id, atom) in graph.iter() {
        spend(&mut reader.work)?;
        if hidden.contains(&id) {
            continue;
        }
        if let Kind::Record {
            role: Role::ObjectLink,
            label: Some(label),
        } = &atom.kind
        {
            if label.starts_with("lc.")
                && reader
                    .links
                    .insert((atom.fields[0], label.clone()), atom.fields[1])
                    .is_some()
            {
                return Err(Error::InvalidGraph("ambiguous field"));
            }
        }
    }
    if reader.label(root)? != "lc.Machine" {
        return Err(Error::InvalidGraph("not machine root"));
    }
    let profile = reader.links.get(&(root, "lc.profile".into())).copied();
    match (open, profile) {
        (true, Some(id)) => {
            let label = reader.label(id)?;
            let names = label
                .strip_prefix("lc.Profile:open-cbn-v1:")
                .ok_or(Error::InvalidGraph("unknown lambda profile"))?;
            reader.free_names = if names.is_empty() {
                Vec::new()
            } else {
                names.split(',').map(str::to_owned).collect()
            };
            check_names(&reader.free_names, limits)?;
        }
        (false, None) => (),
        _ => return Err(Error::InvalidGraph("lambda profile mismatch")),
    }
    let t = reader.link(root, "term")?;
    let e = reader.link(root, "env")?;
    let mut s = reader.link(root, "stack")?;
    let mut result = reader.term(t, e, 0, 0, 0)?;
    loop {
        match reader.label(s)? {
            "lc.NilStack" => break,
            "lc.Stack" => {
                let a = reader.link(s, "term")?;
                let f = reader.link(s, "env")?;
                let a = reader.term(a, f, 0, 0, 0)?;
                result = reader.add(Node::App(result, a))?;
                s = reader.link(s, "tail")?;
            }
            _ => return Err(Error::InvalidGraph("invalid stack")),
        }
    }
    OpenTerm::new(reader.nodes, result, reader.free_names, limits)
}
