//! Closed lambda terms, encoded as a graph-resident call-by-name environment machine.
//!
//! This adapter constructs syntax and rules; it never executes beta reduction.
//! The source arena uses zero-based de Bruijn indices. See `docs/lambda-encoding.md`
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
    if text.len() > limits.max_work || limits.max_depth > 128 {
        return Err(Error::ResourceLimit);
    }
    struct Reader<'a> {
        text: &'a [u8],
        at: usize,
        nodes: Vec<Node>,
        binders: Vec<String>,
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
        limits,
    };
    let root = reader.term(0)?;
    reader.space();
    if reader.at != text.len() {
        return Err(Error::InvalidSource("trailing input"));
    }
    Term::new(reader.nodes, root, limits)
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
        [root] => Ok(Some(readback(graph, *root, limits)?.display(limits)?)),
        _ => Err(Error::InvalidGraph("multiple lambda machines")),
    }
}

impl Term {
    pub fn new(nodes: Vec<Node>, root: usize, limits: Limits) -> Result<Self, Error> {
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
                Node::Var(n) if n >= binders => return Err(Error::InvalidSource("free variable")),
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
        fn go(
            t: &Term,
            id: usize,
            depth: usize,
            work: &mut usize,
            limits: Limits,
            out: &mut String,
        ) -> Result<(), Error> {
            spend(work)?;
            if depth > limits.max_depth || depth > 128 {
                return Err(Error::ResourceLimit);
            }
            match t.nodes[id] {
                Node::Var(n) => out.push_str(&n.to_string()),
                Node::Lam(b) => {
                    out.push_str("(λ.");
                    go(t, b, depth + 1, work, limits, out)?;
                    out.push(')');
                }
                Node::App(f, a) => {
                    out.push('(');
                    go(t, f, depth + 1, work, limits, out)?;
                    out.push(' ');
                    go(t, a, depth + 1, work, limits, out)?;
                    out.push(')');
                }
            }
            Ok(())
        }
        let mut out = String::new();
        let mut work = limits.max_work;
        go(self, self.root, 0, &mut work, limits, &mut out)?;
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
    // Recheck caller bounds even if the Term was validated with larger allowances.
    if term.nodes.len() > limits.max_nodes {
        return Err(Error::ResourceLimit);
    }
    Term::new(term.nodes.clone(), term.root, limits)?;
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
    let env = ctor(&mut graph, "NilEnv")?;
    let stack = ctor(&mut graph, "NilStack")?;
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
    ) -> Result<usize, Error> {
        if depth > self.limits.max_depth || depth > 128 {
            return Err(Error::ResourceLimit);
        }
        match self.label(t)? {
            "lc.Lam" => {
                let b = self.link(t, "body")?;
                let b = self.term(b, e, binders + 1, depth + 1)?;
                self.add(Node::Lam(b))
            }
            "lc.App" => {
                let f = self.link(t, "fun")?;
                let a = self.link(t, "arg")?;
                let f = self.term(f, e, binders, depth + 1)?;
                let a = self.term(a, e, binders, depth + 1)?;
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
                // Stored closures are closed after environment substitution, so
                // inserting their readback under binders needs no free-variable shift.
                self.term(a, f, 0, depth + 1)
            }
            _ => Err(Error::InvalidGraph("not lambda syntax")),
        }
    }
}

/// Reify the complete current weak-head expression, including pending arguments.
/// Not a beta evaluator, a normal-form test, or validation of the simulation invariant.
/// The correspondence applies to reachable encoded states; arbitrary graphs can fail.
pub fn readback(graph: &Graph, root: AtomKey, limits: Limits) -> Result<Term, Error> {
    if limits.max_depth > 128 {
        return Err(Error::ResourceLimit);
    }
    let mut reader = Reader {
        graph,
        links: BTreeMap::new(),
        nodes: Vec::new(),
        work: limits.max_work,
        limits,
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
    let t = reader.link(root, "term")?;
    let e = reader.link(root, "env")?;
    let mut s = reader.link(root, "stack")?;
    let mut result = reader.term(t, e, 0, 0)?;
    loop {
        match reader.label(s)? {
            "lc.NilStack" => break,
            "lc.Stack" => {
                let a = reader.link(s, "term")?;
                let f = reader.link(s, "env")?;
                let a = reader.term(a, f, 0, 0)?;
                result = reader.add(Node::App(result, a))?;
                s = reader.link(s, "tail")?;
            }
            _ => return Err(Error::InvalidGraph("invalid stack")),
        }
    }
    Term::new(reader.nodes, result, limits)
}
