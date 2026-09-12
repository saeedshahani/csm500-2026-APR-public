#[path = "../../graph-core/tests/support/generated.rs"]
mod generated;
#[path = "../../parser/tests/support/mod.rs"]
mod oracle;
use engine::observation::{all, isomorphic, ObservationLimits};
use generated::*;
use graph_core::{Graph, GraphLimits};
use proptest::prelude::*;

fn observation(g: &Graph) -> engine::observation::Observation {
    all(g, GraphLimits::default(), ObservationLimits::default()).unwrap()
}
#[test]
fn generated_complete_carriers_roundtrip_and_obey_equivalence_laws() {
    check(
        "parser and equivalence",
        "proptest-regressions/roundtrip.txt",
        (objects(), 0usize..4, any::<bool>(), 0usize..4),
        |(recipe, rank, active, depth)| {
            for g in [
                object_graph(&recipe, false),
                lowerer(rank, active, !active),
                nested(depth, active),
            ] {
                let text = parser::print(&g).unwrap();
                let h = parser::load(&text).unwrap().into_graph();
                let k = oracle::reversed(&h);
                prop_assert!(oracle::isomorphic(&g, &h, 1_000_000).unwrap());
                for (a, b) in [(&g, &g), (&g, &h), (&h, &g), (&h, &k), (&g, &k)] {
                    prop_assert!(isomorphic(&observation(a), &observation(b), 5_000_000).unwrap());
                }
            }
            Ok(())
        },
    );
}

#[test]
fn shrunken_inactive_depth_three_package_comparison_regression() {
    // Minimized from persisted seed 7817a634...: no object edges, rank zero,
    // inactive, depth three. S3.D/E exhausted five million units here.
    let graph = nested(3, false);
    let renamed = oracle::reversed(&graph);
    assert!(isomorphic(&observation(&graph), &observation(&renamed), 1_000_000).unwrap());
    assert!(oracle::isomorphic(&graph, &renamed, 1_000_000).unwrap());
}
