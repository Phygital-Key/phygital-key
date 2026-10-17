//! The verifier: every check, in order, fail closed.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use serde_json::Value;
use subtle::ConstantTimeEq;

use crate::canonical;
use crate::model::{
    ApprovalObject, Claims, Config, Expected, Keyring, Level, MAX_APPROVAL_BYTES,
    MAX_CLOCK_SKEW_SECS, MAX_LIFETIME_SECS, MAX_REQUEST_BYTES, VERSION, id_ok, request_shape_ok,
};
use crate::webauthn::{self, AssertionCheck, AssertionError, FLAG_UP, FLAG_UV};
use crate::{b64, strict};

/// Why an approval was refused. The first failing check wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// No approval was given.
    ApprovalRequired,
    /// Bad encoding, bad shape, unknown fields, too big, too deep, floats.
    Malformed,
    /// Made for someone else.
    WrongAudience,
    /// Too old, not valid yet, or lives too long.
    Expired,
    /// Approved a different request.
    RequestMismatch,
    /// The assertion does not check out.
    BadSignature,
    /// The key is not in the keyring.
    UntrustedKey,
    /// Level too low, or presence / verification flags missing.
    AssuranceTooLow,
    /// Already used.
    Replayed,
    /// The person said no (returned by gates, never by `verify`).
    Denied,
    /// Outside the limits of a standing approval.
    NotAllowed,
    /// A standing approval has no uses left.
    UsedUp,
}

impl ErrorCode {
    /// The code as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::ApprovalRequired => "APPROVAL_REQUIRED",
            ErrorCode::Malformed => "MALFORMED",
            ErrorCode::WrongAudience => "WRONG_AUDIENCE",
            ErrorCode::Expired => "EXPIRED",
            ErrorCode::RequestMismatch => "REQUEST_MISMATCH",
            ErrorCode::BadSignature => "BAD_SIGNATURE",
            ErrorCode::UntrustedKey => "UNTRUSTED_KEY",
            ErrorCode::AssuranceTooLow => "ASSURANCE_TOO_LOW",
            ErrorCode::Replayed => "REPLAYED",
            ErrorCode::Denied => "DENIED",
            ErrorCode::NotAllowed => "NOT_ALLOWED",
            ErrorCode::UsedUp => "USED_UP",
        }
    }

    /// Every code, for tests and docs.
    pub const ALL: [ErrorCode; 12] = [
        ErrorCode::ApprovalRequired,
        ErrorCode::Malformed,
        ErrorCode::WrongAudience,
        ErrorCode::Expired,
        ErrorCode::RequestMismatch,
        ErrorCode::BadSignature,
        ErrorCode::UntrustedKey,
        ErrorCode::AssuranceTooLow,
        ErrorCode::Replayed,
        ErrorCode::Denied,
        ErrorCode::NotAllowed,
        ErrorCode::UsedUp,
    ];
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for ErrorCode {}

/// A checked approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// The single-use id.
    pub id: String,
    /// base64url SHA-256 of the canonical request.
    pub request_hash: String,
    /// The key that signed.
    pub credential_id: String,
    /// Name on the key card.
    pub approver: String,
    /// The level that was approved.
    pub level: Level,
    /// Issued at.
    pub iat: i64,
    /// Expires at.
    pub exp: i64,
}

/// Remembers used approval ids until they expire.
pub trait SeenStore {
    /// Atomically record `id` if it is new. Returns `false` if it was seen before.
    /// `exp` says how long it must be remembered; `now` lets the store forget old ids.
    fn insert_if_new(&self, id: &str, exp: i64, now: i64) -> bool;
}

/// A seen store in memory, for one process. Use a shared store (database,
/// Redis `SET NX`) when several instances act on approvals.
#[derive(Debug, Default)]
pub struct MemorySeenStore {
    seen: Mutex<HashMap<String, i64>>,
}

impl MemorySeenStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SeenStore for MemorySeenStore {
    fn insert_if_new(&self, id: &str, exp: i64, now: i64) -> bool {
        let Ok(mut seen) = self.seen.lock() else {
            return false;
        };
        // Keep ids a little past expiry, so a clock that steps back cannot replay them.
        seen.retain(|_, e| e.saturating_add(MAX_CLOCK_SKEW_SECS) > now);
        if seen.contains_key(id) {
            return false;
        }
        seen.insert(id.to_owned(), exp);
        true
    }
}

/// Run every check except the replay check. Gates use this to give the person
/// early feedback without spending the id. Verifiers use [`verify`].
pub fn check(
    approval: &str,
    expected: &Expected,
    keyring: &Keyring,
    config: &Config,
    now: i64,
) -> Result<Verified, ErrorCode> {
    check_kind(approval, expected, keyring, config, now, false)
}

/// The type name reserved for standing approvals. Never an action.
pub const STANDING_TYPE: &str = "standing";

/// [`check`] for one-time approvals (`standing == false`) or standing ones.
/// A standing approval can never pass as a one-time approval, or the reverse.
pub(crate) fn check_kind(
    approval: &str,
    expected: &Expected,
    keyring: &Keyring,
    config: &Config,
    now: i64,
    standing: bool,
) -> Result<Verified, ErrorCode> {
    use ErrorCode as E;

    // 1. Present, well formed, within limits.
    if approval.trim().is_empty() {
        return Err(E::ApprovalRequired);
    }
    if approval.len() > MAX_APPROVAL_BYTES {
        return Err(E::Malformed);
    }
    let raw = b64::decode(approval).ok_or(E::Malformed)?;
    let value = strict::parse(&raw).ok_or(E::Malformed)?;
    let obj: ApprovalObject = serde_json::from_value(value).map_err(|_| E::Malformed)?;
    let claims_value = obj.claims;
    let claims: Claims = serde_json::from_value(claims_value.clone()).map_err(|_| E::Malformed)?;
    if claims.v != VERSION || !id_ok(&claims.id) || !request_shape_ok(&claims.request) {
        return Err(E::Malformed);
    }
    let is_standing = claims.request.get("type").and_then(Value::as_str) == Some(STANDING_TYPE);
    if is_standing != standing {
        return Err(E::Malformed);
    }
    // Standing approvals always need fingerprint, face or PIN, whatever the service asks.
    if standing && claims.level != Level::High {
        return Err(E::AssuranceTooLow);
    }
    let max_life = if standing {
        crate::standing::MAX_STANDING_SECS
    } else {
        MAX_LIFETIME_SECS
    };
    let [sig] = obj.signatures.as_slice() else {
        return Err(E::Malformed);
    };
    let challenge = canonical::challenge(&claims_value).map_err(|_| E::Malformed)?;
    let claims_request = canonical::canonical_bytes(&claims.request).map_err(|_| E::Malformed)?;
    if claims_request.len() > MAX_REQUEST_BYTES {
        return Err(E::Malformed);
    }
    let (Some(cred), Some(auth_data), Some(client_data), Some(sig_der)) = (
        b64::decode(&sig.credential_id),
        b64::decode(&sig.authenticator_data),
        b64::decode(&sig.client_data_json),
        b64::decode(&sig.signature),
    ) else {
        return Err(E::Malformed);
    };
    if cred.is_empty() {
        return Err(E::Malformed);
    }

    // 2. Made for us.
    if claims.aud != expected.aud {
        return Err(E::WrongAudience);
    }

    // 3. Inside its time window.
    if claims.iat > now.saturating_add(MAX_CLOCK_SKEW_SECS)
        || now >= claims.exp
        || claims.exp <= claims.iat
        || claims.exp.saturating_sub(claims.iat) > max_life
    {
        return Err(E::Expired);
    }

    // 4. Approves exactly the request we are about to run.
    let ours = canonical::canonical_bytes(expected.request).map_err(|_| E::Malformed)?;
    if !request_shape_ok(expected.request) {
        return Err(E::Malformed);
    }
    let ours_hash = b64::encode(sha256(&ours));
    let theirs_hash = b64::encode(sha256(&claims_request));
    if !bool::from(ours_hash.as_bytes().ct_eq(claims.request_hash.as_bytes()))
        || !bool::from(theirs_hash.as_bytes().ct_eq(claims.request_hash.as_bytes()))
    {
        return Err(E::RequestMismatch);
    }

    // The level the person approved must be at least what we require.
    if claims.level < expected.level {
        return Err(E::AssuranceTooLow);
    }

    // 5. Signed by a key we trust.
    let card = keyring.find(&sig.credential_id).ok_or(E::UntrustedKey)?;
    if !config.rp_ids.iter().any(|r| r == &card.rp_id) {
        return Err(E::UntrustedKey);
    }
    let key = webauthn::parse_public_key(&card.public_key).ok_or(E::UntrustedKey)?;

    // 6. The assertion checks out, and proves enough.
    let want = AssertionCheck {
        kind: "webauthn.get",
        challenge: &challenge,
        origins: &config.origins,
        rp_id: &card.rp_id,
    };
    let flags = webauthn::verify_assertion(&key, &auth_data, &client_data, &sig_der, &want)
        .map_err(|e| match e {
            AssertionError::BadSignature => E::BadSignature,
            AssertionError::AssuranceTooLow => E::AssuranceTooLow,
        })?;
    if flags & FLAG_UP == 0 || (claims.level == Level::High && flags & FLAG_UV == 0) {
        return Err(E::AssuranceTooLow);
    }

    Ok(Verified {
        id: claims.id,
        request_hash: claims.request_hash,
        credential_id: card.credential_id.clone(),
        approver: card.name.clone(),
        level: claims.level,
        iat: claims.iat,
        exp: claims.exp,
    })
}

/// Verify an approval before acting on `expected.request`.
///
/// Pure: no network, no clock (pass `now` in Unix seconds), no state except
/// `seen`. On `Ok`, the id is spent: act, then log the result.
pub fn verify(
    approval: &str,
    expected: &Expected,
    keyring: &Keyring,
    seen: &dyn SeenStore,
    config: &Config,
    now: i64,
) -> Result<Verified, ErrorCode> {
    let ok = check(approval, expected, keyring, config, now)?;
    // 7. Never used before. Spent now, even if acting fails later.
    if !seen.insert_if_new(&ok.id, ok.exp, now) {
        return Err(ErrorCode::Replayed);
    }
    Ok(ok)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).into()
}

/// Everything a service needs, in one place. Reads the system clock.
///
/// ```no_run
/// use phygital_key_verify::{Config, Keyring, Level, Verifier};
/// # fn run(approval: &str, keyring_json: &str) -> Result<(), Box<dyn std::error::Error>> {
/// let verifier = Verifier::new(
///     "https://deploy.example.com",
///     Keyring::from_json(keyring_json)?,
///     Config { origins: vec!["https://approve.example.com".into()], rp_ids: vec!["approve.example.com".into()] },
/// );
/// let request = serde_json::json!({"type": "deploy", "params": {"service": "api", "version": "2.4.1"}});
/// verifier.verify(approval, &request, Level::High)?; // refuses on any doubt
/// // ...now deploy.
/// # Ok(()) }
/// ```
pub struct Verifier<S: SeenStore = MemorySeenStore> {
    aud: String,
    keyring: Keyring,
    config: Config,
    seen: S,
}

impl Verifier<MemorySeenStore> {
    /// A verifier with an in-memory seen store.
    pub fn new(aud: impl Into<String>, keyring: Keyring, config: Config) -> Self {
        Self::with_store(aud, keyring, config, MemorySeenStore::new())
    }
}

impl<S: SeenStore> Verifier<S> {
    /// A verifier with your own seen store.
    pub fn with_store(aud: impl Into<String>, keyring: Keyring, config: Config, seen: S) -> Self {
        Self {
            aud: aud.into(),
            keyring,
            config,
            seen,
        }
    }

    /// Replace the keyring (for example after reloading it from disk).
    pub fn set_keyring(&mut self, keyring: Keyring) {
        self.keyring = keyring;
    }

    /// Verify `approval` for `request` at `level` or above, now.
    pub fn verify(
        &self,
        approval: &str,
        request: &Value,
        level: Level,
    ) -> Result<Verified, ErrorCode> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
            .map_err(|_| ErrorCode::Expired)?;
        let expected = Expected {
            aud: &self.aud,
            request,
            level,
        };
        verify(
            approval,
            &expected,
            &self.keyring,
            &self.seen,
            &self.config,
            now,
        )
    }
}
