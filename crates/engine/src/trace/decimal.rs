//! Canonical decimal strings, including on JSON object keys. Never JSON numbers.
use serde::{Deserialize, Deserializer, Serializer};
pub fn serialize<S: Serializer>(n: &u64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&n.to_string())
}
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let s = String::deserialize(d)?;
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(serde::de::Error::custom(
            "expected canonical unsigned decimal string",
        ));
    }
    s.parse().map_err(serde::de::Error::custom)
}
