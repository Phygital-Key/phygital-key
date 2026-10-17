//! Signed key history: how anyone can check a published keyring.
//!
//! Every change to a keyring is itself an approval, signed at level `high`
//! by a key that was already trusted:
//!
//! 1. The first entry is `add_key` for the first key, signed by that same key.
//!    Whoever checks the history pins this key's fingerprint once.
//! 2. Every later entry is `add_key` or `remove_key`, signed by a key that is
//!    trusted at that point.
//! 3. Entries are in time order (`iat` never goes back), and replaying the
//!    history must give exactly the key cards listed in the keyring.
//!
//! A checked history proves every listed key was approved by an earlier
//! trusted key, back to one pinned fingerprint. It cannot prove it is the
//! *latest* history: an old copy (from before a key was removed) is also a valid
//! history. So whoever fetches a keyring more than once must keep the last one
//! they accepted and take only extensions of it: [`verify_history_update`].

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
    /// Signed on another gate's approval page.
    WrongOrigin,
    /// The new history does not extend the one accepted before (rollback or fork).
    NotAnUpdate,
}

impl HistoryError {
    /// A stable code, used by the test vectors.
    pub fn code(&self) -> &'static str {
        match self {
            HistoryError::Unsigned => "UNSIGNED",
            HistoryError::WrongRoot => "WRONG_ROOT",
            HistoryError::BadEntry(..) => "BAD_ENTRY",
            HistoryError::KeysDoNotMatch => "KEYS_DO_NOT_MATCH",
            HistoryError::WrongOrigin => "WRONG_ORIGIN",
            HistoryError::NotAnUpdate => "NOT_AN_UPDATE",
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
            HistoryError::WrongOrigin => {
                f.write_str("the history was signed on another approval page")
            }
            HistoryError::NotAnUpdate => f.write_str(
                "the new history does not extend the one accepted before (rollback or fork)",
            ),
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
    // Whole cards: id, key, RP ID, name and date must all match what was signed.
    a.len() == b.len() && a.iter().all(|x| b.iter().filter(|y| *y == x).count() == 1)
}

/// Check a keyring's signed history against a pinned root fingerprint
/// ([`KeyCard::fingerprint`] of the first key).
///
/// Expiry is not checked: each entry is checked as of its own `iat`.
/// Everything else is: signature, trusted key, level `high`, audience,
/// exact request, single use within the history.
pub fn verify_history(
    ring: &Keyring,
    root_fingerprint: &str,
    origin: &str,
) -> Result<(), HistoryError> {
    use HistoryError as H;
    let listed = ring.origin.as_deref().ok_or(H::Unsigned)?;
    if ring.history.is_empty() {
        return Err(H::Unsigned);
    }
    if listed != origin {
        return Err(H::WrongOrigin);
    }
    let mut last_iat = i64::MIN;
    let aud = keyring_aud(origin);
    let mut trusted: Vec<KeyCard> = Vec::new();
    let mut seen = HashSet::new();
    for (n, approval) in ring.history.iter().enumerate() {
        let (claims, iat) = peek(approval).ok_or(H::BadEntry(n, "not an approval"))?;
        if iat < last_iat {
            return Err(H::BadEntry(n, "out of time order"));
        }
        last_iat = iat;
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

/// Accept a refetched keyring only if it extends the one accepted before:
/// same entries first, byte for byte, then any new ones, and the whole thing valid.
/// This is what stops an old copy (rollback) or a branch signed by a removed key (fork).
pub fn verify_history_update(
    accepted: &Keyring,
    new: &Keyring,
    root_fingerprint: &str,
    origin: &str,
) -> Result<(), HistoryError> {
    if new.history.get(..accepted.history.len()) != Some(accepted.history.as_slice()) {
        return Err(HistoryError::NotAnUpdate);
    }
    verify_history(new, root_fingerprint, origin)
}

/// The fingerprint of the key that started a signed history (the one to pin).
/// Read only: it says nothing until [`verify_history`] checks the chain.
pub fn root_fingerprint(ring: &Keyring) -> Option<String> {
    let (claims, _) = peek(ring.history.first()?)?;
    KeyCard::from_params(claims.get("request")?.get("params")?).map(|c| c.fingerprint())
}
