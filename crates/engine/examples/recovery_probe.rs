//! Checked, family-specific research driver. Not the default Runner profile.
#[path = "support/recovery.rs"]
mod recovery;
use engine::{Execution, ExecutionLimits};
use serde_json::json;
use std::time::Instant;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("recovery_probe source|finite|modulating ITEMS STEP_BOUND");
        std::process::exit(2);
    }
    let n: usize = args[1].parse().expect("item count");
    let bound: usize = args[2].parse().expect("step bound");
    assert!(
        n <= 1000 && bound <= 10000,
        "bounded research inputs required"
    );
    assert!(["source", "finite", "modulating"].contains(&args[0].as_str()));
    let family = recovery::build(n, args[0] != "finite");
    if args[0] == "source" {
        print!("{}", parser::print(&family.graph).unwrap());
        return;
    }
    let initial_atoms = family.graph.len();
    let mut e = Execution::new(family.graph.clone(), ExecutionLimits::default()).unwrap();
    let started = Instant::now();
    let mut processed = 0;
    let mut modified = 0;
    let mut first_legitimate = if n == 0 { Some(0) } else { None };
    let mut normal = false;
    let check_started = Instant::now();
    graph_core::validate(e.graph()).unwrap();
    let core_validation_ns = check_started.elapsed().as_nanos();
    let check_started = Instant::now();
    let mut prior = family.check(e.graph()).unwrap();
    let membership_ns = check_started.elapsed().as_nanos();
    let mut enumerate_ns = 0;
    let mut admission_commit_ns = 0;
    for i in 0..bound {
        let phase = Instant::now();
        let mut bindings = family.bindings(e.graph()).unwrap();
        enumerate_ns += phase.elapsed().as_nanos();
        if bindings.is_empty() {
            normal = true;
            break;
        }
        let phase = Instant::now();
        let candidate = e.prepare(bindings.remove(0)).unwrap();
        let event = e.commit(candidate).unwrap();
        admission_commit_ns += phase.elapsed().as_nanos();
        if family.processors.contains(&event.source) {
            processed += 1;
        } else {
            modified += 1;
        }
        let state = family.check(e.graph()).unwrap();
        if prior.pending > 0 {
            assert!(state.potential() < prior.potential());
        } else {
            assert_eq!(state.statuses, prior.statuses);
        }
        if state.pending == 0 && first_legitimate.is_none() {
            first_legitimate = Some(i + 1);
        }
        prior = state;
    }
    if !normal {
        normal = family.bindings(e.graph()).unwrap().is_empty();
    }
    println!(
        "{}",
        json!({"schema":"recovery-probe-v1","profile":"checked-family-v1","items":n,
        "modulating":!family.idle.is_empty(),"step_bound":bound,"commits":e.committed_steps(),
        "processed":processed,"meta_steps":modified,"pending":prior.pending,"first_legitimate":first_legitimate,
        "outcome":if normal {"FamilyNormalForm"} else {"StepLimit"},
        "initial_atoms":initial_atoms,"final_atoms":e.graph().len(),"phase_ns":started.elapsed().as_nanos(),
        "initial_core_validation_ns":core_validation_ns,"initial_membership_ns":membership_ns,
        "enumerate_ns":enumerate_ns,"admission_commit_ns":admission_commit_ns})
    );
}
