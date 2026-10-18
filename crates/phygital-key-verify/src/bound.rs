//! Agent-bound approvals (`pka.2`) and the agent's proof.
//!
//! A `pka.1` approval works once for whoever holds it. A `pka.2` approval also
//! names, in `claims.cnf.key`, the public key of the agent that asked, so the
//! person's passkey signs that key along with the request. To use it, the agent
//! sends a **proof** next to the approval: a compact JWS (ES256, raw r||s)
//! signed with its own key, naming this approval, this service and this request:
//!
//! ```text
//! header  {"alg":"ES256","typ":"pka-proof+jwt"}        (exactly these bytes)
//! payload {"approval_id": "<claims.id>", "aud": "<claims.aud>",
//!          "iat": <now>, "request_hash": "<claims.request_hash>",
//!          "v": "pka.proof.1"}                           (canonical form)
//! ```
//!
//! Someone who copies the approval on its way (from a log, a transcript, the
//! agent's tool output) does not have the agent's private key, so the service
//! refuses it. The proof is made when the agent acts, never earlier.

use p256::ecdsa::signature::{Signer, Verifier as _};
use p256::ecdsa::{Signature, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::model::MAX_CLOCK_SKEW_SECS;
use crate::verify::{ErrorCode, Verified};
use crate::{b64, canonical, strict, webauthn};

/// The JWS `typ` of a proof.
pub const PROOF_TYP: &str = "pka-proof+jwt";

/// The proof format.
pub const PROOF_VERSION: &str = "pka.proof.1";

/// Largest proof accepted, in bytes.
pub const MAX_PROOF_BYTES: usize = 4096;

/// What the agent signs when it uses a bound approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    /// Always [`PROOF_VERSION`].
    pub v: String,
    /// The approval's single-use id (`claims.id`).
    pub approval_id: String,
    /// The service it is for (`claims.aud`).
    pub aud: String,
    /// The approved request's hash (`claims.request_hash`).
    pub request_hash: String,
    /// When the agent made the proof, Unix seconds.
    pub iat: i64,
}

impl Proof {
    /// A proof for this approval, made now.
    pub fn for_approval(approval_id: &str, aud: &str, request_hash: &str, now: i64) -> Proof {
        Proof {
            v: PROOF_VERSION.into(),
            approval_id: approval_id.into(),
            aud: aud.into(),
            request_hash: request_hash.into(),
            iat: now,
        }
    }
}

/// How a service wants binding handled.
#[derive(Debug, Clone, Copy, Default)]
pub struct Binding<'a> {
    /// The agent's proof, when it sent one (the `Phygital-Proof` header).
    pub proof: Option<&'a str>,
    /// Refuse approvals that are not bound to an agent key (`pka.1`). This is
    /// for one-time approvals; standing approvals (`verify_use`) are bound to the
    /// gate's use key instead and are not affected.
    pub required: bool,
}

fn header_bytes() -> Result<Vec<u8>, ErrorCode> {
    canonical::canonical_bytes(&json!({"alg": "ES256", "typ": PROOF_TYP}))
        .map_err(|_| ErrorCode::Malformed)
}

/// Sign a proof with the agent's key (for agents and tests).
pub fn sign_proof(proof: &Proof, key: &SigningKey) -> Result<String, ErrorCode> {
    let payload = serde_json::to_value(proof).map_err(|_| ErrorCode::Malformed)?;
    let p = canonical::canonical_bytes(&payload).map_err(|_| ErrorCode::Malformed)?;
    let input = format!("{}.{}", b64::encode(header_bytes()?), b64::encode(p));
    let sig: Signature = key.sign(input.as_bytes());
    Ok(format!("{input}.{}", b64::encode(sig.to_bytes())))
}

/// Check the agent's proof for a checked `pka.2` approval.
pub(crate) fn check_proof(jws: &str, ok: &Verified, now: i64) -> Result<(), ErrorCode> {
    use ErrorCode as E;
    let Some(agent_key) = ok.agent_key.as_deref() else {
        return Err(E::BadProof);
    };
    if jws.trim().is_empty() {
        return Err(E::ProofRequired);
    }
    if jws.len() > MAX_PROOF_BYTES {
        return Err(E::BadProof);
    }
    let mut parts = jws.split('.');
    let (Some(h), Some(p), Some(s), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(E::BadProof);
    };
    if b64::decode(h).as_deref() != Some(header_bytes()?.as_slice()) {
        return Err(E::BadProof);
    }
    let sig = b64::decode(s)
        .and_then(|b| Signature::from_slice(&b).ok())
        .ok_or(E::BadProof)?;
    let key = webauthn::parse_public_key(agent_key).ok_or(E::BadProof)?;
    key.verify(format!("{h}.{p}").as_bytes(), &sig)
        .map_err(|_| E::BadProof)?;
    let payload = b64::decode(p)
        .and_then(|b| strict::parse(&b))
        .ok_or(E::BadProof)?;
    let proof: Proof = serde_json::from_value(payload).map_err(|_| E::BadProof)?;
    let fresh = proof.iat >= now.saturating_sub(MAX_CLOCK_SKEW_SECS)
        && proof.iat <= now.saturating_add(MAX_CLOCK_SKEW_SECS)
        && proof.iat >= ok.iat.saturating_sub(MAX_CLOCK_SKEW_SECS)
        && proof.iat < ok.exp;
    if proof.v != PROOF_VERSION
        || proof.approval_id != ok.id
        || proof.aud != ok.aud
        || proof.request_hash != ok.request_hash
        || !fresh
    {
        return Err(E::BadProof);
    }
    Ok(())
}
