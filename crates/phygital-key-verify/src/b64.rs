//! base64url without padding, strict on decode.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Encode bytes as base64url without padding.
pub fn encode(bytes: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Decode base64url without padding. Refuses padding, other alphabets and
/// non-canonical trailing bits.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(text).ok()
}
