//! Bounded parsing before typed allocation, including duplicate object keys.
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;

struct Seed<'a> {
    remaining: &'a mut usize,
    depth: usize,
}
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
