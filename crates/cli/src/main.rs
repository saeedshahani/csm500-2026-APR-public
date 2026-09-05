use graph_core::{fixture, validate};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "demo".into());
    if command != "demo" {
        return Err("usage: cli [demo]".into());
    }
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
