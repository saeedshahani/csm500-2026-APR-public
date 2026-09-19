//! Current-state decoding through authored programs and the checked engine API.
use engine::{Event, Execution, ExecutionLimits, OperationalBinding};
use graph_core::{fixture, AtomKey, Binding, ErrorKind, Graph, Kind, Role, Sort};
use parser::{load, print, LoadedProgram};
use std::collections::{BTreeMap, BTreeSet};

fn run(loaded: &LoadedProgram) -> Execution {
    Execution::new(loaded.graph().clone(), ExecutionLimits::default()).unwrap()
}
fn apply(
    execution: &mut Execution,
    head: AtomKey,
    assignments: BTreeMap<AtomKey, AtomKey>,
) -> Event {
    let before = execution.view().unwrap();
    let source = before.rule(head).unwrap().clone();
    let expected = OperationalBinding::new(head, assignments);
    // Query-local indexes must observe every lifecycle change exercised below,
    // including mode/activity changes, quotation, deletion and installation.
    let indexed = execution.enabled_compact().unwrap();
    assert!(indexed.bindings().contains(&expected));
    drop(indexed);
    let proposal = execution
        .enabled_for(head)
        .unwrap()
        .into_steps()
        .into_iter()
        .find(|p| p.binding() == &expected)
        .expect("authored binding not discovered and admitted");
    assert_eq!(proposal.event().source_revision, source.revision);
    assert!(source.representation.is_subset(&proposal.event().read));
    let event = execution.commit(proposal).unwrap();
    // Every reached graph also goes through the authored serialisation boundary.
    let text = print(execution.graph()).unwrap();
    assert_eq!(print(load(&text).unwrap().graph()).unwrap(), text);
    event
}
fn owned(graph: &Graph, head: AtomKey, role: Role) -> AtomKey {
    graph
        .iter()
        .find(|(_, a)| a.kind.role() == Some(role) && a.fields[0] == head)
        .unwrap()
        .0
}
fn header(name: &str, rank: usize, mode: &str, active: bool, body: &str) -> String {
    let mut cells = format!("atom {name}_z0 = Nat();");
    for i in 1..=rank {
        cells += &format!("atom {name}_z{i} = Nat({name}_z{});", i - 1);
    }
    format!("rule {name} {{ {cells} rank {name}_rank = {name}_z{rank}; mode {name}_mode = {mode}; revision {name}_rev; {} {body} }}", if active { format!("active {name}_active;") } else { String::new() })
}

#[test]
fn a3_discovers_exactly_one_live_rule_from_printed_ledger() {
    let graph = fixture::load(include_str!("../../../examples/fixtures/a2.atoms"))
        .unwrap()
        .graph;
    let loaded = load(&print(&graph).unwrap()).unwrap();
    let execution = run(&loaded);
    let view = execution.view().unwrap();
    let rules: Vec<_> = view.live_rules().collect();
    assert_eq!(rules.len(), 1);
    assert_eq!(view.active_rules().count(), 1);
    let r = rules[0];
    assert_eq!(
        (
            r.rank,
            r.meta,
            r.lhs.len(),
            r.rhs.len(),
            r.representation.len()
        ),
        (0, false, 7, 3, 24)
    );
    assert!(r.representation.contains(&r.revision));
    assert!(!r.body.contains(&r.revision));
}

#[test]
fn a4_quoted_head_is_not_live_until_a_fresh_copy_is_installed() {
    let loaded = load(include_str!("../../../examples/syntax/install-wrap.graph")).unwrap();
    let key = |name| loaded.name(name).unwrap();
    let mut execution = run(&loaded);
    let view = execution.view().unwrap();
    assert_eq!(view.live_rules().count(), 1);
    assert!(view.rule(key("wrap")).is_none());
    let installer = view.rule(key("install")).unwrap();
    assert!(installer.package_atoms.contains(&key("x")));
    assert!(installer.lhs.is_empty());
    let event = apply(&mut execution, key("install"), BTreeMap::new());
    let head = *event.entered.first().unwrap();
    assert_ne!(head, key("wrap"));
    let view = execution.view().unwrap();
    assert_eq!(view.active_rules().count(), 2);
    assert!(view.rule(key("wrap")).is_none());
    let rule = view.rule(head).unwrap();
    assert!(rule.representation.is_subset(&event.produced));
    assert_eq!(rule.rank, 0);
    let x = *rule.lhs.first().unwrap();
    assert_ne!(x, key("x"));
    assert_eq!(
        execution.graph().get(x).unwrap().kind,
        Kind::variable(Sort::Constructor, Binding::Free)
    );
    // The template variable remains quoted by the new live head, not the installer.
    assert_eq!(view.configuration().quoted.get(&x), Some(&head));
    // A concrete constructor in the original authored source is still quoted and
    // cannot be supplied as visible object data to the installed rule.
    assert_eq!(
        execution
            .prepare(OperationalBinding::new(
                head,
                BTreeMap::from([(x, key("box"))])
            ))
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidBinding
    );
}

fn activation_source() -> String {
    let target = header("target", 0, "ModeObject", false, "");
    let on = header("on",1,"ModeMeta",true, "atom on_h = PatternVariable[RuleHead] Free; atom on_v = Record[Revision](on_h); atom on_new = Record[Revision](on_h); atom on_a = Record[Active](on_h); lhs { on_lh: on_h; on_lv: on_v; } rhs { on_rh: on_h; on_rv: on_new; on_ra: on_a; }");
    let off = header("off",1,"ModeMeta",true, "atom off_h = PatternVariable[RuleHead] Free; atom off_v = Record[Revision](off_h); atom off_new = Record[Revision](off_h); atom off_a = Record[Active](off_h); lhs { off_lh: off_h; off_lv: off_v; off_la: off_a; } rhs { off_rh: off_h; off_rv: off_new; }");
    format!("program {{ {target} {on} {off} }}")
}

#[test]
fn authored_activation_deactivation_redecodes_and_invalidates_old_proposals() {
    let loaded = load(&activation_source()).unwrap();
    let key = |name| loaded.name(name).unwrap();
    let mut execution = run(&loaded);
    let inactive = execution
        .view()
        .unwrap()
        .rule(key("target"))
        .unwrap()
        .clone();
    assert!(!inactive.active);
    assert_eq!(execution.view().unwrap().live_rules().count(), 3);
    assert_eq!(execution.view().unwrap().active_rules().count(), 2);
    assert_eq!(
        execution
            .prepare(OperationalBinding::new(key("target"), BTreeMap::new()))
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidBinding
    );
    let on = apply(
        &mut execution,
        key("on"),
        BTreeMap::from([
            (key("on_h"), key("target")),
            (key("on_v"), inactive.revision),
        ]),
    );
    assert_eq!(on.altered, BTreeSet::from([key("target")]));
    assert!(on.entered.is_empty()); // Activation is not entry: the inactive head was already live.
    let current = execution
        .view()
        .unwrap()
        .rule(key("target"))
        .unwrap()
        .clone();
    assert!(current.active);
    assert_eq!((current.rank, current.meta), (inactive.rank, inactive.meta));
    assert_ne!(current.revision, inactive.revision);
    let proposal = execution
        .prepare(OperationalBinding::new(key("target"), BTreeMap::new()))
        .unwrap();
    let activity = owned(execution.graph(), key("target"), Role::Active);
    apply(
        &mut execution,
        key("off"),
        BTreeMap::from([
            (key("off_h"), key("target")),
            (key("off_v"), current.revision),
            (key("off_a"), activity),
        ]),
    );
    assert_eq!(
        execution.commit(proposal).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert!(
        !execution
            .view()
            .unwrap()
            .rule(key("target"))
            .unwrap()
            .active
    );
    assert_eq!(execution.view().unwrap().active_rules().count(), 2);
    // Detached inspection data is historical and cannot re-enable the rule.
    assert!(current.active);
    assert_eq!(
        execution
            .prepare(OperationalBinding::new(key("target"), BTreeMap::new()))
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidBinding
    );
}

#[test]
fn complete_head_deletion_then_unrelated_creation_never_reuses_identity() {
    let doomed = header("doomed", 0, "ModeObject", false, "");
    let erase = header("erase",1,"ModeMeta",true,"atom dh = PatternVariable[RuleHead] Free; atom dz = Nat(); atom dr = Record[Rank](dh,dz); atom dm = Record[ModeObject](dh); atom dv = Record[Revision](dh); lhs { dl_h: dh; dl_z: dz; dl_r: dr; dl_m: dm; dl_v: dv; }");
    let installer = include_str!("../../../examples/syntax/install-wrap.graph");
    let source = installer.replacen("program {", &format!("program {{ {doomed} {erase}"), 1);
    let loaded = load(&source).unwrap();
    let key = |n| loaded.name(n).unwrap();
    let mut execution = run(&loaded);
    let representation = execution
        .view()
        .unwrap()
        .rule(key("doomed"))
        .unwrap()
        .representation
        .clone();
    let old_max = execution
        .graph()
        .iter()
        .map(|(id, _)| id.number())
        .max()
        .unwrap();
    let event = apply(
        &mut execution,
        key("erase"),
        BTreeMap::from([
            (key("dh"), key("doomed")),
            (key("dz"), key("doomed_z0")),
            (key("dr"), key("doomed_rank")),
            (key("dm"), key("doomed_mode")),
            (key("dv"), key("doomed_rev")),
        ]),
    );
    assert_eq!(event.consumed, representation);
    assert_eq!(event.exited, BTreeSet::from([key("doomed")]));
    assert!(execution.view().unwrap().rule(key("doomed")).is_none());
    let event = apply(&mut execution, key("install"), BTreeMap::new());
    assert!(event.produced.iter().all(|k| k.number() > old_max));
    assert!(event.produced.is_disjoint(&representation));
    assert!(execution.graph().get(key("doomed")).is_none());
}

#[test]
fn authored_rhs_membership_edit_changes_later_output_a_to_b() {
    let loaded = load(include_str!("../../../examples/syntax/modify-output.graph")).unwrap();
    let key = |n| loaded.name(n).unwrap();
    let mut execution = run(&loaded);
    let first = apply(&mut execution, key("target"), BTreeMap::new());
    assert_eq!(
        execution
            .graph()
            .get(*first.produced.first().unwrap())
            .unwrap()
            .kind,
        Kind::Constructor("A".into())
    );
    let historical = execution
        .view()
        .unwrap()
        .rule(key("target"))
        .unwrap()
        .clone();
    let obsolete = execution
        .prepare(OperationalBinding::new(key("target"), BTreeMap::new()))
        .unwrap();
    let event = apply(
        &mut execution,
        key("supervisor"),
        BTreeMap::from([
            (key("h"), key("target")),
            (key("x"), key("output")),
            (key("old_member"), key("output_member")),
            (key("old_rev"), historical.revision),
        ]),
    );
    assert_eq!(event.altered, BTreeSet::from([key("target")]));
    assert_eq!(
        execution.commit(obsolete).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    let current = execution
        .view()
        .unwrap()
        .rule(key("target"))
        .unwrap()
        .clone();
    assert_ne!(current.revision, historical.revision);
    assert!(!current.rhs.contains(&key("output")));
    assert!(historical.rhs.contains(&key("output")));
    let second = apply(&mut execution, key("target"), BTreeMap::new());
    assert_eq!(second.source_revision, current.revision);
    assert_eq!(
        execution
            .graph()
            .get(*second.produced.first().unwrap())
            .unwrap()
            .kind,
        Kind::Constructor("B".into())
    );
    assert!(current.representation.is_subset(&second.read));
}

#[test]
fn self_rewrite_uses_source_pre_rank_revision_and_representation() {
    let f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let text = print(&f.graph).unwrap();
    let loaded = load(&text).unwrap();
    // Printer order has a documented mapping, used only to supply this binding.
    let names: BTreeMap<_, _> = f
        .names
        .iter()
        .map(|(name, id)| {
            (
                name.as_str(),
                loaded.name(&format!("a{}", id.number())).unwrap(),
            )
        })
        .collect();
    let mut execution = run(&loaded);
    let h = names["sl"];
    let before = execution.view().unwrap().rule(h).unwrap().clone();
    let event = apply(
        &mut execution,
        h,
        [
            ("b_self", "sl"),
            ("b_rest", "sl_r0"),
            ("b_top", "sl_r1"),
            ("b_old_rank", "sl_rank"),
            ("b_old_rev", "sl_revision"),
        ]
        .into_iter()
        .map(|(a, b)| (names[a], names[b]))
        .collect(),
    );
    let view = execution.view().unwrap();
    let after = view.rule(h).unwrap();
    assert_eq!((before.rank, after.rank), (1, 0));
    assert_eq!(event.source_revision, before.revision);
    assert_ne!(event.source_revision, after.revision);
    assert_eq!((before.meta, after.meta), (true, true));
    assert_eq!(before.lhs, after.lhs);
    assert_eq!(before.rhs, after.rhs);
    assert!(before.representation.is_subset(&event.read));
    assert!(!event.read.contains(&after.revision));
}

#[test]
fn mode_change_is_read_from_current_graph_not_a_retained_view() {
    let target = header("target", 0, "ModeObject", true, "");
    let change = header("change",1,"ModeMeta",true,"atom h = PatternVariable[RuleHead] Free; atom old = Record[ModeObject](h); atom oldv = Record[Revision](h); atom new = Record[ModeMeta](h); atom newv = Record[Revision](h); lhs { lh: h; lm: old; lv: oldv; } rhs { rh: h; rm: new; rv: newv; }");
    let loaded = load(&format!("program {{ {target} {change} }}")).unwrap();
    let key = |n| loaded.name(n).unwrap();
    let mut execution = run(&loaded);
    let before = execution
        .view()
        .unwrap()
        .rule(key("target"))
        .unwrap()
        .clone();
    apply(
        &mut execution,
        key("change"),
        BTreeMap::from([
            (key("h"), key("target")),
            (key("old"), key("target_mode")),
            (key("oldv"), before.revision),
        ]),
    );
    assert!(!before.meta);
    assert!(execution.view().unwrap().rule(key("target")).unwrap().meta);
}
