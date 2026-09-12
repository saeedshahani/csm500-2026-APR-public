use engine::scheduling::{candidate_key, Scheduler, SchedulingStrategy};
use engine::{Execution, ExecutionLimits};
use graph_core::{fixture, validate};
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
    let fixture = fixture::load(include_str!("../../../examples/fixtures/b2.atoms"))?;
    let names = &fixture.names;
    let head = names["sl"];
    println!(
        "SelfLower before: {} atoms, rank {}",
        fixture.graph.len(),
        validate(&fixture.graph)?.rules[&head].rank
    );
    let mut execution = Execution::new(fixture.graph, ExecutionLimits::default())?;
    let mut enabled = execution.enabled()?.into_steps();
    enabled.sort_by_cached_key(candidate_key);
    let selected = Scheduler::Deterministic
        .select(&enabled)
        .ok_or("SelfLower has no enabled candidate")?;
    let event = execution.commit(enabled.swap_remove(selected))?;
    println!(
        "SelfLower after: {} atoms, rank {}; consumed {}, produced {}, revised {} rule(s)",
        execution.graph().len(),
        validate(execution.graph())?.rules[&head].rank,
        event.consumed.len(),
        event.produced.len(),
        event.altered.len()
    );
    Ok(())
}
