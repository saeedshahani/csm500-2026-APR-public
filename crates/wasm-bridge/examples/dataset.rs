fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() > 4 {
        return Err("usage: dataset [count] [u32-seed] [fixed|guarded_enable] [graph|json]".into());
    }
    let count = args
        .first()
        .map_or("10", String::as_str)
        .parse::<usize>()
        .map_err(|_| "invalid item count")?;
    let seed = args
        .get(1)
        .map_or("42", String::as_str)
        .parse::<u32>()
        .map_err(|_| "invalid u32 seed")?;
    let policy = match args.get(2).map_or("fixed", String::as_str) {
        "fixed" => wasm_bridge::dataset::Policy::Fixed,
        "guarded_enable" => wasm_bridge::dataset::Policy::GuardedEnable,
        _ => return Err("policy must be fixed or guarded_enable".into()),
    };
    let dataset = wasm_bridge::dataset::generate(count, seed, policy)?;
    match args.get(3).map_or("graph", String::as_str) {
        "graph" => print!("{}", wasm_bridge::dataset::source(&dataset)?),
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&dataset).map_err(|e| e.to_string())?
        ),
        _ => return Err("output must be graph or json".into()),
    }
    Ok(())
}
