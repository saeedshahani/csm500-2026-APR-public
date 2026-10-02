#[path = "support/inert_cases.rs"]
mod cases;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 4, "CASE N MAX_STATES full|inert");
    let (g, c) = cases::case(&args[0], args[1].parse().unwrap());
    let limits = engine::stability::Limits {
        max_states: args[2].parse().unwrap(),
        max_edges: 10000,
        max_retained_units: 10_000_000,
        max_comparison_work: 100_000_000,
        ..Default::default()
    };
    let start = std::time::Instant::now();
    let result = match args[3].as_str() {
        "full" => engine::meta_activity::analyse(g, c.as_ref(), limits),
        "inert" => engine::meta_activity::analyse_inert(g, c.as_ref(), limits, "Log"),
        _ => panic!("mode must be full or inert"),
    };
    let result = match result {
        Ok(r) => serde_json::to_value(r).unwrap(),
        Err(e) => {
            serde_json::json!({"complete":false,"verdict":if e.kind()==graph_core::ErrorKind::ResourceExhaustion {"inconclusive"} else {"unsupported"},"reason":e.to_string()})
        }
    };
    println!(
        "{}",
        serde_json::json!({"result":result,"elapsed_ns":start.elapsed().as_nanos()})
    );
}
