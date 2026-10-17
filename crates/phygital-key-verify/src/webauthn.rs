//! The small part of WebAuthn a verifier needs: one ES256 assertion.
//! Parsing only. The signature math is done by the RustCrypto `p256` crate.

use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::b64;

/// Authenticator data flag: user present.
pub const FLAG_UP: u8 = 0x01;
/// Authenticator data flag: user verified.
pub const FLAG_UV: u8 = 0x04;

/// Largest client data JSON accepted, in bytes.
const MAX_CLIENT_DATA: usize = 4096;

/// Why an assertion failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssertionError {
    /// Bad encoding, wrong type, wrong challenge, origin or RP ID, or bad signature.
    BadSignature,
    /// Valid signature, but the flags do not prove enough.
    AssuranceTooLow,
}

/// Parse a base64url SPKI DER P-256 public key.
pub fn parse_public_key(spki_b64: &str) -> Option<VerifyingKey> {
    let der = b64::decode(spki_b64)?;
    VerifyingKey::from_public_key_der(&der).ok()
}

/// What an assertion must match.
pub struct AssertionCheck<'a> {
    /// `webauthn.get` for approvals, `webauthn.create` for enrollment.
    pub kind: &'a str,
    /// The exact challenge bytes.
    pub challenge: &'a [u8],
    /// Allowed origins.
    pub origins: &'a [String],
    /// Relying party id this key belongs to.
    pub rp_id: &'a str,
}

/// Check client data JSON: type, challenge, origin, not cross-origin.
pub fn check_client_data(client_data_json: &[u8], want: &AssertionCheck) -> bool {
    if client_data_json.len() > MAX_CLIENT_DATA {
        return false;
    }
    let Some(cd) = crate::strict::parse(client_data_json) else {
        return false;
    };
    let s = |k: &str| cd.get(k).and_then(Value::as_str);
    if s("type") != Some(want.kind) {
        return false;
    }
    let Some(got) = s("challenge").and_then(b64::decode) else {
        return false;
    };
    if !bool::from(got.as_slice().ct_eq(want.challenge)) {
        return false;
    }
    if !s("origin").is_some_and(|o| want.origins.iter().any(|a| a == o)) {
        return false;
    }
    // A page inside a frame on another site must not be able to collect a yes.
    !matches!(
        cd.get("crossOrigin"),
        Some(Value::Bool(true)) | Some(Value::String(_))
    ) && cd.get("topOrigin").is_none()
}

/// Verify one ES256 assertion. Returns the authenticator flags on success.
pub fn verify_assertion(
    public_key: &VerifyingKey,
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature_der: &[u8],
    want: &AssertionCheck,
) -> Result<u8, AssertionError> {
    if !check_client_data(client_data_json, want) {
        return Err(AssertionError::BadSignature);
    }
    let (Some(rp_hash), Some(&flags)) = (authenticator_data.get(..32), authenticator_data.get(32))
    else {
        return Err(AssertionError::BadSignature);
    };
    if authenticator_data.len() < 37 {
        return Err(AssertionError::BadSignature);
    }
    let want_rp = Sha256::digest(want.rp_id.as_bytes());
    if !bool::from(rp_hash.ct_eq(want_rp.as_slice())) {
        return Err(AssertionError::BadSignature);
    }
    let sig = Signature::from_der(signature_der).map_err(|_| AssertionError::BadSignature)?;
    let mut signed = authenticator_data.to_vec();
    signed.extend_from_slice(&Sha256::digest(client_data_json));
    public_key
        .verify(&signed, &sig)
        .map_err(|_| AssertionError::BadSignature)?;
    Ok(flags)
}
