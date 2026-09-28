use engine::{
    observation::Observer,
    runner::{ExecutionProfile, Outcome, RunLimits, Runner},
    scheduling::Scheduler,
    trace::{TraceDocument, TraceLimits, TraceOptions, VerifiedTrace},
    ExecutionLimits,
};
const SOURCE: &str = include_str!("../../../examples/practical/month-normalisation.graph");

#[test]
fn distinct_rows_and_shared_dictionary_agree_across_schedules() {
    let source = SOURCE.replace("atom source =", "atom second = Constructor(\"Session:second\"); atom third = Constructor(\"Session:third\"); atom other = Record[ObjectLink](\"source-month\", second, june); atom last = Record[ObjectLink](\"source-month\", third, june); atom source =");
    let mut observations = Vec::new();
    let mut first_choices = std::collections::BTreeSet::new();
    for seed in 0..12 {
        let mut r = Runner::with_profile(
            parser::load(&source).unwrap().into_graph(),
            ExecutionLimits::default(),
            Scheduler::seeded(seed),
            Observer::all(),
            ExecutionProfile::JoinedV4,
        )
        .unwrap();
        r.enable_recording(Default::default()).unwrap();
        assert_eq!(
            r.run(
                RunLimits {
                    max_steps: 4,
                    detect_cycles: true,
                    ..Default::default()
                },
                None,
                || false
            )
            .unwrap()
            .outcome,
            Outcome::NormalForm
        );
        assert_eq!(r.execution().committed_steps(), 3);
        first_choices.insert(format!(
            "{:?}",
            r.recording().unwrap().events()[0].footprint
        ));
        observations.push(
            engine::observation::all(
                r.execution().graph(),
                Default::default(),
                Default::default(),
            )
            .unwrap(),
        );
    }
    assert!(first_choices.len() > 1);
    for observation in &observations[1..] {
        assert!(engine::observation::isomorphic(&observations[0], observation, 1_000_000).unwrap());
    }
    let mut limits = ExecutionLimits::default();
    limits.search.max_candidates = 2;
    let mut limited = Runner::with_profile(
        parser::load(&source).unwrap().into_graph(),
        limits,
        Scheduler::seeded(0),
        Observer::all(),
        ExecutionProfile::JoinedV4,
    )
    .unwrap();
    assert_eq!(
        limited
            .run(
                RunLimits {
                    max_steps: 4,
                    ..Default::default()
                },
                None,
                || false
            )
            .unwrap()
            .outcome,
        Outcome::ResourceLimit
    );
    assert_eq!(limited.execution().committed_steps(), 0);
}

#[test]
fn join_counterfactuals_match_complete_reference_admission() {
    let cases = [
        SOURCE.replace("atom source =", "atom other = Constructor(\"MonthNumber:07\"); atom alternative = Record[ObjectLink](\"normalizes-to\", june, other); atom source ="),
        SOURCE.replace("atom source =", "atom qs = Record[Rhs](normalise, source); atom qr = Record[Rhs](normalise, session); atom qv = Record[Rhs](normalise, june); atom source ="),
        SOURCE.replace("june, six", "june, june"),
        SOURCE.replace("session, june", "six, june"),
        SOURCE.replace("normalizes-to", "unrelated-lookup").replace("source-month", "input-category").replace("calendar-month", "joined-category"),
        SOURCE.replace("active enabled;", ""),
    ];
    for source in cases {
        for seed in [1, 2, 3] {
            let mut histories = Vec::new();
            for profile in [ExecutionProfile::ReferenceV1, ExecutionProfile::JoinedV4] {
                let mut r = Runner::with_profile(
                    parser::load(&source).unwrap().into_graph(),
                    ExecutionLimits::default(),
                    Scheduler::seeded(seed),
                    Observer::all(),
                    profile,
                )
                .unwrap();
                r.enable_recording(Default::default()).unwrap();
                let report = r
                    .run(
                        RunLimits {
                            max_steps: 4,
                            ..Default::default()
                        },
                        None,
                        || false,
                    )
                    .unwrap();
                histories.push((
                    report.outcome,
                    report.rejected,
                    serde_json::to_value(r.recording().unwrap().events()).unwrap(),
                ));
            }
            assert_eq!(histories[0], histories[1]);
        }
    }
}

#[test]
fn immutable_join_preserves_reference_events_and_verified_replay() {
    for source in [
        SOURCE.to_string(),
        SOURCE.replace(
            "atom source =",
            "atom another = Record[ObjectLink](\"source-month\", session, june); atom source =",
        ),
        SOURCE.replace(
            "atom source =",
            "atom duplicate = Record[ObjectLink](\"normalizes-to\", june, six); atom source =",
        ),
    ] {
        for seed in [0, 42, 123] {
            let mut histories = Vec::new();
            for profile in [
                ExecutionProfile::ReferenceV1,
                ExecutionProfile::FramedV3,
                ExecutionProfile::JoinedV4,
            ] {
                let mut r = Runner::with_profile(
                    parser::load(&source).unwrap().into_graph(),
                    ExecutionLimits::default(),
                    Scheduler::seeded(seed),
                    Observer::all(),
                    profile,
                )
                .unwrap();
                r.enable_recording(Default::default()).unwrap();
                assert_eq!(
                    r.run(
                        RunLimits {
                            max_steps: 10,
                            detect_cycles: true,
                            ..Default::default()
                        },
                        None,
                        || false
                    )
                    .unwrap()
                    .outcome,
                    Outcome::NormalForm
                );
                let doc = TraceDocument::from_runner(
                    &r,
                    None,
                    TraceOptions::default(),
                    TraceLimits::default(),
                )
                .unwrap();
                histories.push(serde_json::to_value(&doc.events).unwrap());
                VerifiedTrace::verify(doc, TraceLimits::default()).unwrap();
            }
            assert_eq!(histories[0], histories[1]);
            assert_eq!(histories[0], histories[2]);
        }
    }
}
