use graph_core::{fixture, validate};
use std::collections::BTreeMap;
mod commands;

fn main() -> std::process::ExitCode {
    match commands::main_command() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("{}", error.message);
            std::process::ExitCode::from(error.code)
        }
    }
}

fn demo() -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = fixture::load(include_str!("../../../examples/fixtures/b2.atoms"))?;
    let names = &fixture.names;
    let head = names["sl"];
    let binding = [
        ("b_self", "sl"),
        ("b_rest", "sl_r0"),
        ("b_top", "sl_r1"),
        ("b_old_rank", "sl_rank"),
        ("b_old_rev", "sl_revision"),
    ]
    .into_iter()
    .map(|(p, t)| (names[p], names[t]))
    .collect::<BTreeMap<_, _>>();
    println!(
        "SelfLower before: {} atoms, rank {}",
        fixture.graph.len(),
        validate(&fixture.graph)?.rules[&head].rank
    );
    let event = engine::apply_with_binding(&mut fixture.graph, head, &binding)?;
    println!(
        "SelfLower after: {} atoms, rank {}; consumed {}, produced {}, revised {} rule(s)",
        fixture.graph.len(),
        validate(&fixture.graph)?.rules[&head].rank,
        event.consumed.len(),
        event.produced.len(),
        event.altered.len()
    );
    Ok(())
}
