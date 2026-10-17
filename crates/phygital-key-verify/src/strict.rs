//! Strict JSON parsing: refuses duplicate keys and deep nesting.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

/// Deepest nesting allowed in an approval object.
pub const MAX_DEPTH: usize = 16;

struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = StrictValue;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON")
    }
    fn visit_bool<E>(self, v: bool) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::Bool(v)))
    }
    fn visit_i64<E>(self, v: i64) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::from(v)))
    }
    fn visit_u64<E>(self, v: u64) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::from(v)))
    }
    fn visit_f64<E>(self, v: f64) -> Result<StrictValue, E> {
        // Kept so the canonical step can refuse it with a clear reason.
        Ok(StrictValue(serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)))
    }
    fn visit_str<E>(self, v: &str) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::String(v.to_owned())))
    }
    fn visit_string<E>(self, v: String) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::String(v)))
    }
    fn visit_unit<E>(self) -> Result<StrictValue, E> {
        Ok(StrictValue(Value::Null))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<StrictValue, A::Error> {
        let mut items = Vec::new();
        while let Some(StrictValue(v)) = seq.next_element()? {
            items.push(v);
        }
        Ok(StrictValue(Value::Array(items)))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<StrictValue, A::Error> {
        let mut out = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if out.contains_key(&key) {
                return Err(de::Error::custom("duplicate key"));
            }
            let StrictValue(v) = map.next_value()?;
            out.insert(key, v);
        }
        Ok(StrictValue(Value::Object(out)))
    }
}

/// Nesting depth of a JSON text, counted without parsing.
fn depth(bytes: &[u8]) -> usize {
    let (mut depth, mut max, mut in_str, mut esc) = (0usize, 0usize, false, false);
    for &b in bytes {
        if in_str {
            match (esc, b) {
                (true, _) => esc = false,
                (false, b'\\') => esc = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' | b'[' => {
                depth += 1;
                max = max.max(depth);
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

/// Parse JSON, refusing duplicate keys, lone surrogates, trailing data
/// and nesting deeper than [`MAX_DEPTH`].
pub fn parse(bytes: &[u8]) -> Option<Value> {
    if depth(bytes) > MAX_DEPTH {
        return None;
    }
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let StrictValue(v) = StrictValue::deserialize(&mut de).ok()?;
    de.end().ok()?;
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_duplicates_and_junk() {
        assert!(parse(br#"{"a":1,"a":2}"#).is_none());
        assert!(parse(br#"{"a":{"b":1,"b":1}}"#).is_none());
        assert!(parse(br#"{"a":1} x"#).is_none());
        assert!(parse(br#""\ud800""#).is_none());
        assert!(parse(br#"{"a":1}"#).is_some());
    }

    #[test]
    fn refuses_deep_nesting() {
        let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse(deep.as_bytes()).is_none());
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(ok.as_bytes()).is_some());
    }

    #[test]
    fn brackets_in_strings_do_not_count() {
        let s = format!(r#"{{"a":"{}"}}"#, "[".repeat(100));
        assert!(parse(s.as_bytes()).is_some());
    }
}
