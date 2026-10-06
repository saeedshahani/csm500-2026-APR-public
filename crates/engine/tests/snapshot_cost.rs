#[allow(dead_code)]
mod support;
use engine::observation::Observer;
use engine::recording::RecordingLimits;
use engine::runner::{RunLimits, Runner};
use engine::scheduling::Scheduler;
use engine::trace::{GraphData, TraceDocument, TraceLimits, TraceOptions, VerifiedTrace};
use engine::ExecutionLimits;
use graph_core::{ErrorKind, Graph, GraphLimits, Kind, Sort};
use support::*;

fn history(graph: Graph, steps: u64) -> VerifiedTrace {
    let mut runner = Runner::new(
        graph,
        ExecutionLimits::default(),
        Scheduler::seeded(42),
        Observer::all(),
    )
    .unwrap();
    runner.enable_recording(RecordingLimits::default()).unwrap();
    runner
        .run(
            RunLimits {
                max_steps: steps,
                ..Default::default()
            },
            None,
            || false,
        )
        .unwrap();
    let limits = TraceLimits::default();
    let doc = TraceDocument::from_runner(
        &runner,
        None,
        TraceOptions {
            checkpoint_interval: 1,
            ..Default::default()
        },
        limits,
    )
    .unwrap();
    VerifiedTrace::import(&doc.to_json(limits.max_bytes).unwrap(), limits).unwrap()
}

fn exact(trace: &VerifiedTrace, state: usize) {
    let data = GraphData::from_graph(&trace.state_from_initial(state).unwrap());
    let payload = serde_json::to_string(&data).unwrap();
    let envelope = format!("{{\"schema\":\"identity-snapshot-json-v1\",\"graph\":{payload}}}");
    let cost = trace.snapshot_cost(state, envelope.len()).unwrap();
    assert_eq!(cost.payload_bytes.0 as usize, payload.len());
    assert_eq!(cost.total_bytes.0 as usize, envelope.len());
    assert_eq!(
        cost.envelope_bytes.0,
        cost.total_bytes.0 - cost.payload_bytes.0
    );
    assert_eq!(cost.total_bits.0, 8 * envelope.len() as u64);
    assert_eq!(
        cost.snapshot_sha256,
        engine::trace::sha256(envelope.as_bytes())
    );
    assert_eq!(cost.next_identity, data.next_identity);
    let restored: GraphData = serde_json::from_str(&payload).unwrap();
    assert_eq!(
        GraphData::from_graph(&restored.restore(&GraphLimits::default()).unwrap()),
        data
    );
    assert_eq!(
        trace
            .snapshot_cost(state, envelope.len() - 1)
            .unwrap_err()
            .kind(),
        ErrorKind::ResourceExhaustion
    );
    assert_eq!(trace.snapshot_cost(state, envelope.len()).unwrap(), cost);
}

#[test]
fn empty_and_unicode_payloads_count_actual_utf8_and_framing() {
    exact(&history(Graph::new(), 0), 0);
    let mut g = Graph::new();
    let a = ctor(&mut g, "λ 🦀 \" \\ \n");
    link(&mut g, "关系", a, a);
    let trace = history(g, 0);
    exact(&trace, 0);
    let report = trace.snapshot_cost(0, 1_000_000).unwrap();
    assert_eq!(
        report.label_utf8_bytes.0,
        ("λ 🦀 \" \\ \n".len() + "关系".len()) as u64
    );
    assert_eq!(report.atoms.0, 2);
    assert_eq!(report.fields.0, 2);
}

#[test]
fn cycles_gapped_identities_and_allocator_state_survive_measurement() {
    // Unowned Nat cycles are valid carriers; they cannot serve as rank chains.
    let atoms = [
        (3, Kind::Nat, vec![91]),
        (7, Kind::Nat, vec![7]),
        (91, Kind::Nat, vec![3]),
    ];
    let g = Graph::restore_identity_snapshot(&atoms, 1000, &GraphLimits::default()).unwrap();
    let t = history(g, 0);
    exact(&t, 0);
    let first = t.snapshot_cost(0, 10000).unwrap();
    assert_eq!(first.next_identity.0, 1000);
    let changed = history(
        Graph::restore_identity_snapshot(&atoms, 1001, &GraphLimits::default()).unwrap(),
        0,
    );
    let second = changed.snapshot_cost(0, 10000).unwrap();
    assert_ne!(first.snapshot_sha256, second.snapshot_sha256);
    assert_eq!(t.snapshot_cost(0, 10000).unwrap(), first);
}

#[test]
fn aliasing_duplication_and_order_remain_distinct() {
    let mut g = Graph::new();
    let a = ctor(&mut g, "same");
    let b = ctor(&mut g, "same");
    let mut alias = g.clone();
    link(&mut alias, "ref", a, a);
    let mut ordered = g.clone();
    link(&mut ordered, "ref", a, b);
    link(&mut g, "ref", b, a);
    let traces = [history(alias, 0), history(ordered, 0), history(g, 0)];
    let mut digests = std::collections::BTreeSet::new();
    for trace in &traces {
        exact(trace, 0);
        digests.insert(trace.snapshot_cost(0, 10000).unwrap().snapshot_sha256);
    }
    assert_eq!(digests.len(), 3);
}

#[test]
fn resident_rules_revisions_and_every_recorded_state_are_measured() {
    let mut g = Graph::new();
    let h = rule(&mut g, 0, false, true);
    let x = variable(&mut g, Sort::Constructor);
    let y = variable(&mut g, Sort::Constructor);
    let old = link(&mut g, "pending", x, y);
    let new = link(&mut g, "done", x, y);
    sides(&mut g, h, &[x, y, old], &[x, y, new]);
    let a = ctor(&mut g, "A");
    let b = ctor(&mut g, "B");
    link(&mut g, "pending", a, b);
    link(&mut g, "pending", a, b);
    let trace = history(g, 2);
    assert_eq!(trace.document().events.len(), 2);
    let before = trace.document().clone();
    for i in 0..=2 {
        exact(&trace, i);
    }
    assert!(trace.snapshot_cost(3, 10000).is_err());
    assert_eq!(
        trace.snapshot_cost(0, 0).unwrap_err().kind(),
        ErrorKind::ResourceExhaustion
    );
    assert_eq!(trace.document(), &before);
    assert_ne!(
        trace.snapshot_cost(0, 10000).unwrap().snapshot_sha256,
        trace.snapshot_cost(2, 10000).unwrap().snapshot_sha256
    );
}

#[test]
fn unchanged_entity_count_does_not_imply_unchanged_storage() {
    let mut short = Graph::new();
    ctor(&mut short, "x");
    let mut long = Graph::new();
    ctor(&mut long, &"x".repeat(1024));
    let a = history(short, 0).snapshot_cost(0, 10000).unwrap();
    let b = history(long, 0).snapshot_cost(0, 10000).unwrap();
    assert_eq!(a.atoms, b.atoms);
    assert_eq!(a.fields, b.fields);
    assert_eq!(b.total_bytes.0 - a.total_bytes.0, 1023);
}

#[test]
fn public_identity_range_is_not_limited_to_javascript_numbers() {
    let id = (1u64 << 53) + 7;
    let g = Graph::restore_identity_snapshot(
        &[(id, Kind::Constructor("large identity".into()), vec![])],
        u64::MAX,
        &GraphLimits::default(),
    )
    .unwrap();
    let t = history(g, 0);
    exact(&t, 0);
    let report = t.snapshot_cost(0, 10000).unwrap();
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["next_identity"], u64::MAX.to_string());
}
