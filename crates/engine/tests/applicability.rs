mod support;
use engine::{Execution, ExecutionLimits};
use graph_core::{fixture, ErrorKind, Graph, Kind, Role, Sort};
use support::*;

fn run(g: Graph) -> Execution {
    Execution::new(g, ExecutionLimits::default()).unwrap()
}

#[test]
fn stale_match_after_deletion_is_rejected_and_commit_emits_one_event() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    sides(&mut g, h, &[x], &[]);
    let object = ctor(&mut g, "object");
    let mut execution = run(g);
    let first = execution.enabled().unwrap().into_steps().pop().unwrap();
    let stale = execution.enabled().unwrap().into_steps().pop().unwrap();
    let event = execution.commit(first).unwrap();
    assert!(event.consumed.contains(&object));
    assert_eq!(
        execution.commit(stale).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert_eq!(execution.committed_steps(), 1);
    assert!(execution.enabled().unwrap().steps().is_empty());
}

#[test]
fn changed_incoming_incidence_invalidates_proposal_without_changing_source_revision() {
    let mut g = Graph::new();
    let delete = rule(&mut g, 0, false, true);
    let pattern = ctor(&mut g, "A");
    sides(&mut g, delete, &[pattern], &[]);
    let attach = rule(&mut g, 0, false, true);
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    let e = link(&mut g, "incoming", b, a);
    sides(&mut g, attach, &[a], &[a, b, e]);
    ctor(&mut g, "A");
    let revision = owned(&g, delete, Role::Revision);
    let mut execution = run(g);
    let stale = execution
        .enabled_for(delete)
        .unwrap()
        .into_steps()
        .pop()
        .unwrap();
    let edit = execution
        .enabled_for(attach)
        .unwrap()
        .into_steps()
        .pop()
        .unwrap();
    execution.commit(edit).unwrap();
    assert_eq!(owned(execution.graph(), delete, Role::Revision), revision);
    assert_eq!(
        execution.commit(stale).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    let enabled = execution.enabled_for(delete).unwrap();
    assert!(enabled.steps().is_empty());
    assert_eq!(enabled.rejected()[0].diagnostic.kind(), ErrorKind::Dangling);
}

#[test]
fn stale_revision_is_rejected_while_the_source_head_survives() {
    let f = fixture::load(include_str!("../../../examples/fixtures/b2.atoms")).unwrap();
    let h = f.names["sl"];
    let old = f.names["sl_revision"];
    let mut execution = run(f.graph);
    let first = execution
        .enabled_for(h)
        .unwrap()
        .into_steps()
        .pop()
        .unwrap();
    let stale = execution
        .enabled_for(h)
        .unwrap()
        .into_steps()
        .pop()
        .unwrap();
    let event = execution.commit(first).unwrap();
    assert_eq!(event.source_revision, old);
    assert!(execution.graph().get(h).is_some());
    assert!(execution.graph().get(old).is_none());
    assert_eq!(
        execution.commit(stale).unwrap_err().kind(),
        ErrorKind::StaleCandidate
    );
    assert!(execution.bindings(h).unwrap().is_empty());
}

#[test]
fn fresh_installation_then_new_rule_execution_is_fully_automatic() {
    let f = fixture::load(include_str!("../../../examples/fixtures/a4.atoms")).unwrap();
    let install = f.names["iw"];
    let mut g = f.graph;
    let input = ctor(&mut g, "Input");
    let mut execution = run(g);
    let proposal = execution
        .enabled_for(install)
        .unwrap()
        .into_steps()
        .pop()
        .unwrap();
    let event = execution.commit(proposal).unwrap();
    let head = *event.entered.first().unwrap();
    let mut enabled = execution.enabled_for(head).unwrap().into_steps();
    assert_eq!(enabled.len(), 1);
    let proposal = enabled.pop().unwrap();
    assert!(proposal
        .binding()
        .assignments()
        .values()
        .any(|id| *id == input));
    let result = execution.commit(proposal).unwrap();
    assert_eq!(result.produced.len(), 2);
    assert!(result.preserved.contains(&input));
    graph_core::validate(execution.graph()).unwrap();
}

#[test]
fn allocator_and_order_do_not_depend_on_rejected_hypothetical_candidates() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    let output = ctor(&mut g, "new");
    sides(&mut g, h, &[x], &[output]);
    let doomed = ctor(&mut g, "bad");
    let kept = ctor(&mut g, "good");
    link(&mut g, "self", doomed, doomed);
    let mut execution = run(g);
    let next = execution.graph().clone().insert(Kind::Nat, &[]).unwrap();
    for _ in 0..3 {
        let enabled = execution.enabled_for(h).unwrap();
        assert_eq!(enabled.rejected().len(), 1);
        assert_eq!(enabled.steps().len(), 1);
        assert_eq!(enabled.steps()[0].binding().assignments()[&x], kept);
        assert_eq!(*enabled.steps()[0].event().produced.first().unwrap(), next);
    }
    let step = execution
        .enabled_for(h)
        .unwrap()
        .into_steps()
        .pop()
        .unwrap();
    let event = execution.commit(step).unwrap();
    assert_eq!(*event.produced.first().unwrap(), next);
}

#[test]
fn quotation_exit_and_reentry_in_one_history_require_new_tokens_both_times() {
    for fresh_entry in [false, true] {
        let mut g = Graph::new();
        let p = rule(&mut g, 0, true, false);
        let q = rule(&mut g, 0, true, false);
        let original = owned(&g, q, Role::Revision);
        let mut sources = Vec::new();
        for quoting in [true, false] {
            let source = rule(&mut g, 1, true, true);
            let owner = variable(&mut g, Sort::RuleHead);
            let head = variable(&mut g, Sort::RuleHead);
            let zero = g.insert(Kind::Nat, &[]).unwrap();
            let rank = record(&mut g, Role::Rank, &[head, zero]);
            let mode = record(&mut g, Role::ModeMeta, &[head]);
            let revision = record(&mut g, Role::Revision, &[head]);
            let owner_rev = record(&mut g, Role::Revision, &[owner]);
            let new_owner = record(&mut g, Role::Revision, &[owner]);
            let replacement = if quoting || fresh_entry {
                record(&mut g, Role::Revision, &[head])
            } else {
                revision
            };
            let mut lhs = vec![owner, owner_rev, head, zero, rank, mode, revision];
            let mut rhs = vec![owner, new_owner, head, zero, rank, mode, replacement];
            for item in [
                head,
                zero,
                rank,
                mode,
                if quoting { replacement } else { revision },
            ] {
                let membership = record(&mut g, Role::Rhs, &[owner, item]);
                if quoting {
                    rhs.push(membership);
                } else {
                    lhs.push(membership);
                }
            }
            sides(&mut g, source, &lhs, &rhs);
            sources.push((source, owner, head));
        }
        let mut execution = run(g);
        let (quote, owner, head) = sources[0];
        let step = execution
            .enabled_for(quote)
            .unwrap()
            .into_steps()
            .into_iter()
            .find(|step| {
                step.binding().assignments()[&owner] == p
                    && step.binding().assignments()[&head] == q
            })
            .unwrap();
        let exit = execution.commit(step).unwrap();
        assert!(exit.exited.contains(&q));
        assert!(exit.consumed.contains(&original));
        let inert = owned(execution.graph(), q, Role::Revision);
        assert_ne!(inert, original);
        assert!(exit.produced.contains(&inert));
        let entry = execution.enabled_for(sources[1].0).unwrap();
        if fresh_entry {
            assert_eq!(entry.steps().len(), 1);
            let event = execution.commit(entry.into_steps().pop().unwrap()).unwrap();
            assert!(event.entered.contains(&q));
            assert!(event.consumed.contains(&inert));
            let current = execution.view().unwrap().rule(q).unwrap().revision;
            assert!(event.produced.contains(&current));
            assert_ne!(current, original);
            assert_ne!(current, inert);
        } else {
            assert!(entry.steps().is_empty());
            assert!(entry
                .rejected()
                .iter()
                .any(|r| r.diagnostic.kind() == ErrorKind::Lifecycle));
            assert!(execution.view().unwrap().rule(q).is_none());
        }
    }
}
