use engine::numerical::*;
use serde_json::{json, Value};
fn rat(s: &str) -> Rational {
    Rational::parse(s).unwrap()
}
fn request() -> Request {
    serde_json::from_str(include_str!("../../../examples/numerical/workflow.json")).unwrap()
}
fn fresh() -> Component {
    let r = request();
    Component::new(r.identity, r.task, r.boundary, 0).unwrap()
}
fn packet(c: &Component, r: &Receipt, now: u64) -> Packet {
    c.assess(r, rat("1/1000"), now, 32).unwrap()
}
fn check(v: &Value) -> bool {
    check_packet_json(&serde_json::to_vec(v).unwrap()).is_ok()
}

#[test]
fn methods_and_native_transport() {
    for mode in [Mode::Raw, Mode::Compiled, Mode::Direct] {
        let mut r = request();
        r.mode = mode;
        let report = execute(&r).unwrap();
        let p = &report["publications"][0];
        assert_eq!(p["packet"]["status"], "certified");
        assert_eq!(p["atoms"], 19);
        assert_eq!(p["events"], 1);
        assert!(check(&p["packet"]));
        engine::trace::VerifiedTrace::import(
            &serde_json::to_vec(&p["trace"]).unwrap(),
            Default::default(),
        )
        .unwrap();
    }
}
#[test]
fn residual_bounds_arbitrary_values_and_compiler_rejects_bad_envelopes() {
    let task: Task =
        serde_json::from_value(json!([[{"weights":["3/4"],"reward":"1/8","boundary":"1/8"}]]))
            .unwrap();
    for (value, expected) in [("0", "5/8"), ("1/2", "1/8"), ("5/8", "0"), ("1", "3/8")] {
        assert_eq!(
            residual_radius(&task, &rat("1/4"), &[rat(value)])
                .unwrap()
                .0,
            rat(expected)
        );
    }
    let program = synthesize(&task).unwrap();
    assert!(certify(&task, &program).unwrap());
    assert!(!certify(&task, &vec![vec![(rat("0"), rat("0"))]]).unwrap());
    assert!(!certify(&task, &vec![vec![(rat("1"), rat("0"))]]).unwrap());
    assert!(!certify(&task, &vec![]).unwrap());
}
#[test]
fn malformed_schema_never_coerces_strings_or_ignores_keys() {
    let mut c = fresh();
    let r = c.compute(Mode::Direct, 128, 24).unwrap();
    let p = serde_json::to_value(packet(&c, &r, 0)).unwrap();
    for bad in [json!("false"), json!(0), json!(null), json!([])] {
        let mut v = p.clone();
        v["current"]["active"] = bad;
        assert!(!check(&v));
    }
    for section in ["", "current", "receipt"] {
        let mut v = p.clone();
        if section.is_empty() {
            v["unknown"] = json!(1);
        } else {
            v[section]["unknown"] = json!(1);
        }
        assert!(!check(&v));
    }
    for (field, value) in [
        ("checked_at", json!(-1)),
        ("tolerance", json!(0.1)),
        ("max_age", json!(true)),
        ("schema", json!("t2-publication-v1")),
    ] {
        let mut v = p.clone();
        v[field] = value;
        assert!(!check(&v));
    }
    let mut v = p.clone();
    v["current"]["fault"] = json!("invented");
    assert!(!check(&v));
    let mut v = p;
    v["radius_now"] = json!("0");
    assert!(!check(&v));
}
#[test]
fn invalid_queries_do_not_change_time_or_invalidate_other_receipts() {
    let mut c = fresh();
    let r = c.compute(Mode::Direct, 128, 24).unwrap();
    let mut other = fresh();
    other.observe(rat("3/4"), 1, 0, None).unwrap();
    let unknown = other.compute(Mode::Direct, 128, 24).unwrap();
    assert!(c.assess(&unknown, rat("1/1000"), 33, 32).is_err());
    assert_eq!(packet(&c, &r, 0).status(), Status::Certified);
    assert_eq!(packet(&c, &r, 33).status(), Status::Stale);
    // Assessments are immutable logical queries, including successful queries.
    assert_eq!(packet(&c, &r, 0).status(), Status::Certified);
    c.observe(rat("3/8"), 1, 1, None).unwrap();
    assert!(c.assess(&r, rat("1/1000"), 0, 32).is_err());
    assert_eq!(packet(&c, &r, 1).status(), Status::Certified);
}
#[test]
fn admitted_bad_observations_latch_and_revision_recovers() {
    for (p, seq, tick, d) in [
        ("3/4", 1, 1, Some("1/8")),
        ("2", 1, 1, None),
        ("0", 2, 1, None),
        ("0", 1, 1_000_000_001, None),
    ] {
        let mut c = fresh();
        let r = c.compute(Mode::Direct, 128, 24).unwrap();
        assert!(c.observe(rat(p), seq, tick, d.map(rat)).is_err());
        assert_eq!(packet(&c, &r, 1).status(), Status::Stale);
        assert!(c.compute(Mode::Raw, 1, 24).is_err());
        c.revise(request().task, rat("3/8"), 1).unwrap();
        assert_eq!(packet(&c, &r, 1).status(), Status::Stale);
        let new = c.compute(Mode::Direct, 128, 24).unwrap();
        assert_eq!(packet(&c, &new, 1).status(), Status::Certified);
    }
    let mut c = fresh();
    c.observe(rat("3/8"), 1, 2, None).unwrap();
    assert!(c.observe(rat("3/8"), 2, 1, None).is_err());
}
#[test]
fn malformed_observation_syntax_never_enters_the_typed_api() {
    let mut c = fresh();
    let r = c.compute(Mode::Direct, 128, 24).unwrap();
    for v in [
        json!({"operation":"observe","boundary":"bad","sequence":1,"tick":1}),
        json!({"operation":"observe","boundary":"0","sequence":-1,"tick":1}),
    ] {
        assert!(serde_json::from_value::<Operation>(v).is_err());
        assert_eq!(packet(&c, &r, 0).status(), Status::Certified);
    }
}
#[test]
fn departure_revision_and_incarnation_fence_old_receipts() {
    let mut c = fresh();
    let r = c.compute(Mode::Direct, 128, 24).unwrap();
    c.depart();
    assert_eq!(packet(&c, &r, 1).status(), Status::Stale);
    assert!(c.compute(Mode::Direct, 128, 24).is_err());
    c.rejoin(rat("3/8"), 2).unwrap();
    assert_eq!(packet(&c, &r, 2).status(), Status::Stale);
    let new = c.compute(Mode::Direct, 128, 24).unwrap();
    assert_eq!(packet(&c, &new, 2).status(), Status::Certified);
    assert!(c.revise(vec![], rat("0"), 3).is_err());
    assert_eq!(packet(&c, &new, 2).status(), Status::Certified);
    let mut task = request().task;
    task[0][0].reward = rat("1/16");
    c.revise(task, rat("1/4"), 3).unwrap();
    assert_eq!(packet(&c, &new, 3).status(), Status::Stale);
    assert!(fresh().assess(&new, rat("1/10"), 3, 32).is_err());
}
#[test]
fn held_receipt_movement_and_precision_limits() {
    let mut c = fresh();
    let r = c.compute(Mode::Direct, 128, 24).unwrap();
    c.observe(rat("3/4"), 1, 1, None).unwrap();
    let p = packet(&c, &r, 1);
    assert_eq!(p.status(), Status::Uncertified);
    verify_packet(&p).unwrap();
    let low = c.compute(Mode::Compiled, 128, 4).unwrap();
    assert_eq!(
        c.assess(&low, rat("1/1000000"), 1, 32).unwrap().status(),
        Status::Uncertified
    );
    let zero = c.compute(Mode::Raw, 0, 24).unwrap();
    assert_eq!(packet(&c, &zero, 1).status(), Status::Uncertified);
}
#[test]
fn high_retry_direct_and_compiled_work_when_iteration_does_not() {
    let d = "1267650600228229401496703205376";
    let task:Task=serde_json::from_value(json!([[{"weights":[format!("1267650600228229401496703205375/{d}")],"reward":"1/2535301200456458802993406410752","boundary":"0"}]])).unwrap();
    for mode in [Mode::Raw, Mode::Compiled, Mode::Direct] {
        let y = solve(&task, &rat("0"), mode, 4096, 64).unwrap();
        assert_eq!(y[0], rat(if mode == Mode::Raw { "0" } else { "1/2" }));
        assert_eq!(
            residual_radius(&task, &rat("0"), &y).unwrap().0,
            rat(if mode == Mode::Raw { "1/2" } else { "0" })
        );
    }
}
#[test]
fn caps_are_enforced_and_session_operations_publish_each_status() {
    let mut c = fresh();
    assert!(c.compute(Mode::Raw, 4097, 24).is_err());
    assert!(c.compute(Mode::Raw, 1, 65).is_err());
    for n in 0..64 {
        c.observe(rat(&format!("{n}/128")), n + 1, n, None).unwrap();
        c.compute(Mode::Raw, 0, 24).unwrap();
    }
    assert!(c.compute(Mode::Raw, 0, 24).is_err());
    let mut r = serde_json::to_value(request()).unwrap();
    r["operations"] = json!([
        {"operation":"observe","boundary":"3/4","sequence":1,"tick":1},
        {"operation":"assess","receipt":0,"tolerance":"1/1000","now":1,"max_age":32},
        {"operation":"depart"},
        {"operation":"assess","receipt":0,"tolerance":"1/1000","now":1,"max_age":32}]);
    let result = execute(&serde_json::from_value(r).unwrap()).unwrap();
    let statuses: Vec<_> = result["publications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["packet"]["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, vec!["certified", "uncertified", "stale"]);
}

#[test]
fn bounded_exact_inputs_and_task_domains_reject_without_panics() {
    for text in ["", "true", "1/0", "1/-2", "1/2/3", "0.5", "NaN", " 1"] {
        assert!(Rational::parse(text).is_err(), "{text}");
    }
    assert!(Rational::parse(&"9".repeat(5001)).is_err());
    let mut r = request();
    r.boundary = rat("1/680564733841876926926749214863536422912");
    assert!(execute(&r).is_err());
    let mut r = request();
    r.task[0][0].weights[0] = rat("1");
    assert!(execute(&r).is_err());
    let mut r = request();
    r.task[0][0].weights.pop();
    assert!(execute(&r).is_err());
    let mut r = request();
    r.task = vec![r.task[0].clone(); 5];
    assert!(execute(&r).is_err());
}

#[test]
fn tampered_receipt_bounds_reject_even_with_recomputed_content_hash() {
    use sha2::{Digest, Sha256};
    let mut c = fresh();
    let r = c.compute(Mode::Direct, 128, 24).unwrap();
    let p = serde_json::to_value(packet(&c, &r, 0)).unwrap();
    for field in ["radius", "residual", "q"] {
        let mut v = p.clone();
        v["receipt"][field] = json!("1");
        v["receipt_digest"] = json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&v["receipt"]).unwrap())
        ));
        assert!(!check(&v), "{field}");
    }
    let mut v = p.clone();
    v["receipt"]["task"][0][0]["extra"] = json!(true);
    assert!(!check(&v));
    let mut v = p;
    v["observed_variation"] = json!("-1");
    assert!(!check(&v));
}
