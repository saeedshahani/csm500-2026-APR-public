//! Fixed release-process cost probe; not a second lambda evaluator.
use parser::lambda::{self, Limits};
use std::time::Instant;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "PROFILE DEPTH");
    let profile = &args[1];
    let depth: usize = args[2].parse().unwrap();
    assert!(depth <= 32);
    let open = profile == lambda::OPEN_PROFILE;
    let mut source = if open { "database" } else { "(\\z. z)" }.to_string();
    for _ in 0..depth {
        source = format!("((\\z. z) {source})");
    }
    let start = Instant::now();
    let names = if open {
        vec!["database".into()]
    } else {
        vec![]
    };
    let encoded = lambda::compile_profile(&source, profile, names, Limits::default()).unwrap();
    let compile_ns = start.elapsed().as_nanos();
    let initial_atoms = encoded.graph.len();
    let start = Instant::now();
    let mut limits = engine::ExecutionLimits::default();
    limits.search.max_work = 50_000_000;
    let mut execution = engine::Execution::new(encoded.graph, limits).unwrap();
    let mut commits = 0;
    let mut beta = 0;
    let mut terminal = false;
    for _ in 0..256 {
        let enabled = execution.enabled().unwrap();
        if enabled.steps().is_empty() {
            terminal = true;
            break;
        }
        assert_eq!(enabled.steps().len(), 1);
        let rewrite = enabled.into_steps().pop().unwrap();
        beta += usize::from(encoded.rules[&rewrite.event().source] == lambda::Transition::Beta);
        execution.commit(rewrite).unwrap();
        commits += 1;
    }
    let execution_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let readback = lambda::interpret(execution.graph(), Limits::default())
        .unwrap()
        .unwrap();
    let readback_ns = start.elapsed().as_nanos();
    assert!(terminal);
    assert_eq!(beta, depth);
    assert_eq!(readback, if open { "database" } else { "(λ.0)" });
    println!(
        "{}",
        serde_json::json!({"schema":"lambda-open-cost-v1","profile":profile,
        "depth":depth,"source":source,"compile_ns":compile_ns,"execution_ns":execution_ns,
        "readback_ns":readback_ns,"initial_atoms":initial_atoms,"final_atoms":execution.graph().len(),
        "commits":commits,"beta":beta,"terminal":terminal,"readback":readback})
    );
}
