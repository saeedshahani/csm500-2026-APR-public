#[path = "support/meta_cases.rs"]
mod cases;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3, "CASE N MAX_STATES");
    let (g, c) = cases::case(&args[0], args[1].parse().unwrap());
    let limits = engine::stability::Limits {
        max_states: args[2].parse().unwrap(),
        max_edges: 10000,
        max_retained_units: 10_000_000,
        max_comparison_work: 100_000_000,
        ..Default::default()
    };
    let start = std::time::Instant::now();
    let value = match engine::meta_activity::analyse(g, c.as_ref(), limits) {
        Ok(r) => serde_json::to_value(r).unwrap(),
        Err(e) if e.kind() == graph_core::ErrorKind::ResourceExhaustion => {
            serde_json::json!({"schema":"meta-activity-analysis-v1","complete":false,"verdict":"inconclusive","reason":e.to_string()})
        }
        Err(e) => panic!("{e}"),
    };
    println!(
        "{}",
        serde_json::json!({"result":value,"elapsed_ns":start.elapsed().as_nanos()})
    );
}
