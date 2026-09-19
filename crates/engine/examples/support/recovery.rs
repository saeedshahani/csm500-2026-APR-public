//! Constructed recovery family, not a new engine admission policy.
use graph_core::{Atom, AtomKey, Binding, Graph, Kind, Role, Sort};
use std::collections::{BTreeMap, BTreeSet};

fn atom(g: &mut Graph, label: &str) -> AtomKey {
    g.insert(Kind::Constructor(label.into()), &[]).unwrap()
}
fn var(g: &mut Graph, sort: Sort) -> AtomKey {
    g.insert(Kind::variable(sort, Binding::Free), &[]).unwrap()
}
fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> AtomKey {
    g.insert(Kind::record(role), fields).unwrap()
}
fn link(g: &mut Graph, label: &str, a: AtomKey, b: AtomKey) -> AtomKey {
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(label.into()),
        },
        &[a, b],
    )
    .unwrap()
}
fn rule(g: &mut Graph, meta: bool, active: bool) -> AtomKey {
    let h = g.insert(Kind::RuleHead, &[]).unwrap();
    let zero = g.insert(Kind::Nat, &[]).unwrap();
    let rank = if meta {
        g.insert(Kind::Nat, &[zero]).unwrap()
    } else {
        zero
    };
    record(g, Role::Rank, &[h, rank]);
    record(g, Role::Revision, &[h]);
    record(
        g,
        if meta {
            Role::ModeMeta
        } else {
            Role::ModeObject
        },
        &[h],
    );
    if active {
        record(g, Role::Active, &[h]);
    }
    h
}
fn sides(g: &mut Graph, head: AtomKey, lhs: &[AtomKey], rhs: &[AtomKey]) {
    for x in lhs {
        record(g, Role::Lhs, &[head, *x]);
    }
    for x in rhs {
        record(g, Role::Rhs, &[head, *x]);
    }
}

#[derive(Clone)]
pub struct Family {
    pub graph: Graph,
    pub processors: [AtomKey; 2],
    pub control: AtomKey,
    pub items: Vec<AtomKey>,
    pub switches: Vec<AtomKey>,
    pub idle: Vec<AtomKey>,
    // A trusted construction manifest, not a certificate supplied by a caller.
    fixed: BTreeMap<AtomKey, Atom>,
    end: AtomKey,
    templates: BTreeMap<AtomKey, Vec<AtomKey>>,
    markers: [AtomKey; 2],
    quoted_markers: Vec<AtomKey>,
}

/// Two actual policy rules alternate activation. All object-link endpoints
/// remain Constructors; metadata is accessed through properly typed memberships.
pub fn build(n: usize, modulating: bool) -> Family {
    assert!(n <= 1000, "research generator is bounded to 1000 items");
    let mut g = Graph::new();
    let mut processors = Vec::new();
    let mut templates = BTreeMap::new();
    let mut quoted_markers = Vec::new();
    for (policy, route, active) in [("PolicyA", "route-a", true), ("PolicyB", "route-b", false)] {
        let h = rule(&mut g, false, active);
        let marker = atom(&mut g, policy);
        quoted_markers.push(marker);
        let c = atom(&mut g, "RecoveryControl");
        let work = atom(&mut g, "Work");
        let phase = link(&mut g, "phase", c, work);
        let top = atom(&mut g, "Count");
        let tail = var(&mut g, Sort::Constructor);
        let next = link(&mut g, "next", top, tail);
        let remaining = link(&mut g, "remaining", c, top);
        let item = var(&mut g, Sort::Constructor);
        let pending = link(&mut g, "pending", item, item);
        let after = link(&mut g, "remaining", c, tail);
        let switch = atom(&mut g, "Switch");
        let next_phase = link(&mut g, "phase", c, switch);
        let done = link(&mut g, route, item, item);
        let lhs = vec![
            marker, c, work, phase, top, tail, next, remaining, item, pending,
        ];
        sides(
            &mut g,
            h,
            &lhs,
            &[marker, c, tail, item, after, switch, next_phase, done],
        );
        templates.insert(h, lhs);
        processors.push(h);
    }
    let mut switches = Vec::new();
    let mut idle = Vec::new();
    for is_idle in [false, true] {
        if is_idle && !modulating {
            continue;
        }
        for (from, to) in [("PolicyA", "PolicyB"), ("PolicyB", "PolicyA")] {
            let source = rule(&mut g, true, true);
            let a = var(&mut g, Sort::RuleHead);
            let b = var(&mut g, Sort::RuleHead);
            let ma = atom(&mut g, from);
            let mb = atom(&mut g, to);
            let la = record(&mut g, Role::Lhs, &[a, ma]);
            let lb = record(&mut g, Role::Lhs, &[b, mb]);
            let oa = record(&mut g, Role::ModeObject, &[a]);
            let ob = record(&mut g, Role::ModeObject, &[b]);
            let ra = record(&mut g, Role::Revision, &[a]);
            let rb = record(&mut g, Role::Revision, &[b]);
            let na = record(&mut g, Role::Revision, &[a]);
            let nb = record(&mut g, Role::Revision, &[b]);
            let active = record(&mut g, Role::Active, &[a]);
            let activated = record(&mut g, Role::Active, &[b]);
            let c = atom(&mut g, "RecoveryControl");
            let phase_value = atom(&mut g, if is_idle { "Work" } else { "Switch" });
            let phase = link(&mut g, "phase", c, phase_value);
            let work = atom(&mut g, "Work");
            let new_phase = link(&mut g, "phase", c, work);
            let mut lhs = vec![
                a,
                b,
                ma,
                mb,
                la,
                lb,
                oa,
                ob,
                ra,
                rb,
                active,
                c,
                phase_value,
                phase,
            ];
            let mut rhs = vec![
                a, b, ma, mb, la, lb, oa, ob, na, nb, activated, c, work, new_phase,
            ];
            if is_idle {
                let end = atom(&mut g, "End");
                let remaining = link(&mut g, "remaining", c, end);
                lhs.extend([end, remaining]);
                rhs.extend([end, remaining]);
                idle.push(source);
            } else {
                switches.push(source);
            }
            sides(&mut g, source, &lhs, &rhs);
            templates.insert(source, lhs);
        }
    }
    // Capture immutable syntax except the two processors' activity/revisions.
    let mut fixed: BTreeMap<_, _> = g
        .iter()
        .filter(|(_, a)| {
            !matches!(a.kind.role(), Some(Role::Active | Role::Revision))
                || !processors.contains(&a.fields[0])
        })
        .map(|(id, a)| (id, a.clone()))
        .collect();
    let control = atom(&mut g, "RecoveryControl");
    let work = atom(&mut g, "Work");
    link(&mut g, "phase", control, work);
    let end = atom(&mut g, "End");
    let mut counters = vec![end];
    for _ in 0..n {
        let top = atom(&mut g, "Count");
        link(&mut g, "next", top, *counters.last().unwrap());
        counters.push(top);
    }
    link(&mut g, "remaining", control, *counters.last().unwrap());
    let mut items = Vec::new();
    for i in 0..n {
        let item = atom(&mut g, &format!("item:{i}"));
        link(&mut g, "pending", item, item);
        items.push(item);
    }
    let markers = [atom(&mut g, "PolicyA"), atom(&mut g, "PolicyB")];
    for id in [control, end]
        .into_iter()
        .chain(items.iter().copied())
        .chain(markers)
    {
        fixed.insert(id, g.get(id).unwrap().clone());
    }
    graph_core::validate(&g).unwrap();
    Family {
        graph: g,
        processors: processors.try_into().unwrap(),
        control,
        items,
        switches,
        idle,
        fixed,
        end,
        templates,
        markers,
        quoted_markers,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub statuses: BTreeMap<AtomKey, String>,
    pub pending: usize,
    pub switch: bool,
    pub policy: usize,
}
impl State {
    pub fn potential(&self) -> usize {
        2 * self.pending + usize::from(self.switch)
    }
}

/// Full membership check for the exact generated family, not arbitrary programs.
/// Existing core validation remains mandatory. Unknown atoms/program edits,
/// altered counters, lost items and duplicate statuses are rejected.
impl Family {
    pub fn check(&self, g: &Graph) -> Result<State, String> {
        let config = graph_core::validate(g).map_err(|e| e.to_string())?;
        let mut accounted = BTreeSet::new();
        for (id, a) in &self.fixed {
            if g.get(*id) != Some(a) {
                return Err("fixed payload/program changed".into());
            }
            accounted.insert(*id);
        }
        let mut active = Vec::new();
        for (i, h) in self.processors.iter().enumerate() {
            let rule = config.rules.get(h).ok_or("processor missing")?;
            if rule.active {
                active.push(i);
            }
            for (id, a) in g.iter().filter(|(_, a)| {
                matches!(a.kind.role(), Some(Role::Revision | Role::Active)) && a.fields[0] == *h
            }) {
                accounted.insert(id);
                if a.kind.role() == Some(Role::Revision) && a.fields != [*h] {
                    return Err("revision shape".into());
                }
            }
        }
        if active.len() != 1 {
            return Err("exactly one processing policy required".into());
        }
        let statuses = statuses(g, &self.items)?;
        let pending = statuses
            .values()
            .filter(|s| s.as_str() == "pending")
            .count();
        for (id, _) in g.iter().filter(|(_, a)| {
            a.kind.role() == Some(Role::ObjectLink) && self.items.contains(&a.fields[0])
        }) {
            accounted.insert(id);
        }
        let one_link = |label: &str| -> Result<(AtomKey, AtomKey), String> {
            let found: Vec<_> = g
                .iter()
                .filter(|(_, a)| {
                    matches!(&a.kind,Kind::Record {role:Role::ObjectLink,label:Some(s)} if s==label)
                        && a.fields[0] == self.control
                })
                .collect();
            if found.len() != 1 {
                return Err(format!("expected one {label}"));
            }
            Ok((found[0].0, found[0].1.fields[1]))
        };
        let (remaining, mut target) = one_link("remaining")?;
        for _ in 0..pending {
            if !matches!(g.get(target).map(|a|&a.kind),Some(Kind::Constructor(s)) if s=="Count")
                || !accounted.insert(target)
            {
                return Err("counter node is missing, shared or cyclic".into());
            }
            let links: Vec<_> = g
                .iter()
                .filter(|(_, a)| {
                    matches!(&a.kind,Kind::Record{role:Role::ObjectLink,label:Some(s)} if s=="next")
                        && a.fields[0] == target
                })
                .collect();
            if links.len() != 1 || !accounted.insert(links[0].0) {
                return Err("counter link mismatch".into());
            }
            target = links[0].1.fields[1];
        }
        if target != self.end {
            return Err("counter does not equal pending work".into());
        }
        accounted.insert(remaining);
        let (phase, value) = one_link("phase")?;
        let switch = match g.get(value).map(|a| &a.kind) {
            Some(Kind::Constructor(s)) if s == "Work" => false,
            Some(Kind::Constructor(s)) if s == "Switch" => true,
            _ => return Err("invalid phase".into()),
        };
        if !accounted.insert(phase) || !accounted.insert(value) {
            return Err("phase must be separate from payload and quoted syntax".into());
        }
        if accounted.len() != g.len() {
            return Err("unexpected atoms outside certified family".into());
        }
        Ok(State {
            statuses,
            pending,
            switch,
            policy: active[0],
        })
    }

    /// Explicit family enumeration, not a replacement for general matching.
    /// Used only after full manifest membership checks; each selected binding
    /// must still pass Execution::prepare and commit. No exported scheduler
    /// continuation or default-runtime throughput claim follows from this path.
    pub fn bindings(&self, g: &Graph) -> Result<Vec<engine::OperationalBinding>, String> {
        let s = self.check(g)?;
        let owned = |head, role| {
            g.iter()
                .find(|(_, a)| a.kind.role() == Some(role) && a.fields[0] == head)
                .unwrap()
                .0
        };
        let object_link = |label: &str, from| {
            g.iter()
                .find(|(_, a)| {
                    matches!(&a.kind,Kind::Record{role:Role::ObjectLink,label:Some(l)} if l==label)
                        && a.fields[0] == from
                })
                .unwrap()
                .0
        };
        let phase = object_link("phase", self.control);
        let phase_value = g.get(phase).unwrap().fields[1];
        let remaining = object_link("remaining", self.control);
        let make = |head, values: Vec<AtomKey>| {
            engine::OperationalBinding::new(
                head,
                self.templates[&head].iter().copied().zip(values).collect(),
            )
        };
        if s.pending > 0 && !s.switch {
            let top = g.get(remaining).unwrap().fields[1];
            let next = object_link("next", top);
            let tail = g.get(next).unwrap().fields[1];
            return Ok(s
                .statuses
                .iter()
                .filter(|(_, label)| label.as_str() == "pending")
                .map(|(item, _)| {
                    make(
                        self.processors[s.policy],
                        vec![
                            self.markers[s.policy],
                            self.control,
                            phase_value,
                            phase,
                            top,
                            tail,
                            next,
                            remaining,
                            *item,
                            object_link("pending", *item),
                        ],
                    )
                })
                .collect());
        }
        if !s.switch && self.idle.is_empty() {
            return Ok(Vec::new());
        }
        let source = if s.switch {
            self.switches[s.policy]
        } else {
            self.idle[s.policy]
        };
        let a = self.processors[s.policy];
        let b = self.processors[1 - s.policy];
        let ma = self.quoted_markers[s.policy];
        let mb = self.quoted_markers[1 - s.policy];
        let membership = |head, marker| {
            g.iter()
                .find(|(_, x)| x.kind.role() == Some(Role::Lhs) && x.fields == [head, marker])
                .unwrap()
                .0
        };
        let mut values = vec![
            a,
            b,
            ma,
            mb,
            membership(a, ma),
            membership(b, mb),
            owned(a, Role::ModeObject),
            owned(b, Role::ModeObject),
            owned(a, Role::Revision),
            owned(b, Role::Revision),
            owned(a, Role::Active),
            self.control,
            phase_value,
            phase,
        ];
        if !s.switch {
            values.extend([self.end, remaining]);
        }
        Ok(vec![make(source, values)])
    }
}

/// Independent domain readback, not by itself a full membership certificate.
pub fn statuses(g: &Graph, items: &[AtomKey]) -> Result<BTreeMap<AtomKey, String>, String> {
    let mut out = BTreeMap::new();
    for item in items {
        if !matches!(g.get(*item).map(|a|&a.kind),Some(Kind::Constructor(s)) if s.starts_with("item:"))
        {
            return Err("item missing or changed".into());
        }
        let records: Vec<_> = g
            .iter()
            .filter(|(_, a)| a.kind.role() == Some(Role::ObjectLink) && a.fields.contains(item))
            .collect();
        if records.len() != 1 {
            return Err("item must have exactly one status".into());
        }
        let (_, a) = records[0];
        if a.fields != [*item, *item] {
            return Err("status must be a self link".into());
        }
        let Kind::Record {
            label: Some(label), ..
        } = &a.kind
        else {
            return Err("missing status label".into());
        };
        if !["pending", "route-a", "route-b"].contains(&label.as_str()) {
            return Err("unknown status".into());
        }
        out.insert(*item, label.clone());
    }
    Ok(out)
}
