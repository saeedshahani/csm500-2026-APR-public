//! Deterministic finite research cells; not part of engine admission.
#[path = "../tests/support/mod.rs"]
#[allow(dead_code)]
mod support;
use engine::counterfactual::{analyse_query, Limits, Query};
use engine::observation::Observer;
use engine::recording::{EventId, Recording};
use engine::trace::{TraceDocument, VerifiedTrace};
use engine::Execution;
use graph_core::Graph;
use support::*;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "interventions conjunction|alternatives PREFIX (0..8)"
    );
    let n: usize = args[2].parse().unwrap();
    assert!(n <= 8);
    let conjunction = match args[1].as_str() {
        "conjunction" => true,
        "alternatives" => false,
        _ => panic!("unknown family"),
    };
    let mut g = Graph::new();
    let mut producers = Vec::new();
    for i in 0..n {
        let source = rule(&mut g, 0, false, true);
        let item = ctor(
            &mut g,
            &format!("Product{}", if conjunction { i } else { 0 }),
        );
        sides(&mut g, source, &[], &[item]);
        producers.push(source);
    }
    let target = rule(&mut g, 0, false, true);
    let pattern: Vec<_> = (0..if conjunction { n } else { usize::from(n > 0) })
        .map(|i| ctor(&mut g, &format!("Product{i}")))
        .collect();
    sides(&mut g, target, &pattern, &pattern);
    let mut run = Execution::new(g, Default::default()).unwrap();
    let mut log = Recording::new(&run, Default::default()).unwrap();
    for source in producers.into_iter().chain([target]) {
        let step = run.enabled_for(source).unwrap().into_steps().pop().unwrap();
        log.commit(&mut run, step).unwrap();
    }
    let doc = TraceDocument::from_recording(
        &log,
        &Observer::all(),
        None,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let verified = VerifiedTrace::verify(doc.clone(), Default::default()).unwrap();
    let mut reports = Vec::new();
    for query in [Query::ExactOccurrence, Query::SourceRevision] {
        let start = std::time::Instant::now();
        let report = analyse_query(&verified, EventId(n as u64), query, Limits::default()).unwrap();
        reports.push(serde_json::json!({"report":report,"elapsed_ns":start.elapsed().as_nanos().to_string()}));
    }
    println!(
        "{}",
        serde_json::json!({"family":args[1],"prefix":n,"trace":doc,"reports":reports})
    );
}
