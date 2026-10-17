//! Signed receipts: the service's answer ("done" or "failed") for one approval.
//!
//! A receipt is a standard compact JWS (ES256, `typ: pka-receipt+jwt`), so any
//! JWT library can read it. It is signed by the service's own key (a service is
//! not a person, so a normal key file is fine). It points at the approval by hash,
//! like AP2 receipts do:
//!
//! ```json
//! { "v": "pka.receipt.1", "iss": "<service aud>", "approval_id": "...",
//!   "reference": "<base64url SHA-256 of the approval string>",
//!   "request_hash": "...", "status": "success", "iat": 1760700005 }
//! ```
//! `status` is `success` or `error`; an error may carry `error` (a short code).

use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{b64, canonical, strict};

/// The `typ` of a receipt's JWS header.
pub const RECEIPT_TYP: &str = "pka-receipt+jwt";

/// What a receipt says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    /// Always `pka.receipt.1`.
    pub v: String,
    /// The service that acted (its audience id).
    pub iss: String,
    /// The approval's single-use id.
    pub approval_id: String,
    /// base64url SHA-256 of the exact approval string the service received.
    pub reference: String,
    /// The approved request's hash.
    pub request_hash: String,
    /// `success` or `error`.
    pub status: String,
    /// Short error code when `status` is `error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When the service answered, Unix seconds.
    pub iat: i64,
}

/// The `reference` for an approval string.
pub fn reference_of(approval: &str) -> String {
    b64::encode(Sha256::digest(approval.as_bytes()))
}

/// Why a receipt does not check out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptError {
    /// Not a compact JWS with the right header, or bad JSON.
    Malformed,
    /// The signature does not match the service's key.
    BadSignature,
    /// It is about another approval or another service.
    Mismatch,
}

impl std::fmt::Display for ReceiptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ReceiptError::Malformed => "MALFORMED",
            ReceiptError::BadSignature => "BAD_SIGNATURE",
            ReceiptError::Mismatch => "MISMATCH",
        })
    }
}

impl std::error::Error for ReceiptError {}

/// Sign a receipt with the service's key. Returns a compact JWS.
pub fn sign(receipt: &Receipt, key: &SigningKey) -> Result<String, ReceiptError> {
    let header = json!({"alg": "ES256", "typ": RECEIPT_TYP});
    let payload = serde_json::to_value(receipt).map_err(|_| ReceiptError::Malformed)?;
    let h = canonical::canonical_bytes(&header).map_err(|_| ReceiptError::Malformed)?;
    let p = canonical::canonical_bytes(&payload).map_err(|_| ReceiptError::Malformed)?;
    let input = format!("{}.{}", b64::encode(h), b64::encode(p));
    let sig: Signature = key.sign(input.as_bytes());
    Ok(format!("{input}.{}", b64::encode(sig.to_bytes())))
}

/// Check a receipt against the service's public key, and that it is about
/// `approval` from service `iss`.
pub fn verify(
    jws: &str,
    service_key: &VerifyingKey,
    iss: &str,
    approval: &str,
) -> Result<Receipt, ReceiptError> {
    use ReceiptError as R;
    if jws.len() > 4096 {
        return Err(R::Malformed);
    }
    let mut parts = jws.split('.');
    let (Some(h), Some(p), Some(s), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(R::Malformed);
    };
    let header = b64::decode(h)
        .and_then(|b| strict::parse(&b))
        .ok_or(R::Malformed)?;
    let want = json!({"alg": "ES256", "typ": RECEIPT_TYP});
    if header != want {
        return Err(R::Malformed);
    }
    let sig_bytes = b64::decode(s).ok_or(R::Malformed)?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| R::Malformed)?;
    service_key
        .verify(format!("{h}.{p}").as_bytes(), &sig)
        .map_err(|_| R::BadSignature)?;
    let payload: Value = b64::decode(p)
        .and_then(|b| strict::parse(&b))
        .ok_or(R::Malformed)?;
    let receipt: Receipt = serde_json::from_value(payload).map_err(|_| R::Malformed)?;
    if receipt.v != "pka.receipt.1" || !matches!(receipt.status.as_str(), "success" | "error") {
        return Err(R::Malformed);
    }
    let reference = reference_of(approval);
    if receipt.iss != iss || !bool::from(receipt.reference.as_bytes().ct_eq(reference.as_bytes())) {
        return Err(R::Mismatch);
    }
    Ok(receipt)
}

/// Read a receipt's payload WITHOUT checking its signature. Only for showing
/// what a service reported; use [`verify`] to trust it.
pub fn peek(jws: &str) -> Option<Receipt> {
    if jws.len() > 4096 {
        return None;
    }
    let p = jws.split('.').nth(1)?;
    let v = strict::parse(&b64::decode(p)?)?;
    serde_json::from_value(v).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> SigningKey {
        let bytes: [u8; 32] = Sha256::digest([n; 4]).into();
        SigningKey::from_slice(&bytes).unwrap_or_else(|_| {
            SigningKey::from_slice(&[1u8; 32]).unwrap_or_else(|_| unreachable!())
        })
    }

    fn sample(approval: &str) -> Receipt {
        Receipt {
            v: "pka.receipt.1".into(),
            iss: "https://deploy.example.com".into(),
            approval_id: "BwcHBwcHBwcHBwcHBwcHBw".into(),
            reference: reference_of(approval),
            request_hash: "x".into(),
            status: "success".into(),
            error: None,
            iat: 1_760_700_005,
        }
    }

    #[test]
    fn round_trip_and_refusals() -> Result<(), ReceiptError> {
        let (k, other) = (key(1), key(2));
        let jws = sign(&sample("APPROVAL"), &k)?;
        let r = verify(
            &jws,
            k.verifying_key(),
            "https://deploy.example.com",
            "APPROVAL",
        )?;
        assert_eq!(r.status, "success");
        assert_eq!(
            verify(
                &jws,
                other.verifying_key(),
                "https://deploy.example.com",
                "APPROVAL"
            ),
            Err(ReceiptError::BadSignature)
        );
        assert_eq!(
            verify(&jws, k.verifying_key(), "https://other.example", "APPROVAL"),
            Err(ReceiptError::Mismatch)
        );
        assert_eq!(
            verify(
                &jws,
                k.verifying_key(),
                "https://deploy.example.com",
                "OTHER"
            ),
            Err(ReceiptError::Mismatch)
        );
        let mut parts: Vec<String> = jws.split('.').map(String::from).collect();
        if let Some(p) = parts.get_mut(1) {
            *p = b64::encode(br#"{"v":"pka.receipt.1"}"#);
        }
        assert_eq!(
            verify(
                &parts.join("."),
                k.verifying_key(),
                "https://deploy.example.com",
                "APPROVAL"
            ),
            Err(ReceiptError::BadSignature)
        );
        let none_alg = format!(
            "{}.{}.",
            b64::encode(br#"{"alg":"none","typ":"pka-receipt+jwt"}"#),
            parts.get(1).cloned().unwrap_or_default()
        );
        assert_eq!(
            verify(
                &none_alg,
                k.verifying_key(),
                "https://deploy.example.com",
                "APPROVAL"
            ),
            Err(ReceiptError::Malformed)
        );
        Ok(())
    }
}
