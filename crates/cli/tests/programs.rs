//! Expected carriers are hand-derived files, never engine-generated snapshots.
#[path = "../../parser/tests/support/mod.rs"]
mod iso;
use engine::{Event, Execution, ExecutionLimits, PreparedStep};
use graph_core::{AtomKey, ErrorKind, Graph, Kind, Role};
use parser::{load, LoadedProgram};

fn specimen(name: &str) -> LoadedProgram {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/programs")
        .join(format!("{name}.graph"));
    load(&std::fs::read_to_string(path).unwrap()).unwrap()
}
fn start(name: &str) -> (LoadedProgram, Execution) {
    let input = specimen(name);
    let run = Execution::new(input.graph().clone(), ExecutionLimits::default()).unwrap();
    (input, run)
}
fn expected(run: &Execution, name: &str) {
    let hand = specimen(name);
    assert_eq!(
        iso::isomorphic(run.graph(), hand.graph(), 1_000_000),
        Ok(true),
        "{name}"
    );
    assert_eq!(
        iso::isomorphic(run.graph(), &iso::reversed(hand.graph()), 1_000_000),
        Ok(true),
        "renamed {name}"
    );
}
fn choose(run: &mut Execution, head: AtomKey, predicate: impl Fn(&PreparedStep) -> bool) -> Event {
    let step = run
        .enabled_for(head)
        .unwrap()
        .into_steps()
        .into_iter()
        .find(predicate)
        .expect("required candidate not enabled");
    run.commit(step).unwrap()
}
fn owned(g: &Graph, h: AtomKey, role: Role) -> AtomKey {
    g.iter()
        .find(|(_, a)| a.kind.role() == Some(role) && a.fields[0] == h)
        .unwrap()
        .0
}

#[test]
fn arithmetic_reaches_the_hand_derived_nonempty_normal_form() {
    let (input, mut run) = start("arithmetic");
    let event = choose(&mut run, input.name("az").unwrap(), |_| true);
    assert_eq!((event.consumed.len(), event.produced.len()), (5, 1));
    assert!(event.altered.is_empty());
    expected(&run, "arithmetic.expected");
    assert!(run.enabled().unwrap().steps().is_empty());
    assert_eq!((run.graph().len(), run.graph().field_count()), (27, 35));
}

#[test]
fn arithmetic_names_are_not_native_operators() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/programs/arithmetic.graph");
    let text = std::fs::read_to_string(path).unwrap();
    let changed = text
        .replace("\"Add\"", "\"Tea\"")
        .replace("\"Zero\"", "\"Cup\"");
    let mut run = Execution::new(
        load(&changed).unwrap().into_graph(),
        ExecutionLimits::default(),
    )
    .unwrap();
    let step = run.enabled().unwrap().into_steps().pop().unwrap();
    assert_eq!(run.commit(step).unwrap().consumed.len(), 5);
    assert!(run.enabled().unwrap().steps().is_empty());
    // Removing the graph-resident rule's activation disables the computation.
    let inactive = text
        .lines()
        .filter(|line| !line.contains("atom az_active ="))
        .collect::<Vec<_>>()
        .join("\n");
    let run = Execution::new(
        load(&inactive).unwrap().into_graph(),
        ExecutionLimits::default(),
    )
    .unwrap();
    assert!(run.enabled().unwrap().steps().is_empty());
}

#[test]
fn self_modification_changes_a_to_b_after_graph_resident_promotion() {
    let (input, mut run) = start("self-modify");
    let h = input.name("sl").unwrap();
    let mut member = input.name("sl_R_output_a").unwrap();
    let old_member = input.name("old_output_member").unwrap();
    for (round, label, golden) in [
        (0, "A", "self-modify.first.expected"),
        (1, "B", "self-modify.expected"),
    ] {
        let revision = run.view().unwrap().rule(h).unwrap().revision;
        let event = choose(&mut run, h, |s| s.event().consumed.contains(&member));
        assert_eq!((event.consumed.len(), event.produced.len()), (4, 5));
        assert_eq!(event.source_revision, revision);
        let state = run.view().unwrap();
        assert_eq!(state.rule(h).unwrap().rank, 0);
        assert_ne!(state.rule(h).unwrap().revision, revision);
        let visible: Vec<_> = event
            .produced
            .iter()
            .filter(|k| !state.configuration().quoted.contains_key(k))
            .filter_map(|k| match &run.graph().get(*k).unwrap().kind {
                Kind::Constructor(l) => Some(l.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(visible, [label]);
        expected(&run, golden);
        assert!(run.enabled_for(h).unwrap().steps().is_empty());
        member = *event
            .produced
            .iter()
            .find(|k| run.graph().get(**k).unwrap().kind.role() == Some(Role::Rhs))
            .unwrap();
        // The original template remains; only its matched live membership changes.
        assert!(run.graph().get(old_member).is_some());
        if round == 0 {
            let rank = owned(run.graph(), h, Role::Rank);
            let promoted = choose(&mut run, input.name("supervisor").unwrap(), |s| {
                s.event().consumed.contains(&rank)
            });
            assert_eq!((promoted.consumed.len(), promoted.produced.len()), (2, 3));
            assert_eq!(run.view().unwrap().rule(h).unwrap().rank, 1);
            expected(&run, "self-modify.promoted.expected");
        }
    }
}

#[test]
fn mutual_regress_is_rejected_without_changing_any_atom_or_revision() {
    let (_, run) = start("mutual-regress");
    let set = run.enabled().unwrap();
    assert!(set.steps().is_empty());
    assert!(set
        .rejected()
        .iter()
        .any(|r| r.diagnostic.kind() == ErrorKind::Guardedness));
    assert_eq!(run.committed_steps(), 0);
    expected(&run, "mutual-regress");
}

#[test]
fn staging_installs_a_private_rule_then_runs_it_on_the_authored_object() {
    let (input, mut run) = start("staging");
    let install = choose(&mut run, input.name("iw").unwrap(), |_| true);
    assert_eq!((install.consumed.len(), install.produced.len()), (0, 13));
    let h = *install.entered.first().unwrap();
    assert_eq!(run.view().unwrap().rule(h).unwrap().rank, 0);
    expected(&run, "staging.installed.expected");
    let event = choose(&mut run, h, |_| true);
    assert_eq!((event.consumed.len(), event.produced.len()), (0, 2));
    assert!(event.preserved.contains(&input.name("input").unwrap()));
    expected(&run, "staging.expected");
}

#[test]
fn restoration_returns_to_the_hand_derived_carrier_with_fresh_revisions() {
    let (input, mut run) = start("restoration");
    let initial = (run.graph().len(), run.graph().field_count());
    for _ in 0..4 {
        for (source, target) in [("p", "q"), ("q", "p")] {
            let h = input.name(target).unwrap();
            let rank = owned(run.graph(), h, Role::Rank);
            let revision = run.view().unwrap().rule(h).unwrap().revision;
            let event = choose(&mut run, input.name(source).unwrap(), |s| {
                s.event().consumed.contains(&rank)
            });
            assert_eq!((event.consumed.len(), event.produced.len()), (6, 5));
            assert_eq!(run.view().unwrap().rule(h).unwrap().rank, 0);
            assert_ne!(run.view().unwrap().rule(h).unwrap().revision, revision);
            let rank = owned(run.graph(), h, Role::Rank);
            let event = choose(&mut run, input.name("supervisor").unwrap(), |s| {
                s.event().consumed.contains(&rank)
            });
            assert_eq!((event.consumed.len(), event.produced.len()), (2, 3));
            assert_eq!(run.view().unwrap().rule(h).unwrap().rank, 1);
        }
        assert_eq!((run.graph().len(), run.graph().field_count()), initial);
        expected(&run, "restoration.expected");
    }
}
