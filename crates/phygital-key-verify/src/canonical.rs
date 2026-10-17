//! Canonical form: every string already in NFC, integers only, then RFC 8785 (JCS).
//!
//! Strings are never rewritten. NFC folds some single characters into others
//! (KELVIN SIGN U+212A becomes K, GREEK QUESTION MARK U+037E becomes `;`), so
//! normalizing would let two different byte strings share one approval.
//! Text that is not already NFC is refused instead.

use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::is_nfc;

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
    /// A string or key that is not in Unicode NFC.
    NotNfc,
    /// The JCS serializer failed.
    Serialize,
}

/// Check that `value` has a canonical form: every string and key in NFC,
/// every number a safe integer.
pub fn validate(value: &Value) -> Result<(), CanonError> {
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(n) => {
            // Every safe integer fits in i64; anything else (floats, huge u64) is refused.
            let ok = n
                .as_i64()
                .is_some_and(|i| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&i));
            if ok {
                Ok(())
            } else {
                Err(CanonError::NotSafeInteger)
            }
        }
        Value::String(s) => {
            if is_nfc(s) {
                Ok(())
            } else {
                Err(CanonError::NotNfc)
            }
        }
        Value::Array(items) => items.iter().try_for_each(validate),
        Value::Object(map) => map.iter().try_for_each(|(k, v)| {
            if !is_nfc(k) {
                return Err(CanonError::NotNfc);
            }
            validate(v)
        }),
    }
}

/// The canonical bytes of `value`: RFC 8785, after [`validate`].
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonError> {
    validate(value)?;
    serde_jcs::to_vec(value).map_err(|_| CanonError::Serialize)
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
        assert_eq!(
            canonical_string(&v),
            Ok(r#"{"a":[true,null,"x"],"b":1}"#.to_string())
        );
    }

    #[test]
    fn floats_are_refused() {
        assert_eq!(
            canonical_bytes(&json!({"amount": 1.5})),
            Err(CanonError::NotSafeInteger)
        );
        assert_eq!(
            canonical_bytes(&json!(9_007_199_254_740_992_u64)),
            Err(CanonError::NotSafeInteger)
        );
        assert!(canonical_bytes(&json!(-9_007_199_254_740_991_i64)).is_ok());
    }

    #[test]
    fn text_that_is_not_nfc_is_refused() {
        assert!(canonical_bytes(&json!({"name": "Jos\u{e9}"})).is_ok());
        assert_eq!(
            canonical_bytes(&json!({"name": "Jose\u{301}"})),
            Err(CanonError::NotNfc)
        );
        assert_eq!(
            canonical_bytes(&json!({"Jose\u{301}": 1})),
            Err(CanonError::NotNfc)
        );
    }

    #[test]
    fn nfc_singletons_are_refused_not_folded() {
        // KELVIN SIGN would fold into K; GREEK QUESTION MARK into ';'.
        assert_eq!(
            canonical_bytes(&json!({"to": "\u{212a}irk"})),
            Err(CanonError::NotNfc)
        );
        assert_eq!(canonical_bytes(&json!("a\u{37e}")), Err(CanonError::NotNfc));
        assert_ne!(
            canonical_bytes(&json!({"to": "Kirk"})),
            canonical_bytes(&json!({"to": "\u{212a}irk"}))
        );
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
    fn composition_exclusions_are_refused() {
        // U+FB33 is not NFC (it decomposes and never recomposes).
        assert_eq!(canonical_bytes(&json!("\u{fb33}")), Err(CanonError::NotNfc));
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
