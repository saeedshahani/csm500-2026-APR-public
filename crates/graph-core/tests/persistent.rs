use graph_core::{Graph, Kind, NewAtom, Target};
use std::collections::BTreeSet;

#[test]
fn candidates_share_immutable_payloads_but_not_mutations_or_allocators() {
    let mut original = Graph::new();
    let a = original
        .insert(Kind::Constructor("a".repeat(1000)), &[])
        .unwrap();
    let b = original.insert(Kind::Constructor("b".into()), &[]).unwrap();
    let mut candidate = original.clone();
    assert!(std::ptr::eq(
        original.get(a).unwrap(),
        candidate.get(a).unwrap()
    ));
    candidate.remove_set(&BTreeSet::from([b])).unwrap();
    let fresh = candidate
        .insert(Kind::Constructor("fresh".into()), &[])
        .unwrap();
    assert!(original.get(b).is_some());
    assert!(original.get(fresh).is_none());
    assert!(candidate.get(b).is_none());
    assert!(std::ptr::eq(
        original.get(a).unwrap(),
        candidate.get(a).unwrap()
    ));
    assert_ne!(original.next_identity(), candidate.next_identity());
    let snapshot = candidate.snapshot();
    let next = candidate.next_identity();
    assert!(candidate
        .insert_batch(&[NewAtom {
            kind: Kind::Nat,
            fields: vec![Target::Existing(b)]
        }])
        .is_err());
    assert_eq!(candidate.snapshot(), snapshot);
    assert_eq!(candidate.next_identity(), next);
    assert_eq!(original.next_identity(), b.number() + 1);
}
