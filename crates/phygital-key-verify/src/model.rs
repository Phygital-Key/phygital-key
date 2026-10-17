//! The data shapes: levels, claims, approval object, key cards, config.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Version string carried in every approval.
pub const VERSION: &str = "pka.1";

/// Largest encoded approval accepted (base64url text), in bytes.
pub const MAX_APPROVAL_BYTES: usize = 16 * 1024;

/// Largest canonical request accepted, in bytes.
pub const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// Longest approval life: `exp - iat`.
pub const MAX_LIFETIME_SECS: i64 = 300;

/// Default approval life.
pub const DEFAULT_LIFETIME_SECS: i64 = 60;

/// How far `iat` may sit in the future (clock drift).
pub const MAX_CLOCK_SKEW_SECS: i64 = 30;

/// How strongly the person proved it was them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// A touch on the authenticator (user presence).
    Medium,
    /// Fingerprint, face or PIN (user verification).
    High,
}

impl Level {
    /// The lowercase name used on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Medium => "medium",
            Level::High => "high",
        }
    }

    /// Parse `"medium"` or `"high"`.
    pub fn parse(s: &str) -> Option<Level> {
        match s {
            "medium" => Some(Level::Medium),
            "high" => Some(Level::High),
            _ => None,
        }
    }
}

/// The signed part of an approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    /// Always [`VERSION`].
    pub v: String,
    /// 128-bit random id, base64url (22 characters). Single use.
    pub id: String,
    /// Who will act on the yes.
    pub aud: String,
    /// The exact request: `{"type": ..., "params": {...}}`.
    pub request: Value,
    /// base64url SHA-256 of the canonical request.
    pub request_hash: String,
    /// Required assurance.
    pub level: Level,
    /// Issued at, Unix seconds.
    pub iat: i64,
    /// Expires at, Unix seconds.
    pub exp: i64,
}

/// One WebAuthn assertion, every field base64url.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assertion {
    /// Credential id of the key that signed.
    pub credential_id: String,
    /// Raw authenticator data.
    pub authenticator_data: String,
    /// Raw client data JSON.
    pub client_data_json: String,
    /// DER ECDSA P-256 signature.
    pub signature: String,
}

/// The approval object as it travels (before base64url).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalObject {
    /// Signed claims.
    pub claims: Value,
    /// Exactly one assertion in v0.1.
    pub signatures: Vec<Assertion>,
}

/// A public key card: what a verifier needs to trust one approver key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyCard {
    /// Credential id, base64url.
    pub credential_id: String,
    /// SubjectPublicKeyInfo DER of the P-256 key, base64url.
    pub public_key: String,
    /// WebAuthn relying party id the key belongs to.
    pub rp_id: String,
    /// Human name, for display only.
    pub name: String,
    /// When it was enrolled (ISO 8601 date), for display only.
    pub created: String,
}

/// The keys a verifier trusts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyring {
    /// Trusted key cards.
    pub keys: Vec<KeyCard>,
}

impl Keyring {
    /// Parse a keyring JSON document (`{"keys": [...]}`).
    pub fn from_json(text: &str) -> Result<Keyring, String> {
        let ring: Keyring = serde_json::from_str(text).map_err(|e| format!("keyring: {e}"))?;
        for k in &ring.keys {
            crate::webauthn::parse_public_key(&k.public_key)
                .ok_or_else(|| format!("keyring: bad public key for {}", k.name))?;
        }
        Ok(ring)
    }

    /// Find a key by credential id.
    pub fn find(&self, credential_id: &str) -> Option<&KeyCard> {
        self.keys.iter().find(|k| k.credential_id == credential_id)
    }
}

/// Where approvals may come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Allowed approval page origins, e.g. `https://approve.example.com`.
    pub origins: Vec<String>,
    /// Allowed WebAuthn relying party ids, e.g. `approve.example.com`.
    pub rp_ids: Vec<String>,
}

/// What the verifier expects: who it is, what it is about to do, how sure it must be.
#[derive(Debug, Clone, Copy)]
pub struct Expected<'a> {
    /// This verifier's own audience id.
    pub aud: &'a str,
    /// The request it is about to act on.
    pub request: &'a Value,
    /// The lowest level it accepts for this request.
    pub level: Level,
}

/// Check that a request has the shape `{"type": "<name>", "params": {...}}`.
pub fn request_shape_ok(request: &Value) -> bool {
    let Some(obj) = request.as_object() else {
        return false;
    };
    if obj.len() != 2 {
        return false;
    }
    let type_ok = obj.get("type").and_then(Value::as_str).is_some_and(|t| {
        !t.is_empty()
            && t.len() <= 64
            && t.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.' || b == b'-'
            })
    });
    type_ok && obj.get("params").is_some_and(Value::is_object)
}

/// Check that an id is 128 bits of base64url (22 characters).
pub fn id_ok(id: &str) -> bool {
    id.len() == 22 && crate::b64::decode(id).is_some_and(|b| b.len() == 16)
}
