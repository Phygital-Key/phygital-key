//! Standing approvals: "allow this, within these limits, a few times, until then".
//!
//! Kept deliberately narrow (the AP2 "open mandate" idea, without a policy language):
//!
//! 1. The person approves, at level `high`, a request of type `standing`:
//!
//!    ```json
//!    {"type": "standing", "params": {
//!       "for_type": "payment",
//!       "fixed": {"to": "ACME Ltd", "currency": "EUR"},
//!       "vary": {"amount_cents": {"min": 1, "max": 2000}, "memo": {"one_of": ["coffee", "lunch"]}},
//!       "max_uses": 5,
//!       "use_key": "<base64url SPKI of a P-256 key held by the gate>"}}
//!    ```
//!
//!    Its `exp` is when the permission ends (at most 30 days after `iat`).
//! 2. For each use, the holder of `use_key` signs a short **use token**
//!    (ES256 JWS, `typ: pka-use+jwt`, life at most 60 s) for one exact request.
//! 3. The verifier checks both, checks the request is inside the limits, and
//!    counts uses itself ([`UseStore`]).
//!
//! The request must have exactly the `fixed` and `vary` fields: nothing missing,
//! nothing extra. `vary` limits are integer ranges (`min` and `max`, both required,
//! so a forgotten lower limit can never let negative amounts through) or a short list
//! of allowed strings (`one_of`). Anything else is refused.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use p256::ecdsa::signature::{Signer, Verifier as _};
use p256::ecdsa::{Signature, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use subtle::ConstantTimeEq;

use crate::model::{Config, Expected, Keyring, MAX_CLOCK_SKEW_SECS, request_shape_ok};
use crate::verify::{ErrorCode, REVOKE_TYPE, STANDING_TYPE, Verified, check_kind};
use crate::{b64, canonical, strict, webauthn};

/// Longest a standing approval may live: 30 days.
pub const MAX_STANDING_SECS: i64 = 30 * 24 * 3600;
/// Most uses one standing approval may allow.
pub const MAX_USES: u32 = 1000;
/// Longest life of one use token.
pub const MAX_USE_SECS: i64 = 60;
/// Most strings in a `one_of` list.
pub const MAX_ONE_OF: usize = 20;
/// Most entries read from one revocation list.
pub const MAX_REVOCATIONS: usize = 1000;
/// The `typ` of a use token's JWS header.
pub const USE_TYP: &str = "pka-use+jwt";

/// One varying field's limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Limit {
    /// An integer from `min` to `max`, both included. Both are required.
    Range {
        /// Smallest allowed value.
        min: i64,
        /// Largest allowed value.
        max: i64,
    },
    /// One of these strings.
    OneOf {
        /// The allowed strings.
        one_of: Vec<String>,
    },
}

/// The params of a `standing` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandingParams {
    /// The action type each use must have.
    pub for_type: String,
    /// Fields that must have exactly these values.
    pub fixed: Map<String, Value>,
    /// Fields that may change, within limits.
    pub vary: std::collections::BTreeMap<String, Limit>,
    /// How many times it may be used.
    pub max_uses: u32,
    /// Public key (SPKI DER, base64url) that signs each use token.
    pub use_key: String,
}

impl StandingParams {
    /// Read and check the params of a standing request.
    pub fn from_request(request: &Value) -> Result<StandingParams, ErrorCode> {
        if request.get("type").and_then(Value::as_str) != Some(STANDING_TYPE) {
            return Err(ErrorCode::Malformed);
        }
        let p: StandingParams =
            serde_json::from_value(request.get("params").cloned().unwrap_or(Value::Null))
                .map_err(|_| ErrorCode::Malformed)?;
        let shape = json!({"type": p.for_type, "params": {}});
        let limits_ok = p.vary.values().all(|l| match l {
            Limit::Range { max, min } => min <= max,
            Limit::OneOf { one_of } => !one_of.is_empty() && one_of.len() <= MAX_ONE_OF,
        });
        if !request_shape_ok(&shape)
            || p.for_type == STANDING_TYPE
            || !(1..=MAX_USES).contains(&p.max_uses)
            || p.fixed.keys().any(|k| p.vary.contains_key(k))
            || !limits_ok
            || webauthn::parse_public_key(&p.use_key).is_none()
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(p)
    }

    /// The `standing` request for these params.
    pub fn to_request(&self) -> Value {
        json!({"type": STANDING_TYPE, "params": self})
    }

    /// Is `request` inside these limits? Exactly the fixed and varying fields, nothing else.
    pub fn allows(&self, request: &Value) -> bool {
        if request.get("type").and_then(Value::as_str) != Some(self.for_type.as_str()) {
            return false;
        }
        let Some(params) = request.get("params").and_then(Value::as_object) else {
            return false;
        };
        if params.len() != self.fixed.len() + self.vary.len() {
            return false;
        }
        let fixed_ok = self.fixed.iter().all(|(k, want)| {
            let (Some(got), Ok(w)) = (params.get(k), canonical::canonical_bytes(want)) else {
                return false;
            };
            canonical::canonical_bytes(got).is_ok_and(|g| g == w)
        });
        let vary_ok = self
            .vary
            .iter()
            .all(|(k, limit)| match (params.get(k), limit) {
                (Some(Value::Number(n)), Limit::Range { max, min }) => {
                    n.as_i64().is_some_and(|v| (*min..=*max).contains(&v))
                }
                (Some(Value::String(s)), Limit::OneOf { one_of }) => one_of.iter().any(|o| o == s),
                _ => false,
            });
        fixed_ok && vary_ok
    }
}

/// One use of a standing approval, signed by the holder of `use_key`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Use {
    /// Always `pka.use.1`.
    pub v: String,
    /// The standing approval's id.
    pub standing_id: String,
    /// This use's own single-use id (128-bit, base64url).
    pub id: String,
    /// Who acts on it.
    pub aud: String,
    /// The exact request.
    pub request: Value,
    /// Its hash.
    pub request_hash: String,
    /// Issued at.
    pub iat: i64,
    /// Expires at (at most 60 s after `iat`).
    pub exp: i64,
}

fn jws_input(payload: &Value, typ: &str) -> Result<String, ErrorCode> {
    let header = json!({"alg": "ES256", "typ": typ});
    let h = canonical::canonical_bytes(&header).map_err(|_| ErrorCode::Malformed)?;
    let p = canonical::canonical_bytes(payload).map_err(|_| ErrorCode::Malformed)?;
    Ok(format!("{}.{}", b64::encode(h), b64::encode(p)))
}

/// Sign a use token (done by the gate that holds the use key).
pub fn sign_use(u: &Use, key: &SigningKey) -> Result<String, ErrorCode> {
    let input = jws_input(
        &serde_json::to_value(u).map_err(|_| ErrorCode::Malformed)?,
        USE_TYP,
    )?;
    let sig: Signature = key.sign(input.as_bytes());
    Ok(format!("{input}.{}", b64::encode(sig.to_bytes())))
}

fn read_use(jws: &str, use_key: &str) -> Result<Use, ErrorCode> {
    use ErrorCode as E;
    if jws.trim().is_empty() {
        return Err(E::ApprovalRequired);
    }
    if jws.len() > crate::MAX_APPROVAL_BYTES {
        return Err(E::Malformed);
    }
    let mut parts = jws.split('.');
    let (Some(h), Some(p), Some(s), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(E::Malformed);
    };
    // The header must be exactly these bytes (as the spec says), not just equal JSON.
    let want = canonical::canonical_bytes(&json!({"alg": "ES256", "typ": USE_TYP}))
        .map_err(|_| E::Malformed)?;
    if b64::decode(h).as_deref() != Some(want.as_slice()) {
        return Err(E::Malformed);
    }
    let sig = b64::decode(s)
        .and_then(|b| Signature::from_slice(&b).ok())
        .ok_or(E::Malformed)?;
    let key = webauthn::parse_public_key(use_key).ok_or(E::Malformed)?;
    key.verify(format!("{h}.{p}").as_bytes(), &sig)
        .map_err(|_| E::BadSignature)?;
    let payload = b64::decode(p)
        .and_then(|b| strict::parse(&b))
        .ok_or(E::Malformed)?;
    let u: Use = serde_json::from_value(payload).map_err(|_| E::Malformed)?;
    if u.v != "pka.use.1" || !crate::id_ok(&u.id) {
        return Err(E::Malformed);
    }
    Ok(u)
}

/// Counts uses of standing approvals.
pub trait UseStore {
    /// Atomically record use `use_id` of `standing_id`, allowed `max_uses` times.
    /// Keep it until `until`. Err `REPLAYED` if this use id was seen, `USED_UP` if no uses are left.
    fn record_use(
        &self,
        standing_id: &str,
        use_id: &str,
        max_uses: u32,
        until: i64,
        now: i64,
    ) -> Result<u32, ErrorCode>;
}

/// A use store in memory, for one process.
#[derive(Debug, Default)]
pub struct MemoryUseStore {
    uses: Mutex<HashMap<String, (i64, HashSet<String>)>>,
}

impl MemoryUseStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl UseStore for MemoryUseStore {
    fn record_use(
        &self,
        standing_id: &str,
        use_id: &str,
        max_uses: u32,
        until: i64,
        now: i64,
    ) -> Result<u32, ErrorCode> {
        let Ok(mut all) = self.uses.lock() else {
            return Err(ErrorCode::UsedUp);
        };
        all.retain(|_, (u, _)| u.saturating_add(MAX_CLOCK_SKEW_SECS) > now);
        let entry = all
            .entry(standing_id.to_string())
            .or_insert_with(|| (until, HashSet::new()));
        if entry.1.contains(use_id) {
            return Err(ErrorCode::Replayed);
        }
        if entry.1.len() >= max_uses as usize {
            return Err(ErrorCode::UsedUp);
        }
        entry.1.insert(use_id.to_string());
        Ok(max_uses - u32::try_from(entry.1.len()).unwrap_or(max_uses))
    }
}

/// A checked use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedUse {
    /// The standing approval (the person's signature).
    pub standing: Verified,
    /// This use's id.
    pub use_id: String,
    /// Uses left after this one.
    pub uses_left: u32,
}

/// Verify one use of a standing approval before acting on `expected.request`.
///
/// `standing` is the person's approval of the `standing` request; `use_token`
/// is the short token for this one request. Pure, like [`crate::verify`].
#[allow(clippy::too_many_arguments)]
pub fn verify_use(
    standing: &str,
    use_token: &str,
    expected: &Expected,
    keyring: &Keyring,
    uses: &dyn UseStore,
    revoked: &Revocations,
    config: &Config,
    now: i64,
) -> Result<VerifiedUse, ErrorCode> {
    use ErrorCode as E;
    if standing.trim().is_empty() {
        return Err(E::ApprovalRequired);
    }
    // The standing request is whatever the person signed; check that signature first.
    let raw = b64::decode(standing)
        .filter(|_| standing.len() <= crate::MAX_APPROVAL_BYTES)
        .ok_or(E::Malformed)?;
    let signed_request = strict::parse(&raw)
        .and_then(|v| v.get("claims")?.get("request").cloned())
        .ok_or(E::Malformed)?;
    let params = StandingParams::from_request(&signed_request)?;
    let person = check_kind(
        standing,
        &Expected {
            aud: expected.aud,
            request: &signed_request,
            level: expected.level,
        },
        keyring,
        config,
        now,
        true,
    )?;

    // Revoked by the person (a signed revocation the service has picked up).
    if revoked.contains(&person.id) {
        return Err(E::Revoked);
    }

    // The use token: signed by the use key, for this standing approval, this service, now.
    let u = read_use(use_token, &params.use_key)?;
    if !bool::from(u.standing_id.as_bytes().ct_eq(person.id.as_bytes())) {
        return Err(E::RequestMismatch);
    }
    if u.aud != expected.aud {
        return Err(E::WrongAudience);
    }
    if u.iat > now.saturating_add(MAX_CLOCK_SKEW_SECS)
        || now >= u.exp
        || u.exp <= u.iat
        || u.exp.saturating_sub(u.iat) > MAX_USE_SECS
        || u.exp > person.exp
    {
        return Err(E::Expired);
    }

    // Exactly the request we are about to run.
    if !request_shape_ok(expected.request)
        || expected.request.get("type").and_then(Value::as_str) == Some(STANDING_TYPE)
    {
        return Err(E::Malformed);
    }
    let ours = canonical::canonical_bytes(expected.request).map_err(|_| E::Malformed)?;
    let theirs = canonical::canonical_bytes(&u.request).map_err(|_| E::Malformed)?;
    let ours_hash = canonical::hash_b64(expected.request).map_err(|_| E::Malformed)?;
    if ours != theirs || !bool::from(ours_hash.as_bytes().ct_eq(u.request_hash.as_bytes())) {
        return Err(E::RequestMismatch);
    }

    // Inside the limits the person set.
    if !params.allows(expected.request) {
        return Err(E::NotAllowed);
    }

    // Counted by the verifier itself.
    let uses_left = uses.record_use(&person.id, &u.id, params.max_uses, person.exp, now)?;
    Ok(VerifiedUse {
        standing: person,
        use_id: u.id,
        uses_left,
    })
}

/// Every check of a standing approval except counting uses (for gates).
pub fn check_standing(
    approval: &str,
    expected: &Expected,
    keyring: &Keyring,
    config: &Config,
    now: i64,
) -> Result<Verified, ErrorCode> {
    check_kind(approval, expected, keyring, config, now, true)
}

/// The request a person signs to revoke a standing approval.
pub fn revoke_request(standing_id: &str) -> Value {
    json!({"type": REVOKE_TYPE, "params": {"standing_id": standing_id}})
}

/// Standing approvals the person has revoked, each proven by their own signature.
///
/// A gate publishes signed revocations at `/.well-known/phygital-key-revoked.json`
/// (`{"revocations": ["<approval>", ...]}`). A service fetches that list now and
/// then and builds this set: only revocations signed by a trusted key, for this
/// service, count; unsigned or foreign entries are ignored. A service MUST keep
/// every revocation it has accepted ([`Revocations::merge`]): whoever serves the
/// list could leave entries out later, and removing the signing key from the
/// keyring would make an old revocation stop checking. At most
/// [`MAX_REVOCATIONS`] entries are read from one list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Revocations {
    ids: HashSet<String>,
}

impl Revocations {
    /// No revocations.
    pub fn none() -> Self {
        Self::default()
    }

    /// Check each signed revocation and keep the standing ids of the valid ones.
    pub fn from_approvals(
        approvals: &[String],
        aud: &str,
        keyring: &Keyring,
        config: &Config,
    ) -> Self {
        let mut ids = HashSet::new();
        for a in approvals.iter().take(MAX_REVOCATIONS) {
            let Some(claims) = b64::decode(a)
                .filter(|_| a.len() <= crate::MAX_APPROVAL_BYTES)
                .and_then(|raw| strict::parse(&raw))
                .and_then(|v| v.get("claims").cloned())
            else {
                continue;
            };
            let request = claims.get("request").cloned().unwrap_or(Value::Null);
            let (Some(iat), Some(id)) = (
                claims.get("iat").and_then(Value::as_i64),
                request
                    .pointer("/params/standing_id")
                    .and_then(Value::as_str),
            ) else {
                continue;
            };
            if request != revoke_request(id) {
                continue;
            }
            // Checked as of its own time: a revocation never expires.
            let expected = Expected {
                aud,
                request: &request,
                level: crate::Level::Medium,
            };
            if check_kind(a, &expected, keyring, config, iat, false).is_ok() {
                ids.insert(id.to_string());
            }
        }
        Self { ids }
    }

    /// Read `{"revocations": [...]}` and check every entry.
    pub fn from_json(text: &str, aud: &str, keyring: &Keyring, config: &Config) -> Self {
        let list: Vec<String> = serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|v| v.get("revocations").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        Self::from_approvals(&list, aud, keyring, config)
    }

    /// Add revocations from a newer list, keeping everything accepted before.
    pub fn merge(&mut self, other: Revocations) {
        self.ids.extend(other.ids);
    }

    /// Was this standing approval revoked?
    pub fn contains(&self, standing_id: &str) -> bool {
        self.ids.contains(standing_id)
    }

    /// How many valid revocations.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when there are none.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}
