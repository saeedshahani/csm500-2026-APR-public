//! Bounded parsing before typed allocation, including duplicate object keys.
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
#[cfg(test)]
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// Validate a serialized document without retaining another JSON tree. Container
/// lengths are charged by field name, exactly as in the value-based ingress check.
pub(super) fn validate(bytes: &[u8], limits: super::TraceLimits) -> super::Result<()> {
    struct Check<'a> {
        nodes: &'a mut usize,
        counts: &'a mut [usize; 7],
        depth: usize,
        field: Option<usize>,
        root: bool,
    }
    impl Check<'_> {
        fn item<E: de::Error>(&mut self) -> Result<(), E> {
            if let Some(i) = self.field {
                self.counts[i] = self.counts[i]
                    .checked_sub(1)
                    .ok_or_else(|| de::Error::custom("aggregate trace container limit"))?;
            }
            Ok(())
        }
    }
    impl<'de> DeserializeSeed<'de> for Check<'_> {
        type Value = ();
        fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
            *self.nodes = self
                .nodes
                .checked_sub(1)
                .ok_or_else(|| de::Error::custom("JSON node limit"))?;
            if self.depth == 0 {
                return Err(de::Error::custom("JSON depth limit"));
            }
            d.deserialize_any(self)
        }
    }
    impl<'de> Visitor<'de> for Check<'_> {
        type Value = ();
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("bounded trace JSON; integers must be decimal strings")
        }
        fn visit_unit<E: de::Error>(self) -> Result<(), E> {
            Ok(())
        }
        fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
            Ok(())
        }
        fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
            Ok(())
        }
        fn visit_string<E: de::Error>(self, _: String) -> Result<(), E> {
            Ok(())
        }
        fn visit_seq<A: SeqAccess<'de>>(mut self, mut a: A) -> Result<(), A::Error> {
            while a
                .next_element_seed(Check {
                    nodes: self.nodes,
                    counts: self.counts,
                    depth: self.depth - 1,
                    field: None,
                    root: false,
                })?
                .is_some()
            {
                self.item()?;
            }
            Ok(())
        }
        fn visit_map<A: MapAccess<'de>>(mut self, mut a: A) -> Result<(), A::Error> {
            let mut keys = BTreeSet::new();
            while let Some(key) = a.next_key::<String>()? {
                *self.nodes = self
                    .nodes
                    .checked_sub(1)
                    .ok_or_else(|| de::Error::custom("JSON node limit"))?;
                self.item()?;
                let field = match key.as_str() {
                    "atoms" | "fresh" => Some(0),
                    "fields" => Some(1),
                    "binding" => Some(2),
                    "witnesses" => Some(3),
                    "pairs" => Some(4),
                    "events" if self.root => Some(5),
                    "checkpoints" if self.root => Some(6),
                    _ => None,
                };
                if !keys.insert(key) {
                    return Err(de::Error::custom("duplicate JSON key"));
                }
                a.next_value_seed(Check {
                    nodes: self.nodes,
                    counts: self.counts,
                    depth: self.depth - 1,
                    field,
                    root: false,
                })?;
            }
            Ok(())
        }
    }
    let mut nodes = limits.max_nodes;
    let mut counts = [
        limits.max_atoms_total,
        limits.max_fields_total,
        limits.max_bindings,
        limits.max_witnesses,
        limits.max_conflicts,
        limits.max_events,
        limits.max_checkpoints,
    ];
    let mut de = serde_json::Deserializer::from_slice(bytes);
    Check {
        nodes: &mut nodes,
        counts: &mut counts,
        depth: limits.max_depth.min(64),
        field: None,
        root: true,
    }
    .deserialize(&mut de)
    .map_err(|e| {
        if e.to_string().starts_with("aggregate trace container limit") {
            super::exhausted("aggregate trace container limit")
        } else {
            super::bad(&e.to_string())
        }
    })?;
    de.end().map_err(|e| super::bad(&e.to_string()))
}

#[cfg(test)]
struct Seed<'a> {
    remaining: &'a mut usize,
    depth: usize,
}
#[cfg(test)]
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        *self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| de::Error::custom("JSON node limit"))?;
        if self.depth == 0 {
            return Err(de::Error::custom("JSON depth limit"));
        }
        d.deserialize_any(self)
    }
}
#[cfg(test)]
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded trace JSON; integers must be decimal strings")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut result = Vec::new();
        while let Some(v) = a.next_element_seed(Seed {
            remaining: self.remaining,
            depth: self.depth - 1,
        })? {
            result.push(v);
        }
        Ok(Value::Array(result))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut result = serde_json::Map::new();
        while let Some(k) = a.next_key::<String>()? {
            *self.remaining = self
                .remaining
                .checked_sub(1)
                .ok_or_else(|| de::Error::custom("JSON node limit"))?;
            if result.contains_key(&k) {
                return Err(de::Error::custom(format!("duplicate key {k}")));
            }
            let v = a.next_value_seed(Seed {
                remaining: self.remaining,
                depth: self.depth - 1,
            })?;
            result.insert(k, v);
        }
        Ok(Value::Object(result))
    }
}
#[cfg(test)]
pub(super) fn parse(bytes: &[u8], nodes: usize, depth: usize) -> super::Result<Value> {
    let mut remaining = nodes;
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let value = Seed {
        remaining: &mut remaining,
        depth: depth.min(64),
    }
    .deserialize(&mut de)
    .map_err(|e| super::bad(&e.to_string()))?;
    de.end().map_err(|e| super::bad(&e.to_string()))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::super::{structural_limits, TraceLimits};
    use super::*;
    use proptest::prelude::*;

    fn reference(bytes: &[u8], limits: TraceLimits) -> super::super::Result<()> {
        structural_limits(&parse(bytes, limits.max_nodes, limits.max_depth)?, limits)
    }

    #[test]
    fn streaming_budget_preserves_rejections_and_resource_category() {
        for bytes in [
            b"{\"a\":null,\"a\":true}".as_slice(),
            b"null false",
            b"[1]",
            b"{",
            b"[false",
        ] {
            assert!(validate(bytes, TraceLimits::default()).is_err());
            assert!(reference(bytes, TraceLimits::default()).is_err());
        }
        let limits = TraceLimits {
            max_atoms_total: 1,
            ..Default::default()
        };
        let bytes = br#"{"atoms":{"0":null},"fresh":[null]}"#;
        assert_eq!(
            validate(bytes, limits).unwrap_err().kind(),
            graph_core::ErrorKind::ResourceExhaustion
        );
        assert_eq!(
            reference(bytes, limits).unwrap_err().kind(),
            graph_core::ErrorKind::ResourceExhaustion
        );
    }

    proptest! {
        #[test]
        fn streaming_budget_agrees_with_tree_oracle(n in 0usize..12, cap in 0usize..80,
            nodes in 0usize..300, depth in 0usize..8) {
            let array = vec![serde_json::json!({"label":"λ 🦀","fields":["0","1"]}); n];
            let value = serde_json::json!({"atoms":array,"fresh":{"a":null},
                "nested":{"binding":array,"witnesses":array,"pairs":array},"other":[null,true,false]});
            let bytes = serde_json::to_vec(&value).unwrap();
            let limits = TraceLimits { max_nodes:nodes, max_depth:depth, max_atoms_total:cap,
                max_fields_total:cap, max_bindings:cap, max_witnesses:cap, max_conflicts:cap,
                ..Default::default() };
            prop_assert_eq!(validate(&bytes,limits).is_ok(), reference(&bytes,limits).is_ok());
        }
    }
}
