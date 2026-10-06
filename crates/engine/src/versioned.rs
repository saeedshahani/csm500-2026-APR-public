//! A finite resident interpreter with immutable captured implementation versions.
//!
//! Construction authors ordinary rules; publication and evaluation use normal
//! admission. Readback reports the installed description, not a cached proposal.
//! The supported implementations both return the argument identity. Their guest
//! costs are one and two evaluation rewrites, not host time or a general tariff.
use graph_core::{AtomKey, Diagnostic, Graph, Kind, Role, Sort};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const REGISTRY: &str = "versioned.Registry";
const VERSION: &str = "versioned.Version";
const RETURN: &str = "versioned.Return";
const DELAY: &str = "versioned.DelayReturn";
const PUBLISH: &str = "versioned.Publish";
const SPAWN: &str = "versioned.Spawn";
const CALL: &str = "versioned.Call";
const PENDING: &str = "versioned.Pending";
const DONE: &str = "versioned.Done";

/// The closed implementation language of this application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Implementation {
    Return,
    DelayReturn,
}
impl Implementation {
    /// Number of guest evaluation rewrites after capture, excluding publication.
    pub fn cost(self) -> u8 {
        match self {
            Self::Return => 1,
            Self::DelayReturn => 2,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Return => RETURN,
            Self::DelayReturn => DELAY,
        }
    }
}

/// One old call and one spawn request share an inert payload. Publication may
/// race with spawning; a spawned call captures the active version at that event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub previous: Implementation,
    pub replacement: Implementation,
    pub budget: u8,
    pub payload: String,
}

#[derive(Debug)]
pub enum Error {
    Input(String),
    Native(Diagnostic),
}
impl From<Diagnostic> for Error {
    fn from(value: Diagnostic) -> Self {
        Self::Native(value)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(s) => f.write_str(s),
            Self::Native(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
fn invalid(s: &str) -> Error {
    Error::Input(s.into())
}
fn ctor(g: &mut Graph, label: &str) -> Result<AtomKey, Error> {
    Ok(g.insert(Kind::Constructor(label.into()), &[])?)
}
fn link(g: &mut Graph, label: &str, a: AtomKey, b: AtomKey) -> Result<AtomKey, Error> {
    Ok(g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(label.into()),
        },
        &[a, b],
    )?)
}

#[derive(Clone, Copy)]
enum Side {
    Both,
    Left,
    Right,
}
struct Pattern<'a> {
    graph: &'a mut Graph,
    head: AtomKey,
    left: Vec<AtomKey>,
    right: Vec<AtomKey>,
}
impl<'a> Pattern<'a> {
    fn new(graph: &'a mut Graph) -> Result<Self, Error> {
        let head = graph.insert(Kind::RuleHead, &[])?;
        let zero = graph.insert(Kind::Nat, &[])?;
        graph.insert(Kind::record(Role::Rank), &[head, zero])?;
        for role in [Role::Revision, Role::ModeObject, Role::Active] {
            graph.insert(Kind::record(role), &[head])?;
        }
        Ok(Self {
            graph,
            head,
            left: vec![],
            right: vec![],
        })
    }
    fn add(&mut self, kind: Kind, fields: &[AtomKey], side: Side) -> Result<AtomKey, Error> {
        let id = self.graph.insert(kind, fields)?;
        if matches!(side, Side::Both | Side::Left) {
            self.left.push(id);
        }
        if matches!(side, Side::Both | Side::Right) {
            self.right.push(id);
        }
        Ok(id)
    }
    fn atom(&mut self, label: &str, side: Side) -> Result<AtomKey, Error> {
        self.add(Kind::Constructor(label.into()), &[], side)
    }
    fn variable(&mut self) -> Result<AtomKey, Error> {
        self.add(
            Kind::variable(Sort::Constructor, graph_core::Binding::Free),
            &[],
            Side::Both,
        )
    }
    fn edge(&mut self, label: &str, a: AtomKey, b: AtomKey, side: Side) -> Result<(), Error> {
        self.add(
            Kind::Record {
                role: Role::ObjectLink,
                label: Some(label.into()),
            },
            &[a, b],
            side,
        )?;
        Ok(())
    }
    fn finish(self) -> Result<(), Error> {
        for (role, members) in [(Role::Lhs, self.left), (Role::Rhs, self.right)] {
            for item in members {
                self.graph.insert(Kind::record(role), &[self.head, item])?;
            }
        }
        Ok(())
    }
}

fn publication(g: &mut Graph, body: Implementation, budget: u8) -> Result<(), Error> {
    let mut p = Pattern::new(g)?;
    let r = p.atom(REGISTRY, Side::Both)?;
    let old = p.atom(VERSION, Side::Both)?;
    let new = p.atom(VERSION, Side::Both)?;
    let description = p.atom(body.label(), Side::Both)?;
    let bound = p.atom(&format!("versioned.Budget:{budget}"), Side::Both)?;
    p.edge("budget", r, bound, Side::Both)?;
    p.edge("body", new, description, Side::Both)?;
    p.edge("active", r, old, Side::Left)?;
    p.edge("active", r, new, Side::Right)?;
    let ticket = p.atom(PUBLISH, Side::Left)?;
    p.edge("registry", ticket, r, Side::Left)?;
    p.edge("from", ticket, old, Side::Left)?;
    p.edge("to", ticket, new, Side::Left)?;
    p.finish()
}
fn spawn(g: &mut Graph) -> Result<(), Error> {
    let mut p = Pattern::new(g)?;
    let r = p.atom(REGISTRY, Side::Both)?;
    let v = p.atom(VERSION, Side::Both)?;
    let x = p.variable()?;
    p.edge("active", r, v, Side::Both)?;
    let ticket = p.atom(SPAWN, Side::Left)?;
    p.edge("registry", ticket, r, Side::Left)?;
    p.edge("arg", ticket, x, Side::Left)?;
    let call = p.atom(CALL, Side::Right)?;
    p.edge("version", call, v, Side::Right)?;
    p.edge("arg", call, x, Side::Right)?;
    p.finish()
}
fn evaluation(
    g: &mut Graph,
    from: &str,
    to: &str,
    implementation: Option<Implementation>,
) -> Result<(), Error> {
    let mut p = Pattern::new(g)?;
    let v = p.atom(VERSION, Side::Both)?;
    let x = p.variable()?;
    if let Some(body) = implementation {
        let d = p.atom(body.label(), Side::Both)?;
        p.edge("body", v, d, Side::Both)?;
    }
    let before = p.atom(from, Side::Left)?;
    p.edge("version", before, v, Side::Left)?;
    p.edge("arg", before, x, Side::Left)?;
    let after = p.atom(to, Side::Right)?;
    p.edge("version", after, v, Side::Right)?;
    p.edge("arg", after, x, Side::Right)?;
    p.finish()
}

/// Construct the closed profile. A replacement exceeding the budget remains a
/// pending proposal with no enabled publication. No proposal is installed here.
pub fn compile(request: &Request) -> Result<Graph, Error> {
    if !(1..=2).contains(&request.budget)
        || request.payload.is_empty()
        || request.payload.len() > 256
    {
        return Err(invalid(
            "budget must be 1 or 2; payload must contain 1..256 UTF-8 bytes",
        ));
    }
    let mut g = Graph::new();
    publication(&mut g, Implementation::Return, 1)?;
    publication(&mut g, Implementation::Return, 2)?;
    publication(&mut g, Implementation::DelayReturn, 2)?;
    spawn(&mut g)?;
    evaluation(&mut g, CALL, DONE, Some(Implementation::Return))?;
    evaluation(&mut g, CALL, PENDING, Some(Implementation::DelayReturn))?;
    evaluation(&mut g, PENDING, DONE, None)?;
    let r = ctor(&mut g, REGISTRY)?;
    let old = ctor(&mut g, VERSION)?;
    let new = ctor(&mut g, VERSION)?;
    let a = ctor(&mut g, request.previous.label())?;
    let b = ctor(&mut g, request.replacement.label())?;
    link(&mut g, "body", old, a)?;
    link(&mut g, "body", new, b)?;
    link(&mut g, "active", r, old)?;
    let bound = ctor(&mut g, &format!("versioned.Budget:{}", request.budget))?;
    link(&mut g, "budget", r, bound)?;
    let publish = ctor(&mut g, PUBLISH)?;
    link(&mut g, "registry", publish, r)?;
    link(&mut g, "from", publish, old)?;
    link(&mut g, "to", publish, new)?;
    let x = ctor(&mut g, &format!("payload:{}", request.payload))?;
    let call = ctor(&mut g, CALL)?;
    link(&mut g, "version", call, old)?;
    link(&mut g, "arg", call, x)?;
    let spawn = ctor(&mut g, SPAWN)?;
    link(&mut g, "registry", spawn, r)?;
    link(&mut g, "arg", spawn, x)?;
    graph_core::validate(&g)?;
    inspect(&g)?;
    Ok(g)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Version {
    pub identity: String,
    pub implementation: Implementation,
    pub evaluation_steps: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Call {
    pub identity: String,
    pub status: String,
    pub version: Version,
    pub payload_identity: String,
    pub payload: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub schema: &'static str,
    pub active: Version,
    pub versions: Vec<Version>,
    pub budget: u8,
    pub publication_pending: bool,
    pub spawn_pending: bool,
    pub calls: Vec<Call>,
}

/// Decode operative version bodies, including the installed active body. This
/// is a bounded shape check/readback, not authentication or a proof that an
/// arbitrary edited graph contains only the compiler's transition rules.
pub fn inspect(g: &Graph) -> Result<View, Error> {
    let cfg = graph_core::validate(g)?;
    let quoted: BTreeSet<_> = cfg
        .rules
        .values()
        .flat_map(|r| r.representation.iter().copied())
        .collect();
    let visible = |label: &str| -> Vec<AtomKey> {
        g.iter()
            .filter_map(|(k, a)| {
                (!quoted.contains(&k) && a.kind == Kind::Constructor(label.into())).then_some(k)
            })
            .collect()
    };
    let edge = |owner: AtomKey, label: &str| -> Result<AtomKey, Error> {
        let mut found = g.iter().filter_map(|(k, a)| match &a.kind {
            Kind::Record {
                role: Role::ObjectLink,
                label: Some(s),
            } if !quoted.contains(&k) && s == label && a.fields[0] == owner => Some(a.fields[1]),
            _ => None,
        });
        let target = found
            .next()
            .ok_or_else(|| invalid("missing versioned field"))?;
        if found.next().is_some() || quoted.contains(&target) {
            return Err(invalid("ambiguous or quoted versioned field"));
        }
        Ok(target)
    };
    let registries = visible(REGISTRY);
    if registries.len() != 1 {
        return Err(invalid("expected one versioned registry"));
    }
    let r = registries[0];
    let versions = visible(VERSION);
    if versions.len() != 2 {
        return Err(invalid("expected two implementation versions"));
    }
    let decode = |v: AtomKey| -> Result<Version, Error> {
        if !versions.contains(&v) {
            return Err(invalid("unknown implementation version"));
        }
        let body = edge(v, "body")?;
        let implementation = match &g.atom(body)?.kind {
            Kind::Constructor(s) if s == RETURN => Implementation::Return,
            Kind::Constructor(s) if s == DELAY => Implementation::DelayReturn,
            _ => return Err(invalid("unsupported installed implementation")),
        };
        Ok(Version {
            identity: v.number().to_string(),
            implementation,
            evaluation_steps: implementation.cost(),
        })
    };
    let active = decode(edge(r, "active")?)?;
    let bound = edge(r, "budget")?;
    let budget = match &g.atom(bound)?.kind {
        Kind::Constructor(s) if s == "versioned.Budget:1" => 1,
        Kind::Constructor(s) if s == "versioned.Budget:2" => 2,
        _ => return Err(invalid("unsupported publication budget")),
    };
    let publication = visible(PUBLISH);
    let spawning = visible(SPAWN);
    if publication.len() > 1 || spawning.len() > 1 {
        return Err(invalid("duplicate versioned request"));
    }
    if let Some(t) = publication.first() {
        if edge(*t, "registry")? != r {
            return Err(invalid("foreign publication registry"));
        }
        decode(edge(*t, "from")?)?;
        decode(edge(*t, "to")?)?;
    }
    if let Some(t) = spawning.first() {
        if edge(*t, "registry")? != r {
            return Err(invalid("foreign spawn registry"));
        }
        edge(*t, "arg")?;
    }
    let mut calls = vec![];
    for (label, status) in [(CALL, "call"), (PENDING, "pending"), (DONE, "done")] {
        for id in visible(label) {
            let version = decode(edge(id, "version")?)?;
            if status == "pending" && version.implementation != Implementation::DelayReturn {
                return Err(invalid("pending call requires delay_return"));
            }
            let x = edge(id, "arg")?;
            let payload = match &g.atom(x)?.kind {
                Kind::Constructor(s) => s
                    .strip_prefix("payload:")
                    .ok_or_else(|| invalid("unsupported payload"))?
                    .to_owned(),
                _ => return Err(invalid("payload must be a constructor")),
            };
            calls.push(Call {
                identity: id.number().to_string(),
                status: status.into(),
                version,
                payload_identity: x.number().to_string(),
                payload,
            });
        }
    }
    calls.sort_by(|a, b| a.identity.cmp(&b.identity));
    Ok(View {
        schema: "versioned-execution-v1",
        active,
        versions: versions
            .iter()
            .copied()
            .map(decode)
            .collect::<Result<_, _>>()?,
        budget,
        publication_pending: !publication.is_empty(),
        spawn_pending: !spawning.is_empty(),
        calls,
    })
}
