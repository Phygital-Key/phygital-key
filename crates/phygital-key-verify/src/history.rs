//! Signed key history: how anyone can check a published keyring.
//!
//! Every change to a keyring is itself an approval, signed at level `high`
//! by a key that was already trusted:
//!
//! 1. The first entry is `add_key` for the first key, signed by that same key.
//!    Whoever checks the history pins this key's fingerprint once.
//! 2. Every later entry is `add_key` or `remove_key`, signed by a key that is
//!    trusted at that point.
//! 3. Replaying the history must give exactly the keys listed in the keyring.
//!
//! So a keyring fetched from a website can be trusted with one short
//! fingerprint, and nobody who controls the website (or the file) can slip in
//! an extra key without a signature from a key already trusted.

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::model::{Config, Expected, KeyCard, Keyring, Level};
use crate::{b64, check, strict};

/// The audience of key changes made on the approval page at `origin`.
pub fn keyring_aud(origin: &str) -> String {
    format!("phygital-key:keyring:{origin}")
}

/// The `add_key` request for a card.
pub fn add_key_request(card: &KeyCard) -> Value {
    json!({"type": "add_key", "params": card.to_params()})
}

/// The `remove_key` request for a card.
pub fn remove_key_request(card: &KeyCard) -> Value {
    json!({"type": "remove_key", "params": {"credential_id": card.credential_id, "name": card.name}})
}

/// Why a key history does not check out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryError {
    /// No history, or no origin.
    Unsigned,
    /// The first entry is not the pinned key approving itself.
    WrongRoot,
    /// Entry `n` (0-based) is not a valid approval by a trusted key.
    BadEntry(usize, &'static str),
    /// The keys listed are not the keys the history produces.
    KeysDoNotMatch,
}

impl HistoryError {
    /// A stable code, used by the test vectors.
    pub fn code(&self) -> &'static str {
        match self {
            HistoryError::Unsigned => "UNSIGNED",
            HistoryError::WrongRoot => "WRONG_ROOT",
            HistoryError::BadEntry(..) => "BAD_ENTRY",
            HistoryError::KeysDoNotMatch => "KEYS_DO_NOT_MATCH",
        }
    }
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistoryError::Unsigned => f.write_str("the keyring has no signed history"),
            HistoryError::WrongRoot => {
                f.write_str("the history does not start with the pinned key")
            }
            HistoryError::BadEntry(n, why) => write!(f, "history entry {}: {why}", n + 1),
            HistoryError::KeysDoNotMatch => {
                f.write_str("the listed keys are not the keys the history signs")
            }
        }
    }
}

impl std::error::Error for HistoryError {}

/// Read the claims of an approval without checking it.
fn peek(approval: &str) -> Option<(Value, i64)> {
    let raw = b64::decode(approval)?;
    let v = strict::parse(&raw)?;
    let claims = v.get("claims")?.clone();
    let iat = claims.get("iat")?.as_i64()?;
    Some((claims, iat))
}

fn same_keys(a: &[KeyCard], b: &[KeyCard]) -> bool {
    let set = |k: &[KeyCard]| -> HashSet<(String, String)> {
        k.iter()
            .map(|c| (c.credential_id.clone(), c.public_key.clone()))
            .collect()
    };
    a.len() == b.len() && set(a) == set(b)
}

/// Check a keyring's signed history against a pinned root fingerprint
/// ([`KeyCard::fingerprint`] of the first key).
///
/// Expiry is not checked: each entry is checked as of its own `iat`.
/// Everything else is: signature, trusted key, level `high`, audience,
/// exact request, single use within the history.
pub fn verify_history(ring: &Keyring, root_fingerprint: &str) -> Result<(), HistoryError> {
    use HistoryError as H;
    let origin = ring.origin.as_deref().ok_or(H::Unsigned)?;
    if ring.history.is_empty() {
        return Err(H::Unsigned);
    }
    let aud = keyring_aud(origin);
    let mut trusted: Vec<KeyCard> = Vec::new();
    let mut seen = HashSet::new();
    for (n, approval) in ring.history.iter().enumerate() {
        let (claims, iat) = peek(approval).ok_or(H::BadEntry(n, "not an approval"))?;
        let id = claims
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !seen.insert(id) {
            return Err(H::BadEntry(n, "used twice"));
        }
        let request = claims.get("request").cloned().unwrap_or(Value::Null);
        let kind = request
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let (signers, change) = match kind {
            "add_key" => {
                let card = KeyCard::from_params(&params).ok_or(H::BadEntry(n, "bad key card"))?;
                if crate::webauthn::parse_public_key(&card.public_key).is_none() {
                    return Err(H::BadEntry(n, "bad public key"));
                }
                if n == 0 {
                    if card.fingerprint() != root_fingerprint {
                        return Err(H::WrongRoot);
                    }
                    (vec![card.clone()], Some(card))
                } else {
                    if trusted
                        .iter()
                        .any(|k| k.credential_id == card.credential_id)
                    {
                        return Err(H::BadEntry(n, "key added twice"));
                    }
                    (trusted.clone(), Some(card))
                }
            }
            "remove_key" if n > 0 => {
                let cred = params
                    .get("credential_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !trusted.iter().any(|k| k.credential_id == cred) {
                    return Err(H::BadEntry(n, "removes a key that is not there"));
                }
                if trusted.len() == 1 {
                    return Err(H::BadEntry(n, "removes the last key"));
                }
                (trusted.clone(), None)
            }
            _ => {
                return Err(if n == 0 {
                    H::WrongRoot
                } else {
                    H::BadEntry(n, "not a key change")
                });
            }
        };
        let rp_ids: Vec<String> = signers.iter().map(|k| k.rp_id.clone()).collect();
        let config = Config {
            origins: vec![origin.to_string()],
            rp_ids,
        };
        let expected = Expected {
            aud: &aud,
            request: &request,
            level: Level::High,
        };
        check(approval, &expected, &Keyring::of(signers), &config, iat)
            .map_err(|_| H::BadEntry(n, "not signed by a trusted key"))?;
        match change {
            Some(card) => trusted.push(card),
            None => {
                let cred = params
                    .get("credential_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                trusted.retain(|k| k.credential_id != cred);
            }
        }
    }
    if same_keys(&trusted, &ring.keys) {
        Ok(())
    } else {
        Err(H::KeysDoNotMatch)
    }
}

/// The fingerprint of the key that started a signed history (the one to pin).
/// Read only: it says nothing until [`verify_history`] checks the chain.
pub fn root_fingerprint(ring: &Keyring) -> Option<String> {
    let (claims, _) = peek(ring.history.first()?)?;
    KeyCard::from_params(claims.get("request")?.get("params")?).map(|c| c.fingerprint())
}
