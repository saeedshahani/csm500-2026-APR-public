//! Exact JSON equality without complete untyped trees. Objects ignore key order;
//! arrays retain order and multiplicity. Strict ingress validates both domains.
use serde_json::value::RawValue;
use std::collections::BTreeMap;

pub(super) fn equal(left: &[u8], right: &[u8]) -> serde_json::Result<bool> {
    compare(
        serde_json::from_slice(left)?,
        serde_json::from_slice(right)?,
    )
}
fn compare(left: &RawValue, right: &RawValue) -> serde_json::Result<bool> {
    let a = left.get();
    let b = right.get();
    if a == b {
        return Ok(true);
    }
    if a.as_bytes()[0] != b.as_bytes()[0] {
        return Ok(false);
    }
    match a.as_bytes()[0] {
        b'{' => {
            let a: BTreeMap<String, &RawValue> = serde_json::from_str(a)?;
            let b: BTreeMap<String, &RawValue> = serde_json::from_str(b)?;
            if a.len() != b.len() {
                return Ok(false);
            }
            for ((ak, av), (bk, bv)) in a.iter().zip(&b) {
                if ak != bk || !compare(av, bv)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        b'[' => {
            let a: Vec<&RawValue> = serde_json::from_str(a)?;
            let b: Vec<&RawValue> = serde_json::from_str(b)?;
            if a.len() != b.len() {
                return Ok(false);
            }
            for (av, bv) in a.iter().zip(&b) {
                if !compare(av, bv)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        b'"' => Ok(serde_json::from_str::<String>(a)? == serde_json::from_str::<String>(b)?),
        // Validated wire scalars are strings, null and booleans, never numbers.
        _ => Ok(false),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::Value;

    fn values() -> impl Strategy<Value = Value> {
        prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            proptest::collection::vec(any::<char>(), 0..12)
                .prop_map(|chars| Value::String(chars.into_iter().collect()))
        ]
        .prop_recursive(4, 64, 6, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                proptest::collection::btree_map("[a-z]{1,4}", inner, 0..6)
                    .prop_map(|map| Value::Object(map.into_iter().collect()))
            ]
        })
    }

    proptest! {
        #[test]
        fn borrowed_comparison_agrees_with_value_oracle(a in values(), b in values()) {
            let compact = serde_json::to_vec(&a).unwrap();
            let pretty = serde_json::to_vec_pretty(&a).unwrap();
            prop_assert!(equal(&compact, &pretty).unwrap());
            prop_assert_eq!(equal(&compact, &serde_json::to_vec(&b).unwrap()).unwrap(), a == b);
        }
    }
    #[test]
    fn object_order_and_escapes_are_equal_but_arrays_are_exact() {
        assert!(equal(
            br#"{"b":["\u0061",true],"a":null}"#,
            br#"{ "a": null, "b": ["a", true] }"#
        )
        .unwrap());
        for (a, b) in [
            (br#"["1","2"]"#.as_slice(), br#"["2","1"]"#.as_slice()),
            (br#"["1","1"]"#, br#"["1"]"#),
            (br#"{"a":null}"#, br#"{}"#),
            (b"true", b"false"),
        ] {
            assert!(!equal(a, b).unwrap());
        }
    }
}
