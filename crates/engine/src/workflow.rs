//! Opt-in, whole-carrier workflow contracts. This is a sufficient structural
//! recogniser, not a classifier for arbitrary self-modifying programs.
//! Native admission is unchanged. The optional scheduler serves persistent task
//! identities, not replaceable rule revisions. See the public report's scope.
use crate::recording::{Recording, RecordingLimits};
use crate::{Execution, ExecutionLimits, OperationalBinding};
use graph_core::{
    AtomKey, Binding, Diagnostic, ErrorKind, Graph, Kind, LimitKind, Result, Role, Sort,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Child {
    pub stage: String,
    pub payload: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub children: Vec<Child>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub from: String,
    pub old: String,
    pub to: String,
    /// None admits continuing revisions; Some requires a consumed loose permit.
    pub permit: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema: String,
    /// Decimal u64 strings avoid JSON-number precision loss.
    pub weights: BTreeMap<String, String>,
    pub terminal: BTreeSet<String>,
    pub routes: Vec<Route>,
    pub updates: Vec<Update>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Record {
    pub ticket: String,
    pub payload_identity: String,
    pub payload: String,
    pub identity_link: String,
    pub stage: String,
    #[serde(skip)]
    key: Option<AtomKey>,
    #[serde(skip)]
    status: Option<AtomKey>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub verdict: &'static str,
    pub guarantee: &'static str,
    pub proof_scope: &'static str,
    pub records: Vec<Record>,
    pub unfinished: usize,
    pub work_potential: String,
    pub loose_permits: usize,
    pub all_orders_transaction_bound: Option<String>,
    pub observation: &'static str,
    pub disturbances: &'static str,
    pub agreement: &'static str,
}
#[derive(Clone)]
struct Processor {
    head: AtomKey,
    item: AtomKey,
    input: AtomKey,
    route: Route,
}
struct Checked {
    report: Report,
    processors: Vec<Processor>,
    meta: BTreeSet<AtomKey>,
}
fn unsupported(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(ErrorKind::MalformedInput, message)
}
fn charge(work: &mut usize, n: usize) -> Result<()> {
    *work = work.checked_sub(n).ok_or_else(|| {
        Diagnostic::exhausted(LimitKind::SearchWork, "workflow recognition work exhausted")
    })?;
    Ok(())
}
fn link(role: Role, label: Option<&str>) -> Kind {
    Kind::Record {
        role,
        label: label.map(str::to_owned),
    }
}
#[derive(Default)]
struct Template(BTreeMap<String, (Kind, Vec<String>)>);
impl Template {
    fn put(&mut self, name: &str, kind: Kind, fields: &[&str]) {
        self.0.insert(
            name.into(),
            (kind, fields.iter().map(|s| (*s).into()).collect()),
        );
    }
    fn member(&mut self, side: Role, name: &str) {
        self.put(
            &format!("{side:?}_{name}"),
            Kind::record(side),
            &["h", name],
        );
    }
    fn base(meta: bool) -> Self {
        let mut a = Self::default();
        a.put("h", Kind::RuleHead, &[]);
        a.put("z", Kind::Nat, &[]);
        if meta {
            a.put("one", Kind::Nat, &["z"]);
        }
        a.put(
            "rank",
            Kind::record(Role::Rank),
            &["h", if meta { "one" } else { "z" }],
        );
        a.put(
            "mode",
            Kind::record(if meta {
                Role::ModeMeta
            } else {
                Role::ModeObject
            }),
            &["h"],
        );
        a.put("revision", Kind::record(Role::Revision), &["h"]);
        a.put("active", Kind::record(Role::Active), &["h"]);
        a
    }
    fn route(r: &Route) -> Self {
        let mut a = Self::base(false);
        a.put("item", Kind::Constructor("Ticket".into()), &[]);
        a.put(
            "input",
            link(Role::ObjectLink, Some(&r.from)),
            &["item", "item"],
        );
        a.put(
            "output",
            link(Role::ObjectLink, Some(&r.to)),
            &["item", "item"],
        );
        for n in ["item", "input"] {
            a.member(Role::Lhs, n);
        }
        for n in ["item", "output"] {
            a.member(Role::Rhs, n);
        }
        for (i, c) in r.children.iter().enumerate() {
            let t = format!("child{i}");
            let p = format!("payload{i}");
            let id = format!("identity{i}");
            let s = format!("status{i}");
            a.put(&t, Kind::Constructor("Ticket".into()), &[]);
            a.put(&p, Kind::Constructor(c.payload.clone()), &[]);
            a.put(&id, link(Role::ObjectLink, Some("id")), &[&t, &p]);
            a.put(&s, link(Role::ObjectLink, Some(&c.stage)), &[&t, &t]);
            for n in [&t, &p, &id, &s] {
                a.member(Role::Rhs, n);
            }
        }
        a
    }
    fn update(u: &Update) -> Self {
        let mut a = Self::base(true);
        a.put("target", Kind::variable(Sort::RuleHead, Binding::Free), &[]);
        a.put("targetMode", Kind::record(Role::ModeObject), &["target"]);
        a.put("oldRev", Kind::record(Role::Revision), &["target"]);
        a.put("newRev", Kind::record(Role::Revision), &["target"]);
        a.put("item", Kind::Constructor("Ticket".into()), &[]);
        a.put(
            "input",
            link(Role::ObjectLink, Some(&u.from)),
            &["item", "item"],
        );
        a.put("inputMember", Kind::record(Role::Lhs), &["target", "input"]);
        a.put(
            "oldOutput",
            link(Role::ObjectLink, Some(&u.old)),
            &["item", "item"],
        );
        a.put(
            "oldMember",
            Kind::record(Role::Rhs),
            &["target", "oldOutput"],
        );
        a.put(
            "newOutput",
            link(Role::ObjectLink, Some(&u.to)),
            &["item", "item"],
        );
        a.put(
            "newMember",
            Kind::record(Role::Rhs),
            &["target", "newOutput"],
        );
        for n in [
            "target",
            "targetMode",
            "oldRev",
            "item",
            "input",
            "inputMember",
            "oldOutput",
            "oldMember",
        ] {
            a.member(Role::Lhs, n);
        }
        if let Some(p) = &u.permit {
            a.put("permit", Kind::Constructor(p.clone()), &[]);
            a.member(Role::Lhs, "permit");
        }
        for n in [
            "target",
            "targetMode",
            "newRev",
            "item",
            "input",
            "inputMember",
            "newOutput",
            "newMember",
        ] {
            a.member(Role::Rhs, n);
        }
        a
    }
    fn instantiate(&self, graph: &mut Graph) -> Result<BTreeMap<String, AtomKey>> {
        fn put(
            name: &str,
            t: &Template,
            graph: &mut Graph,
            names: &mut BTreeMap<String, AtomKey>,
        ) -> Result<AtomKey> {
            if let Some(key) = names.get(name) {
                return Ok(*key);
            }
            let (kind, fields) = &t.0[name];
            let fields = fields
                .iter()
                .map(|f| put(f, t, graph, names))
                .collect::<Result<Vec<_>>>()?;
            let key = graph.insert(kind.clone(), &fields)?;
            names.insert(name.into(), key);
            Ok(key)
        }
        let mut names = BTreeMap::new();
        for n in self.0.keys() {
            put(n, self, graph, &mut names)?;
        }
        Ok(names)
    }
    fn matches(
        &self,
        graph: &Graph,
        component: &BTreeSet<AtomKey>,
        work: &mut usize,
    ) -> Result<Option<BTreeMap<String, AtomKey>>> {
        if self.0.len() != component.len() {
            return Ok(None);
        }
        let mut candidates = BTreeMap::new();
        for (n, (kind, fields)) in &self.0 {
            let mut found = Vec::new();
            for key in component {
                charge(work, 1)?;
                let atom = graph.atom(*key)?;
                if atom.kind == *kind && atom.fields.len() == fields.len() {
                    found.push(*key);
                }
            }
            if found.is_empty() {
                return Ok(None);
            }
            candidates.insert(n.clone(), found);
        }
        let mut names: Vec<_> = self.0.keys().cloned().collect();
        names.sort_by_key(|n| candidates[n].len());
        fn search(
            t: &Template,
            graph: &Graph,
            names: &[String],
            choices: &BTreeMap<String, Vec<AtomKey>>,
            map: &mut BTreeMap<String, AtomKey>,
            work: &mut usize,
        ) -> Result<bool> {
            charge(work, 1)?;
            let Some(n) = names.first() else {
                return Ok(true);
            };
            for key in &choices[n] {
                charge(work, map.len() + 1)?;
                if map.values().any(|x| x == key) {
                    continue;
                }
                map.insert(n.clone(), *key);
                let valid = map.iter().all(|(n, k)| {
                    t.0[n].1.iter().enumerate().all(|(i, f)| {
                        map.get(f)
                            .is_none_or(|v| graph.get(*k).is_some_and(|a| a.fields[i] == *v))
                    })
                });
                if valid && search(t, graph, &names[1..], choices, map, work)? {
                    return Ok(true);
                }
                map.remove(n);
            }
            Ok(false)
        }
        let mut map = BTreeMap::new();
        Ok(search(self, graph, &names, &candidates, &mut map, work)?.then_some(map))
    }
}

impl Contract {
    fn validate(&self) -> Result<BTreeMap<String, u64>> {
        if self.schema != "workflow-stability-v1"
            || self.weights.is_empty()
            || self.weights.len() > 64
            || self.routes.len() > 128
            || self.updates.len() > 64
            || self.terminal.is_empty()
        {
            return Err(unsupported("unsupported workflow contract schema or size"));
        }
        let mut w = BTreeMap::new();
        for (s, n) in &self.weights {
            if s.is_empty()
                || s == "id"
                || s.len() > 64
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                || n.is_empty()
                || (n.len() > 1 && n.starts_with('0'))
                || !n.bytes().all(|c| c.is_ascii_digit())
            {
                return Err(unsupported("invalid status or decimal weight"));
            }
            let weight = n
                .parse::<u64>()
                .map_err(|_| unsupported("weight exceeds u64"))?;
            if (weight == 0) != self.terminal.contains(s) {
                return Err(unsupported("terminal anchor differs from zero weights"));
            }
            w.insert(s.clone(), weight);
        }
        if !self.terminal.iter().all(|s| w.contains_key(s)) {
            return Err(unsupported("unknown terminal"));
        }
        for r in &self.routes {
            if r.children.len() > 8 || !w.contains_key(&r.from) || !w.contains_key(&r.to) {
                return Err(unsupported("unknown route status or excessive fanout"));
            }
            let mut post = u128::from(w[&r.to]);
            for c in &r.children {
                if !c.payload.starts_with("id:")
                    || c.payload.len() > 256
                    || !w.contains_key(&c.stage)
                {
                    return Err(unsupported("invalid child descriptor"));
                }
                post += u128::from(w[&c.stage]);
            }
            if u128::from(w[&r.from]) <= post {
                return Err(unsupported("route fails aggregate work descent"));
            }
        }
        // A supervisor can match any processor with its input/output guard.
        // Require closure under that replacement, INCLUDING every child list.
        for u in &self.updates {
            if !w.contains_key(&u.from)
                || !w.contains_key(&u.old)
                || !w.contains_key(&u.to)
                || u.permit
                    .as_ref()
                    .is_some_and(|p| !p.starts_with("Permit:") || p.len() > 256)
            {
                return Err(unsupported("invalid revision descriptor"));
            }
            for r in self
                .routes
                .iter()
                .filter(|r| r.from == u.from && r.to == u.old)
            {
                let mut next = r.clone();
                next.to.clone_from(&u.to);
                if !self.routes.contains(&next) {
                    return Err(unsupported(
                        "allowed route family is not closed under a permitted revision",
                    ));
                }
            }
        }
        Ok(w)
    }
}

fn inspect(graph: &Graph, contract: &Contract, limits: ExecutionLimits) -> Result<Checked> {
    let weights = contract.validate()?;
    let configuration = graph_core::validate_with_limits(graph, &limits.graph)?;
    let mut work = limits.search.max_work;
    charge(&mut work, graph.len() + graph.field_count())?;
    let mut adjacency: BTreeMap<_, Vec<_>> = graph.iter().map(|(k, _)| (k, Vec::new())).collect();
    for (k, a) in graph.iter() {
        for f in &a.fields {
            adjacency.get_mut(&k).expect("key").push(*f);
            adjacency.get_mut(f).expect("validated field").push(k);
        }
    }
    let mut seen = BTreeSet::new();
    let mut processors = Vec::new();
    let mut meta = BTreeSet::new();
    let mut records = Vec::new();
    let mut permits = 0;
    let mut persistent = false;
    let routes: Vec<_> = contract.routes.iter().map(Template::route).collect();
    let updates: Vec<_> = contract.updates.iter().map(Template::update).collect();
    for key in adjacency.keys() {
        if !seen.insert(*key) {
            continue;
        }
        let mut component = BTreeSet::new();
        let mut todo = vec![*key];
        while let Some(k) = todo.pop() {
            component.insert(k);
            for n in &adjacency[&k] {
                if seen.insert(*n) {
                    todo.push(*n);
                }
            }
        }
        let heads: Vec<_> = component
            .iter()
            .filter(|k| graph.get(**k).is_some_and(|a| a.kind == Kind::RuleHead))
            .copied()
            .collect();
        if !heads.is_empty() {
            if heads.len() != 1 || !configuration.rules.contains_key(&heads[0]) {
                return Err(unsupported(
                    "shared or non-live rule components unsupported",
                ));
            }
            let head = heads[0];
            let rule = &configuration.rules[&head];
            let mut accepted = false;
            if rule.meta {
                for (u, template) in contract.updates.iter().zip(&updates) {
                    if template.matches(graph, &component, &mut work)?.is_some() {
                        accepted = true;
                        meta.insert(head);
                        persistent |= u.permit.is_none();
                        break;
                    }
                }
            } else {
                for (r, template) in contract.routes.iter().zip(&routes) {
                    if let Some(map) = template.matches(graph, &component, &mut work)? {
                        accepted = true;
                        processors.push(Processor {
                            head,
                            item: map["item"],
                            input: map["input"],
                            route: r.clone(),
                        });
                        break;
                    }
                }
            }
            if !accepted {
                return Err(unsupported(format!(
                    "rule {} does not match the complete permitted profile",
                    head.number()
                )));
            }
        } else if component.len() == 1
            && graph.get(*key).is_some_and(
                |a| matches!(&a.kind, Kind::Constructor(p) if p.starts_with("Permit:")),
            )
        {
            permits += 1;
        } else {
            if component.len() != 4 {
                return Err(unsupported(
                    "record component must contain exactly ticket, payload, identity and status",
                ));
            }
            let ticket = component
                .iter()
                .find(|k| {
                    graph
                        .get(**k)
                        .is_some_and(|a| a.kind == Kind::Constructor("Ticket".into()))
                })
                .copied()
                .ok_or_else(|| unsupported("missing ticket"))?;
            let payload = component
                .iter()
                .find(|k| {
                    graph.get(**k).is_some_and(
                        |a| matches!(&a.kind, Kind::Constructor(p) if p.starts_with("id:")),
                    )
                })
                .copied()
                .ok_or_else(|| unsupported("missing id payload"))?;
            let id = component
                .iter()
                .find(|k| {
                    graph.get(**k).is_some_and(|a| {
                        a.kind == link(Role::ObjectLink, Some("id"))
                            && a.fields == [ticket, payload]
                    })
                })
                .copied()
                .ok_or_else(|| unsupported("missing immutable identity link"))?;
            let status = component.iter().find(|k| graph.get(**k).is_some_and(|a| matches!(&a.kind, Kind::Record {role: Role::ObjectLink, label: Some(s)} if weights.contains_key(s)) && a.fields == [ticket, ticket])).copied().ok_or_else(|| unsupported("missing unique status"))?;
            if BTreeSet::from([ticket, payload, id, status]).len() != 4 {
                return Err(unsupported("record alias"));
            }
            let Kind::Constructor(value) = &graph.atom(payload)?.kind else {
                unreachable!()
            };
            let Kind::Record {
                label: Some(stage), ..
            } = &graph.atom(status)?.kind
            else {
                unreachable!()
            };
            records.push(Record {
                ticket: ticket.number().to_string(),
                payload_identity: payload.number().to_string(),
                payload: value.clone(),
                identity_link: id.number().to_string(),
                stage: stage.clone(),
                key: Some(ticket),
                status: Some(status),
            });
        }
    }
    for (s, w) in &weights {
        if *w > 0 && !processors.iter().any(|p| &p.route.from == s) {
            return Err(unsupported(format!(
                "no active processor covers nonterminal status {s}"
            )));
        }
    }
    let potential: u128 = records.iter().map(|r| u128::from(weights[&r.stage])).sum();
    let unfinished = records.iter().filter(|r| weights[&r.stage] > 0).count();
    Ok(Checked { report: Report { schema: "workflow-stability-report-v1", verdict: "certified", guarantee: if persistent { "completion-under-persistent-work-scheduler" } else { "completion-under-all-maximal-orders" }, proof_scope: "manual-carrier-refinement; mechanised-abstract-ranking; native-engine-not-formally-verified", records, unfinished, work_potential: potential.to_string(), loose_permits: permits,
        all_orders_transaction_bound: (!persistent).then(|| (potential + permits as u128).to_string()), observation: "existing-ticket-identities-and-payloads-preserved; completed-statuses-stable; branching-may-create-new-tasks", disturbances: "only-explicit-status-reset-with-frame-check-and-new-history", agreement: "not-certified: terminal-label-or-branch-choice-may-differ-across-schedules" }, processors, meta })
}

pub fn check(graph: &Graph, contract: &Contract, limits: ExecutionLimits) -> Result<Report> {
    Ok(inspect(graph, contract, limits)?.report)
}

/// A closed, checked execution with a FIFO of persistent unfinished identities.
/// No mutable graph or contract is exposed. Budgets/interruptions are not completion.
pub struct Session {
    execution: Execution,
    recording: Recording,
    contract: Contract,
    checked: Checked,
    limits: ExecutionLimits,
    queue: VecDeque<AtomKey>,
    burst: usize,
    credit: usize,
}
impl Session {
    pub fn new(
        graph: Graph,
        contract: Contract,
        limits: ExecutionLimits,
        recording: RecordingLimits,
        meta_burst: usize,
    ) -> Result<Self> {
        if meta_burst > 32 {
            return Err(unsupported("meta_burst must be at most 32"));
        }
        let checked = inspect(&graph, &contract, limits)?;
        let queue = checked
            .report
            .records
            .iter()
            .filter(|r| !contract.terminal.contains(&r.stage))
            .map(|r| r.key.expect("decoded key"))
            .collect();
        let execution = Execution::new(graph, limits)?;
        let recording = Recording::new(&execution, recording)?;
        Ok(Self {
            execution,
            recording,
            contract,
            checked,
            limits,
            queue,
            burst: meta_burst,
            credit: meta_burst,
        })
    }
    pub fn graph(&self) -> &Graph {
        self.execution.graph()
    }
    pub fn report(&self) -> &Report {
        &self.checked.report
    }
    pub fn recording(&self) -> &Recording {
        &self.recording
    }
    pub fn completion_bound(&self) -> String {
        let work = self
            .checked
            .report
            .work_potential
            .parse::<u128>()
            .expect("computed potential");
        (work * (self.burst as u128 + 1)).to_string()
    }
    pub fn step(&mut self) -> Result<Option<crate::Event>> {
        let mut selected = None;
        if (self.queue.is_empty() || self.credit > 0) && !self.checked.meta.is_empty() {
            let steps = self.execution.enabled()?.into_steps();
            selected = steps
                .iter()
                .filter(|p| self.checked.meta.contains(&p.binding().source()))
                .min_by_key(|p| crate::scheduling::candidate_key(p))
                .map(|p| p.binding().clone());
        }
        let administrative = selected.is_some();
        let mut next_queue = self.queue.clone();
        if selected.is_none() {
            if let Some(ticket) = next_queue.pop_front() {
                let r = self
                    .checked
                    .report
                    .records
                    .iter()
                    .find(|r| r.key == Some(ticket))
                    .ok_or_else(|| unsupported("lost persistent obligation"))?;
                let p = self
                    .checked
                    .processors
                    .iter()
                    .find(|p| p.route.from == r.stage)
                    .ok_or_else(|| unsupported("unfinished work has no processor"))?;
                selected = Some(OperationalBinding::new(
                    p.head,
                    BTreeMap::from([(p.item, ticket), (p.input, r.status.expect("status"))]),
                ));
            } else {
                return Ok(None);
            }
        }
        let binding = selected.expect("selected binding");
        // Check the complete prospective carrier before publishing graph/history.
        let mut trial = Execution::new(self.graph().clone(), self.limits)?;
        let prepared = trial.prepare(binding.clone())?;
        trial.commit(prepared)?;
        let next = inspect(trial.graph(), &self.contract, self.limits)?;
        for old in &self.checked.report.records {
            let new = next
                .report
                .records
                .iter()
                .find(|r| r.ticket == old.ticket)
                .ok_or_else(|| unsupported("protected task disappeared"))?;
            if new.payload_identity != old.payload_identity
                || new.payload != old.payload
                || new.identity_link != old.identity_link
                || (self.contract.terminal.contains(&old.stage) && new.stage != old.stage)
            {
                return Err(unsupported("protected observation changed"));
            }
        }
        let before = self
            .checked
            .report
            .work_potential
            .parse::<u128>()
            .expect("potential");
        let after = next
            .report
            .work_potential
            .parse::<u128>()
            .expect("potential");
        if if administrative {
            before != after
        } else {
            before <= after
        } {
            return Err(unsupported("native transition violates work contract"));
        }
        next_queue.retain(|k| {
            next.report
                .records
                .iter()
                .any(|r| r.key == Some(*k) && !self.contract.terminal.contains(&r.stage))
        });
        for r in &next.report.records {
            if !self.contract.terminal.contains(&r.stage)
                && !next_queue.contains(&r.key.expect("key"))
            {
                next_queue.push_back(r.key.expect("key"));
            }
        }
        let prepared = self.execution.prepare(binding)?;
        let event = self.recording.commit(&mut self.execution, prepared)?;
        self.checked = next;
        self.queue = next_queue;
        self.credit = if administrative {
            self.credit.saturating_sub(1)
        } else {
            self.burst
        };
        Ok(Some(event))
    }
    /// An explicit external intervention starts a new history. It does not claim
    /// to repair malformed code, missing payloads or arbitrary corruption.
    pub fn reset_status(
        &self,
        ticket: &str,
        stage: &str,
        recording: RecordingLimits,
    ) -> Result<Self> {
        if !self.contract.weights.contains_key(stage) || self.contract.terminal.contains(stage) {
            return Err(unsupported("reset requires a known nonterminal status"));
        }
        let r = self
            .checked
            .report
            .records
            .iter()
            .find(|r| r.ticket == ticket)
            .ok_or_else(|| unsupported("unknown task identity"))?;
        let mut graph = self.graph().clone();
        graph.remove_set(&BTreeSet::from([r.status.expect("status")]))?;
        graph.insert(
            link(Role::ObjectLink, Some(stage)),
            &[r.key.expect("ticket"), r.key.expect("ticket")],
        )?;
        Self::new(
            graph,
            self.contract.clone(),
            self.limits,
            recording,
            self.burst,
        )
    }
}

/// Reproducible task-routing examples, not external document processing.
pub fn example(branching: bool, count: usize, persistent: bool) -> Result<(Graph, Contract)> {
    if count > 1000 {
        return Err(unsupported("example count exceeds 1000"));
    }
    let route = |from: &str, to: &str| Route {
        from: from.into(),
        to: to.into(),
        children: vec![],
    };
    let mut routes = vec![
        route("received", "review"),
        route("received", "archive"),
        route("review", "archive"),
    ];
    let mut weights = BTreeMap::from([
        ("received".into(), "2".into()),
        ("review".into(), "1".into()),
        ("archive".into(), "0".into()),
    ]);
    if branching {
        weights.insert("split".into(), "5".into());
        routes.push(Route {
            from: "split".into(),
            to: "archive".into(),
            children: vec![
                Child {
                    stage: "received".into(),
                    payload: "id:validate".into(),
                },
                Child {
                    stage: "received".into(),
                    payload: "id:index".into(),
                },
            ],
        });
    }
    let updates = vec![
        Update {
            from: "received".into(),
            old: "review".into(),
            to: "archive".into(),
            permit: (!persistent).then(|| "Permit:fast".into()),
        },
        Update {
            from: "received".into(),
            old: "archive".into(),
            to: "review".into(),
            permit: (!persistent).then(|| "Permit:careful".into()),
        },
    ];
    let contract = Contract {
        schema: "workflow-stability-v1".into(),
        weights,
        terminal: BTreeSet::from(["archive".into()]),
        routes,
        updates,
    };
    let mut graph = Graph::new();
    for r in &contract.routes {
        if !(r.from == "received" && r.to == "archive") {
            Template::route(r).instantiate(&mut graph)?;
        }
    }
    for u in &contract.updates {
        Template::update(u).instantiate(&mut graph)?;
        if let Some(p) = &u.permit {
            for _ in 0..2 {
                graph.insert(Kind::Constructor(p.clone()), &[])?;
            }
        }
    }
    for i in 0..count {
        let t = graph.insert(Kind::Constructor("Ticket".into()), &[])?;
        let p = graph.insert(Kind::Constructor(format!("id:request-{i}")), &[])?;
        graph.insert(link(Role::ObjectLink, Some("id")), &[t, p])?;
        graph.insert(
            link(
                Role::ObjectLink,
                Some(if branching { "split" } else { "received" }),
            ),
            &[t, t],
        )?;
    }
    check(&graph, &contract, ExecutionLimits::default())?;
    Ok((graph, contract))
}
