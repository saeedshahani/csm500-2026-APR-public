use graph_core::{AtomKey, Binding, Graph, Kind, Role, Sort};

pub fn record(g: &mut Graph, role: Role, fields: &[AtomKey]) -> AtomKey {
    g.insert(Kind::record(role), fields).unwrap()
}
pub fn variable(g: &mut Graph, sort: Sort) -> AtomKey {
    g.insert(Kind::variable(sort, Binding::Free), &[]).unwrap()
}
pub fn ctor(g: &mut Graph, label: &str) -> AtomKey {
    g.insert(Kind::Constructor(label.into()), &[]).unwrap()
}
pub fn link(g: &mut Graph, label: &str, a: AtomKey, b: AtomKey) -> AtomKey {
    g.insert(
        Kind::Record {
            role: Role::ObjectLink,
            label: Some(label.into()),
        },
        &[a, b],
    )
    .unwrap()
}
pub fn rule(g: &mut Graph, rank: usize, meta: bool, active: bool) -> AtomKey {
    let h = g.insert(Kind::RuleHead, &[]).unwrap();
    let mut n = g.insert(Kind::Nat, &[]).unwrap();
    for _ in 0..rank {
        n = g.insert(Kind::Nat, &[n]).unwrap();
    }
    record(g, Role::Rank, &[h, n]);
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
pub fn sides(g: &mut Graph, h: AtomKey, l: &[AtomKey], r: &[AtomKey]) {
    for a in l {
        record(g, Role::Lhs, &[h, *a]);
    }
    for a in r {
        record(g, Role::Rhs, &[h, *a]);
    }
}
pub fn owned(g: &Graph, h: AtomKey, role: Role) -> AtomKey {
    g.iter()
        .find(|(_, a)| a.kind.role() == Some(role) && a.fields[0] == h)
        .unwrap()
        .0
}
