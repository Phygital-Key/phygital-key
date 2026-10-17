//! Canonical form: every string in NFC, integers only, then RFC 8785 (JCS).

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::b64;

/// Domain separation prefix for the WebAuthn challenge.
pub const CHALLENGE_PREFIX: &[u8] = b"phygital-key/approval/v1\0";

/// Largest integer JavaScript can hold exactly (2^53 - 1).
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Why a value has no canonical form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonError {
    /// A number with a fraction, an exponent, or outside the safe integer range.
    NotSafeInteger,
    /// Two object keys became the same string after NFC normalization.
    DuplicateKeyAfterNfc,
    /// The JCS serializer failed.
    Serialize,
}

/// Returns a copy of `value` with every string and key in NFC.
/// Fails on numbers that are not safe integers and on keys that collide after NFC.
pub fn normalize(value: &Value) -> Result<Value, CanonError> {
    Ok(match value {
        Value::Null => Value::Null,
        Value::Bool(b) => Value::Bool(*b),
        Value::Number(n) => {
            let ok = match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&i),
                (None, Some(u)) => u <= MAX_SAFE_INTEGER as u64,
                _ => false,
            };
            if !ok {
                return Err(CanonError::NotSafeInteger);
            }
            Value::Number(n.clone())
        }
        Value::String(s) => Value::String(s.nfc().collect()),
        Value::Array(items) => Value::Array(items.iter().map(normalize).collect::<Result<_, _>>()?),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                let key: String = k.nfc().collect();
                if out.contains_key(&key) {
                    return Err(CanonError::DuplicateKeyAfterNfc);
                }
                out.insert(key, normalize(v)?);
            }
            Value::Object(out)
        }
    })
}

/// The canonical bytes of `value`: NFC, safe integers, RFC 8785.
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonError> {
    let normalized = normalize(value)?;
    serde_jcs::to_vec(&normalized).map_err(|_| CanonError::Serialize)
}

/// The canonical form as a string (always valid UTF-8).
pub fn canonical_string(value: &Value) -> Result<String, CanonError> {
    String::from_utf8(canonical_bytes(value)?).map_err(|_| CanonError::Serialize)
}

/// base64url SHA-256 of the canonical bytes. This is `request_hash`.
pub fn hash_b64(value: &Value) -> Result<String, CanonError> {
    Ok(b64::encode(Sha256::digest(canonical_bytes(value)?)))
}

/// The WebAuthn challenge for a set of claims:
/// SHA-256("phygital-key/approval/v1\0" + JCS(claims)).
pub fn challenge(claims: &Value) -> Result<[u8; 32], CanonError> {
    let mut h = Sha256::new();
    h.update(CHALLENGE_PREFIX);
    h.update(canonical_bytes(claims)?);
    Ok(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_are_sorted() {
        let v = json!({"b": 1, "a": [true, null, "x"]});
        assert_eq!(canonical_string(&v), Ok(r#"{"a":[true,null,"x"],"b":1}"#.to_string()));
    }

    #[test]
    fn floats_are_refused() {
        assert_eq!(canonical_bytes(&json!({"amount": 1.5})), Err(CanonError::NotSafeInteger));
        assert_eq!(canonical_bytes(&json!(9_007_199_254_740_992_u64)), Err(CanonError::NotSafeInteger));
        assert!(canonical_bytes(&json!(-9_007_199_254_740_991_i64)).is_ok());
    }

    #[test]
    fn nfd_and_nfc_give_the_same_bytes() {
        let nfc = json!({"name": "Jos\u{e9}"});
        let nfd = json!({"name": "Jose\u{301}"});
        assert_eq!(canonical_bytes(&nfc), canonical_bytes(&nfd));
    }

    #[test]
    fn keys_that_collide_after_nfc_are_refused() {
        let mut m = Map::new();
        m.insert("e\u{301}".into(), json!(1));
        m.insert("\u{e9}".into(), json!(2));
        assert_eq!(canonical_bytes(&Value::Object(m)), Err(CanonError::DuplicateKeyAfterNfc));
    }

    #[test]
    fn rfc8785_utf16_key_order() {
        // From RFC 8785 section 3.2.3: keys sort by UTF-16 code units.
        let v = json!({
            "\u{20ac}": "Euro Sign",
            "\r": "Carriage Return",
            "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
            "1": "One",
            "\u{1f600}": "Emoji: Grinning Face",
            "\u{80}": "Control",
            "\u{f6}": "Latin Small Letter O With Diaeresis"
        });
        // Raw JCS here: NFC would rewrite U+FB33 (it decomposes and never recomposes).
        let s = String::from_utf8(serde_jcs::to_vec(&v).unwrap_or_default()).unwrap_or_default();
        let order: Vec<&str> = [
            "Carriage Return",
            "One",
            "Control",
            "Latin Small Letter O With Diaeresis",
            "Euro Sign",
            "Emoji: Grinning Face",
            "Hebrew Letter Dalet With Dagesh",
        ]
        .to_vec();
        let mut last = 0;
        for name in order {
            let at = s.find(name).unwrap_or(0);
            assert!(at > last, "{name} out of order in {s}");
            last = at;
        }
    }

    #[test]
    fn nfc_rewrites_composition_exclusions() {
        // U+FB33 becomes U+05D3 U+05BC under NFC, so it sorts before U+20AC.
        assert_eq!(canonical_string(&json!("\u{fb33}")), Ok("\"\u{5d3}\u{5bc}\"".to_string()));
    }

    #[test]
    fn rfc8785_string_escapes() {
        let v = json!("\u{0}\u{8}\t\n\u{c}\r\u{1f}\"\\/\u{7f}é");
        assert_eq!(
            canonical_string(&v),
            Ok("\"\\u0000\\b\\t\\n\\f\\r\\u001f\\\"\\\\/\u{7f}é\"".to_string())
        );
    }
}
