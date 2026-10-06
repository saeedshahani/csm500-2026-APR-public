//! Finite independent truth-table checks of the written path-evidence argument.
//! This is a test oracle, not a production membership-query service.
#[test]
fn all_three_word_three_query_certificates_and_regular_adversaries() {
    let mut configurations = 0;
    let answer = |lang: u8, queries: [u8; 3]| queries.map(|p| p & lang != 0);
    for h in 0u8..8 {
        for p in 0u8..8 {
            for q in 0u8..8 {
                for r in 0u8..8 {
                    let queries = [p, q, r];
                    let answers = answer(h, queries);
                    for evidence in 0u8..8 {
                        let brute = (0u8..8)
                            .filter(|u| (u ^ h) & evidence == 0)
                            .all(|u| answer(u, queries) == answers);
                        let condition = queries.iter().zip(answers).all(|(path, yes)| {
                            if yes {
                                path & h & evidence != 0
                            } else {
                                path & !evidence == 0
                            }
                        });
                        assert_eq!(brute, condition);
                    }
                    for u in 0u8..8 {
                        let desired = answer(u, queries);
                        let mut retained = h;
                        let mut witnesses = 0;
                        for (path, yes) in queries.into_iter().zip(desired) {
                            if yes {
                                let candidates = path & u;
                                witnesses |= 1 << candidates.trailing_zeros();
                            } else {
                                retained &= !path;
                            }
                        }
                        let v = retained | witnesses;
                        assert_eq!(answer(v, queries), desired);
                        assert_eq!((h ^ v) & !(h ^ u), 0);
                        configurations += 1;
                    }
                }
            }
        }
    }
    assert_eq!(configurations, 32768);
    println!("OBSERVER_EVIDENCE configurations={configurations}");
}
