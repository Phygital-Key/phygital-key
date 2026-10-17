# Rust verifier (`phygital-key-verify`)

`phygital-key-verify` is the Rust library a service uses to check a Phygital Key approval before it acts. This page lists every public item with its exact signature, then shows complete examples. For where the checks fit in a service, read [integrate](integrate.md) first; terms are in the [glossary](glossary.md).

On this page:

- [Install](#install)
- [Quick start](#quick-start)
- [The crate at a glance](#the-crate-at-a-glance)
- [Core API](#core-api)
- [Standing approvals: `standing`](#standing-approvals-standing)
- [Receipts: `receipt`](#receipts-receipt)
- [Key history: `history`](#key-history-history)
- [Lower-level modules](#lower-level-modules)
- [Implementing `SeenStore`](#implementing-seenstore)
- [Implementing `UseStore`](#implementing-usestore)
- [Errors](#errors)
- [Threads](#threads)
- [How it is tested](#how-it-is-tested)

## Install

Add the crate from the repository as a git dependency. Pin a tag so a later change in `main` never reaches your build unseen.

```toml
[dependencies]
phygital-key-verify = { git = "https://github.com/s17sh70x2jfl031ag/phygital-key-mcp", tag = "v0.3.0" }
serde_json = "1"
```

Cargo finds the crate by name inside the repository's workspace. Requirements from the workspace `Cargo.toml`:

| Setting | Value |
|---|---|
| Minimum Rust version (`rust-version`) | 1.89 |
| Edition | 2024 |
| Licence | Apache-2.0 |
| `unsafe` code | forbidden (`#![forbid(unsafe_code)]`) |

Its dependencies are `serde`, `serde_json`, `serde_jcs` (RFC 8785), `sha2`, `p256` 0.14 (ECDSA P-256), `base64`, `subtle` (constant-time compares) and `unicode-normalization`. To sign receipts you also need `p256` yourself, for the `SigningKey` type:

```toml
p256 = { version = "0.14", features = ["ecdsa", "pkcs8"] }
```

## Quick start

`Verifier` holds your audience, keyring, config and an in-memory seen store, and reads the system clock.

```rust
use phygital_key_verify::{Config, Keyring, Level, Verifier};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keyring = Keyring::from_json(&std::fs::read_to_string("keyring.json")?)?;
    let config = Config {
        origins: vec!["https://approve.example.com".into()],
        rp_ids: vec!["approve.example.com".into()],
    };
    let verifier = Verifier::new("https://deploy.example.com", keyring, config);

    let approval = std::fs::read_to_string("approval.txt")?;
    let request = serde_json::json!({
        "type": "deploy",
        "params": {"service": "api", "env": "production", "version": "2.4.1"}
    });
    let ok = verifier.verify(approval.trim(), &request, Level::High)?; // refuses on any doubt
    println!("approved by {} (id {})", ok.approver, ok.id);
    // ...now deploy.
    Ok(())
}
```

The crate also ships `examples/basic.rs`, which runs the first shared test vector twice and shows the second use refused: `cargo run -p phygital-key-verify --example basic` from a clone of the repository.

## The crate at a glance

| Path | What is in it |
|---|---|
| `phygital_key_verify` (root) | `verify`, `check`, `Verifier`, `Expected`, `Config`, `Keyring`, `KeyCard`, `Level`, `Verified`, `ErrorCode`, `SeenStore`, `MemorySeenStore`, the data types and limits |
| `standing` | Standing approvals: `verify_use`, `UseStore`, `MemoryUseStore`, `Revocations` |
| `receipt` | Signed receipts: `Receipt`, `sign`, `verify`, `verify_use_receipt` |
| `history` | Signed key history: `verify_history`, `verify_history_update` |
| `canonical` | Canonical form (NFC check, safe integers, RFC 8785), `request_hash`, challenge |
| `strict` | Strict JSON parsing |
| `b64` | base64url without padding |
| `webauthn` | The part of WebAuthn a verifier needs: one ES256 assertion |

Everything is pure: no network, no file access, no global state. Only `Verifier::verify` reads the clock.

## Core API

### `verify`

```rust
pub fn verify(
    approval: &str,
    expected: &Expected,
    keyring: &Keyring,
    seen: &dyn SeenStore,
    config: &Config,
    now: i64,
) -> Result<Verified, ErrorCode>
```

Runs every check of spec section 7, in order, and returns the first failing code. `now` is the current time in Unix seconds. On `Ok`, the approval's id is spent in `seen`: act, then log. The id stays spent even if your action fails.

Before the checks, `verify` refuses a request of type `revoke` (`MALFORMED`): a revocation is never an action.

| Step | Check | Code on failure |
|---|---|---|
| 1 | Present; at most 16384 characters; base64url; strict JSON; no unknown fields; `v` is `pka.1`; `id` is 128-bit base64url; request shape; not a standing approval; exactly one signature; canonical form exists; claims request at most 8192 bytes; every signature field decodes | `APPROVAL_REQUIRED` (empty), else `MALFORMED` |
| 2 | `claims.aud == expected.aud` | `WRONG_AUDIENCE` |
| 3 | `iat <= now + 30`, `now < exp`, `iat < exp`, `exp - iat <= 300` | `EXPIRED` |
| 4 | Your request has a canonical form and the `{type, params}` shape; its hash and the claims request's hash both equal `request_hash` | `MALFORMED` (your request), else `REQUEST_MISMATCH` |
| 5 | `claims.level >= expected.level` | `ASSURANCE_TOO_LOW` |
| 6 | `credential_id` is in the keyring, its `rp_id` is in `config.rp_ids`, its public key parses | `UNTRUSTED_KEY` |
| 7 | Client data at most 4096 bytes, `type` is `webauthn.get`, challenge matches, origin in `config.origins`, not cross-origin; RP ID hash; ES256 signature over `authenticator_data || SHA-256(client_data_json)` | `BAD_SIGNATURE` |
| 8 | User present flag; user verified flag when the level is `high` | `ASSURANCE_TOO_LOW` |
| 9 | `seen.insert_if_new(id, exp, now)` returns `true` | `REPLAYED` |

The challenge the person's key signed is:

$$\text{challenge} = \mathrm{SHA256}\big(\texttt{"phygital-key/approval/v1"} \,\|\, \texttt{0x00} \,\|\, \mathrm{JCS}(\text{claims})\big)$$

### `check`

```rust
pub fn check(
    approval: &str,
    expected: &Expected,
    keyring: &Keyring,
    config: &Config,
    now: i64,
) -> Result<Verified, ErrorCode>
```

Steps 1 to 8, without the replay check and without the `revoke` refusal. Gates use it to give early feedback without spending the id. A service that acts must use `verify`, or spend the id itself right after `check` (see [implementing `SeenStore`](#implementing-seenstore)).

### `Verifier`

```rust
pub struct Verifier<S: SeenStore = MemorySeenStore> { /* private */ }

impl Verifier<MemorySeenStore> {
    pub fn new(aud: impl Into<String>, keyring: Keyring, config: Config) -> Self
}

impl<S: SeenStore> Verifier<S> {
    pub fn with_store(aud: impl Into<String>, keyring: Keyring, config: Config, seen: S) -> Self
    pub fn set_keyring(&mut self, keyring: Keyring)
    pub fn verify(&self, approval: &str, request: &Value, level: Level) -> Result<Verified, ErrorCode>
}
```

| Method | Meaning |
|---|---|
| `new` | A verifier with an in-memory seen store (one process). |
| `with_store` | A verifier with your own seen store. |
| `set_keyring` | Replace the keyring, for example after reloading a checked copy. |
| `verify` | `verify(...)` with `now` from the system clock. A clock before 1970 gives `EXPIRED`. |

### `Expected`

```rust
#[derive(Debug, Clone, Copy)]
pub struct Expected<'a> {
    pub aud: &'a str,
    pub request: &'a Value,
    pub level: Level,
}
```

What the verifier expects: your own audience, the request you are about to run (built from your data), and the lowest level you accept for it.

### `Config`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub origins: Vec<String>,
    pub rp_ids: Vec<String>,
}
```

Allowed approval page origins (`https://approve.example.com`) and WebAuthn relying party ids (`approve.example.com`). It deserializes from `{"origins": [...], "rp_ids": [...]}`.

### `Level`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level { Medium, High }

impl Level {
    pub fn as_str(self) -> &'static str
    pub fn parse(s: &str) -> Option<Level>
}
```

`Medium` is a touch (user presence), `High` is fingerprint, face or PIN (user verification). `Medium < High`. `parse` accepts only `"medium"` and `"high"`.

### `Keyring` and `KeyCard`

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyring {
    pub keys: Vec<KeyCard>,
    pub origin: Option<String>,   // omitted when None
    pub history: Vec<String>,     // omitted when empty
}

impl Keyring {
    pub fn from_json(text: &str) -> Result<Keyring, String>
    pub fn find(&self, credential_id: &str) -> Option<&KeyCard>
    pub fn of(keys: Vec<KeyCard>) -> Keyring
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyCard {
    pub credential_id: String,
    pub public_key: String,
    pub rp_id: String,
    pub name: String,
    pub created: String,
}

impl KeyCard {
    pub fn fingerprint(&self) -> String
    pub fn to_params(&self) -> Value
    pub fn from_params(params: &Value) -> Option<KeyCard>
}
```

| Item | Meaning |
|---|---|
| `Keyring::from_json` | Parse `{"keys": [...], "origin"?, "history"?}`; refuses unknown fields and any card whose public key is not a P-256 SPKI. Deserializing with `serde_json::from_value` skips the key check (a bad key then fails as `UNTRUSTED_KEY`). |
| `Keyring::find` | The card with this credential id. |
| `Keyring::of` | A keyring of these cards, with no origin and no history. |
| `KeyCard::fingerprint` | base64url SHA-256 of the SPKI DER public key: what people pin to trust a whole key history. |
| `KeyCard::to_params` / `from_params` | The card as `add_key` params, and back. |

`public_key` is the SubjectPublicKeyInfo DER of a P-256 key, base64url. `name` and `created` are for display only, but they are signed in the key history and must match it.

$$\text{fingerprint} = \mathrm{base64url}\big(\mathrm{SHA256}(\text{SPKI DER of the public key})\big)$$

### `Verified`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub id: String,
    pub request_hash: String,
    pub credential_id: String,
    pub approver: String,
    pub level: Level,
    pub iat: i64,
    pub exp: i64,
}
```

The checked approval. `approver` is the `name` on the key card that signed. Log `id`, `request_hash`, `credential_id` and `approver` with your result.

### `SeenStore` and `MemorySeenStore`

```rust
pub trait SeenStore {
    fn insert_if_new(&self, id: &str, exp: i64, now: i64) -> bool;
}

#[derive(Debug, Default)]
pub struct MemorySeenStore { /* Mutex<HashMap<String, i64>> */ }

impl MemorySeenStore {
    pub fn new() -> Self
}
```

`insert_if_new` must record `id` atomically and return `true` only if it was not there. `exp` says how long it must be remembered; `now` lets the store forget old ids. `MemorySeenStore` keeps each id while `exp + 30 > now`, and answers `false` if its mutex is poisoned (fail closed).

### `ErrorCode`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    ApprovalRequired, Malformed, WrongAudience, Expired, RequestMismatch, BadSignature,
    UntrustedKey, AssuranceTooLow, Replayed, Denied, NotAllowed, UsedUp, Revoked,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str
    pub const ALL: [ErrorCode; 13]
}
```

`as_str` gives the wire form (`"REQUEST_MISMATCH"`). `ErrorCode` implements `Display` (same text) and `std::error::Error`, so `?` works in functions returning `Box<dyn Error>`. Every code is described in [error codes](error-codes.md).

### Data types and limits

| Item | Signature or value | Meaning |
|---|---|---|
| `Claims` | `pub struct Claims { pub v: String, pub id: String, pub aud: String, pub request: Value, pub request_hash: String, pub level: Level, pub iat: i64, pub exp: i64 }` | The signed part of an approval (`deny_unknown_fields`). |
| `Assertion` | `pub struct Assertion { pub credential_id: String, pub authenticator_data: String, pub client_data_json: String, pub signature: String }` | One WebAuthn assertion, every field base64url; `signature` is DER. |
| `ApprovalObject` | `pub struct ApprovalObject { pub claims: Value, pub signatures: Vec<Assertion> }` | The approval as it travels, before base64url. |
| `request_shape_ok` | `pub fn request_shape_ok(request: &Value) -> bool` | Exactly `{"type", "params"}`; `type` is 1 to 64 of `a-z 0-9 _ . -`; `params` is an object. |
| `id_ok` | `pub fn id_ok(id: &str) -> bool` | 22 characters of base64url decoding to 16 bytes. |
| `VERSION` | `"pka.1"` | Version in every approval. |
| `MAX_APPROVAL_BYTES` | `16 * 1024` | Longest encoded approval. |
| `MAX_REQUEST_BYTES` | `8 * 1024` | Longest canonical request. |
| `MAX_LIFETIME_SECS` | `300` | Longest `exp - iat`. |
| `DEFAULT_LIFETIME_SECS` | `60` | The gate's default approval life. |
| `MAX_CLOCK_SKEW_SECS` | `30` | How far `iat` may be in the future. |
| `STANDING_TYPE` | `"standing"` | Reserved request type for standing approvals. |
| `REVOKE_TYPE` | `"revoke"` | Reserved request type for revocations. |

## Standing approvals: `standing`

### `verify_use`

```rust
pub fn verify_use(
    standing: &str,
    use_token: &str,
    expected: &Expected,
    keyring: &Keyring,
    uses: &dyn UseStore,
    revoked: &Revocations,
    config: &Config,
    now: i64,
) -> Result<VerifiedUse, ErrorCode>
```

Checks one use of a standing approval before you act on `expected.request`. `standing` is the person's approval (sent in `Phygital-Approval`), `use_token` the short token for this action (sent in `Phygital-Use`). In order:

1. The standing approval: empty is `APPROVAL_REQUIRED`; its signed request must carry valid standing params, read first (`MALFORMED` otherwise); then every check of `verify` steps 1 to 8, with a life of up to 30 days and level `high` always (a `medium` one is `ASSURANCE_TOO_LOW`, whatever `expected.level` says).
2. Not revoked: `revoked.contains(standing id)` gives `REVOKED`.
3. The use token: empty is `APPROVAL_REQUIRED`; header exactly `{"alg":"ES256","typ":"pka-use+jwt"}` and payload `v` `pka.use.1` (`MALFORMED`); ES256 signature by `use_key` (`BAD_SIGNATURE`); same `standing_id` (`REQUEST_MISMATCH`); same audience (`WRONG_AUDIENCE`); `iat <= now + 30`, `now < exp`, `exp - iat <= 60`, `exp` not after the standing approval's `exp` (`EXPIRED`).
4. Your request: `{type, params}` shape and not of type `standing` (`MALFORMED`); exactly the token's request and `request_hash` (`REQUEST_MISMATCH`).
5. Inside the limits (`NOT_ALLOWED`).
6. `uses.record_use(standing id, use id, max_uses, standing exp, now)`: `REPLAYED` or `USED_UP` from the store.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedUse {
    pub standing: Verified,
    pub use_id: String,
    pub uses_left: u32,
}
```

### Other items in `standing`

| Item | Signature | Meaning |
|---|---|---|
| `check_standing` | `pub fn check_standing(approval: &str, expected: &Expected, keyring: &Keyring, config: &Config, now: i64) -> Result<Verified, ErrorCode>` | Every check of a standing approval except the use token and counting (for gates). `expected.request` must be the standing request. |
| `UseStore` | trait, see [below](#implementing-usestore) | Counts uses. |
| `MemoryUseStore` | `pub fn new() -> Self` | A use store in memory, for one process. |
| `Revocations` | struct, see below | Revoked standing ids, each proven by a signature. |
| `revoke_request` | `pub fn revoke_request(standing_id: &str) -> Value` | `{"type": "revoke", "params": {"standing_id": ...}}`. |
| `StandingParams` | `pub struct StandingParams { pub for_type: String, pub fixed: Map<String, Value>, pub vary: BTreeMap<String, Limit>, pub max_uses: u32, pub use_key: String }` | The params of a `standing` request. |
| `StandingParams::from_request` | `pub fn from_request(request: &Value) -> Result<StandingParams, ErrorCode>` | Read and check them: `for_type` a valid type and not `standing`, `max_uses` 1 to 1000, `fixed` and `vary` share no field, ranges have `min <= max`, `one_of` has 1 to 20 strings, `use_key` parses. Else `MALFORMED`. |
| `StandingParams::to_request` | `pub fn to_request(&self) -> Value` | The `standing` request for these params. |
| `StandingParams::allows` | `pub fn allows(&self, request: &Value) -> bool` | Type is `for_type`; exactly the fixed fields with equal canonical values and the vary fields within limits; nothing else. |
| `Limit` | `pub enum Limit { Range { min: i64, max: i64 }, OneOf { one_of: Vec<String> } }` | One varying field's limit. Both `min` and `max` are required. |
| `Use` | `pub struct Use { pub v: String, pub standing_id: String, pub id: String, pub aud: String, pub request: Value, pub request_hash: String, pub iat: i64, pub exp: i64 }` | A use token's payload. |
| `sign_use` | `pub fn sign_use(u: &Use, key: &SigningKey) -> Result<String, ErrorCode>` | Sign a use token (the gate does this). |
| `MAX_STANDING_SECS` | `30 * 24 * 3600` | Longest standing approval. |
| `MAX_USES` | `1000` | Most uses. |
| `MAX_USE_SECS` | `60` | Longest use token. |
| `MAX_ONE_OF` | `20` | Most strings in `one_of`. |
| `MAX_REVOCATIONS` | `1000` | Most entries read from one revocation list. |
| `USE_TYP` | `"pka-use+jwt"` | The use token's `typ`. |

### `Revocations`

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Revocations { /* HashSet<String> of standing ids */ }

impl Revocations {
    pub fn none() -> Self
    pub fn from_approvals(approvals: &[String], aud: &str, keyring: &Keyring, config: &Config) -> Self
    pub fn from_json(text: &str, aud: &str, keyring: &Keyring, config: &Config) -> Self
    pub fn merge(&mut self, other: Revocations)
    pub fn contains(&self, standing_id: &str) -> bool
    pub fn len(&self) -> usize
    pub fn is_empty(&self) -> bool
}
```

`from_approvals` keeps the standing id of each entry that is a valid one-time approval of exactly `revoke_request(id)`, for `aud`, level `medium` or higher, signed by a key in `keyring`, checked as of its own `iat`. Anything else is ignored, never an error. `from_json` reads `{"revocations": [...]}` (bad JSON gives an empty set). Keep every set you accepted with `merge`. Full rules: [revocations](revocations.md).

### Example: one use

```rust
use phygital_key_verify::standing::{MemoryUseStore, Revocations, verify_use};
use phygital_key_verify::{Config, ErrorCode, Expected, Keyring, Level};

fn pay(
    standing: &str,
    use_token: &str,
    to: &str,
    amount_cents: i64,
    keyring: &Keyring,
    config: &Config,
    uses: &MemoryUseStore,
    revoked: &Revocations,
    now: i64,
) -> Result<u32, ErrorCode> {
    let request = serde_json::json!({
        "type": "payment",
        "params": {"to": to, "currency": "EUR", "amount_cents": amount_cents, "memo": "coffee"}
    });
    let expected = Expected { aud: "https://pay.example.com", request: &request, level: Level::High };
    let ok = verify_use(standing, use_token, &expected, keyring, uses, revoked, config, now)?;
    // Pay now. Log ok.standing.id, ok.use_id, ok.standing.approver.
    Ok(ok.uses_left)
}
```

### Example: keeping revocations

```rust
use phygital_key_verify::standing::Revocations;

// Once, at start: load what you accepted before (from your own storage).
let mut revoked = Revocations::none();

// Every minute or so: fetch https://<gate>/.well-known/phygital-key-revoked.json
// with your HTTP client, then add what checks out. Never replace the set.
revoked.merge(Revocations::from_json(&fetched_text, "https://pay.example.com", &keyring, &config));
```

`Revocations` does not serialize. To keep accepted revocations across restarts, store the approval strings you fetched and rebuild with `from_approvals` at start, with a keyring that still holds the keys that signed them.

## Receipts: `receipt`

| Item | Signature | Meaning |
|---|---|---|
| `Receipt` | struct, below | What a receipt says. |
| `RECEIPT_TYP` | `"pka-receipt+jwt"` | The receipt's `typ`. |
| `reference_of` | `pub fn reference_of(approval: &str) -> String` | base64url SHA-256 of the exact approval string. |
| `sign` | `pub fn sign(receipt: &Receipt, key: &SigningKey) -> Result<String, ReceiptError>` | Sign with your service's key; returns a compact JWS. |
| `verify` | `pub fn verify(jws: &str, service_key: &VerifyingKey, iss: &str, approval: &str) -> Result<Receipt, ReceiptError>` | Check a receipt for a one-time approval. |
| `verify_use_receipt` | `pub fn verify_use_receipt(jws: &str, service_key: &VerifyingKey, iss: &str, standing_approval: &str, use_token: &str) -> Result<Receipt, ReceiptError>` | Check a receipt for one use of a standing approval. Does not check the use token's own signature. |
| `peek` | `pub fn peek(jws: &str) -> Option<Receipt>` | Read the payload without checking anything. For display only. |
| `ReceiptError` | `pub enum ReceiptError { Malformed, BadSignature, Mismatch }` | `Display` gives `MALFORMED`, `BAD_SIGNATURE`, `MISMATCH`. |

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub v: String,              // "pka.receipt.1"
    pub iss: String,            // your audience
    pub approval_id: String,    // claims.id (the standing id for a use)
    pub reference: String,      // reference_of(approval string)
    pub request_hash: String,   // the request you ran
    pub status: String,         // "success" or "error"
    pub error: Option<String>,  // short code, only with "error"
    pub use_id: Option<String>, // the use token's id, for standing uses
    pub iat: i64,
}
```

`SigningKey` and `VerifyingKey` are `p256::ecdsa::SigningKey` and `p256::ecdsa::VerifyingKey`. `webauthn::parse_public_key` turns a base64url SPKI key into a `VerifyingKey`.

### Example: sign a receipt

```rust
use p256::ecdsa::SigningKey;
use p256::pkcs8::EncodePublicKey;
use phygital_key_verify::receipt::{self, Receipt};
use phygital_key_verify::{Verified, b64};

/// `secret` is your service's 32-byte P-256 private key, from your secret store.
fn receipt_for(approval: &str, ok: &Verified, secret: &[u8], now: i64) -> Result<String, Box<dyn std::error::Error>> {
    let key = SigningKey::from_slice(secret).map_err(|e| e.to_string())?;
    let r = Receipt {
        v: "pka.receipt.1".into(),
        iss: "https://deploy.example.com".into(),
        approval_id: ok.id.clone(),
        reference: receipt::reference_of(approval),
        request_hash: ok.request_hash.clone(),
        status: "success".into(),
        error: None,
        use_id: None,
        iat: now,
    };
    Ok(receipt::sign(&r, &key)?)
}

/// The public half, for /.well-known/phygital-key-service.json.
fn receipt_public_key(key: &SigningKey) -> Result<String, String> {
    let der = key.verifying_key().to_public_key_der().map_err(|e| e.to_string())?;
    Ok(b64::encode(der.as_bytes()))
}
```

### Example: check a receipt

```rust
use phygital_key_verify::{receipt, webauthn};

fn receipt_ok(jws: &str, service_key_b64: &str, approval: &str) -> Result<String, Box<dyn std::error::Error>> {
    let key = webauthn::parse_public_key(service_key_b64).ok_or("not a P-256 SPKI key")?;
    let r = receipt::verify(jws, &key, "https://deploy.example.com", approval)?;
    Ok(r.status) // "success" or "error"
}
```

Format, failure cases and what the gate shows: [receipts](receipts.md).

## Key history: `history`

| Item | Signature | Meaning |
|---|---|---|
| `verify_history` | `pub fn verify_history(ring: &Keyring, root_fingerprint: &str, origin: &str) -> Result<(), HistoryError>` | Check a keyring's signed history from one pinned fingerprint, signed on `origin`. |
| `verify_history_update` | `pub fn verify_history_update(accepted: &Keyring, new: &Keyring, root_fingerprint: &str, origin: &str) -> Result<(), HistoryError>` | Accept a refetched keyring only if it extends the accepted one entry for entry, then check it all. |
| `root_fingerprint` | `pub fn root_fingerprint(ring: &Keyring) -> Option<String>` | The fingerprint of the key in the first entry. Read only: it proves nothing until `verify_history` passes. |
| `keyring_aud` | `pub fn keyring_aud(origin: &str) -> String` | `phygital-key:keyring:<origin>`, the audience of key changes. |
| `add_key_request` | `pub fn add_key_request(card: &KeyCard) -> Value` | `{"type": "add_key", "params": <card>}`. |
| `remove_key_request` | `pub fn remove_key_request(card: &KeyCard) -> Value` | `{"type": "remove_key", "params": {"credential_id", "name"}}`. |
| `HistoryError` | `pub enum HistoryError { Unsigned, WrongRoot, BadEntry(usize, &'static str), KeysDoNotMatch, WrongOrigin, NotAnUpdate }` | `code()` gives `UNSIGNED`, `WRONG_ROOT`, `BAD_ENTRY`, `KEYS_DO_NOT_MATCH`, `WRONG_ORIGIN`, `NOT_AN_UPDATE`. `BadEntry` holds the 0-based entry index and a reason. Implements `Display` and `Error`. |

### Example: refetch a published keyring

```rust
use phygital_key_verify::Keyring;
use phygital_key_verify::history::{verify_history, verify_history_update};

const PINNED: &str = "Pp0IrvTI9djhcmFJeel-FJXJd1_u_tJ2R8j8hpMh1xo"; // learned from the person
const ORIGIN: &str = "https://approve.example.com";

fn accept(accepted: Option<&Keyring>, fetched: &str) -> Result<Keyring, Box<dyn std::error::Error>> {
    let new = Keyring::from_json(fetched)?;
    match accepted {
        Some(old) => verify_history_update(old, &new, PINNED, ORIGIN)?,
        None => verify_history(&new, PINNED, ORIGIN)?,
    }
    Ok(new) // store it as the new accepted copy, then use it for verify
}
```

Every rule and every key-history test vector: [key history](key-history.md).

## Lower-level modules

These are public for gates, tools and other implementations. A service rarely needs them.

| Module | Item | Signature | Meaning |
|---|---|---|---|
| `canonical` | `CHALLENGE_PREFIX` | `&[u8] = b"phygital-key/approval/v1\0"` | Domain separation for the challenge. |
| `canonical` | `MAX_SAFE_INTEGER` | `i64 = 9_007_199_254_740_991` | 2^53 - 1. |
| `canonical` | `CanonError` | `pub enum CanonError { NotSafeInteger, NotNfc, Serialize }` | Why a value has no canonical form. |
| `canonical` | `validate` | `pub fn validate(value: &Value) -> Result<(), CanonError>` | Every string and key NFC, every number a safe integer. |
| `canonical` | `canonical_bytes` | `pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonError>` | RFC 8785 after `validate`. |
| `canonical` | `canonical_string` | `pub fn canonical_string(value: &Value) -> Result<String, CanonError>` | The same as text. |
| `canonical` | `hash_b64` | `pub fn hash_b64(value: &Value) -> Result<String, CanonError>` | base64url SHA-256 of the canonical bytes: `request_hash`. |
| `canonical` | `challenge` | `pub fn challenge(claims: &Value) -> Result<[u8; 32], CanonError>` | The WebAuthn challenge for a set of claims. |
| `strict` | `MAX_DEPTH` | `usize = 16` | Deepest nesting. |
| `strict` | `parse` | `pub fn parse(bytes: &[u8]) -> Option<Value>` | JSON that refuses duplicate keys, lone surrogates, trailing data and deep nesting. |
| `strict` | `depth` | `pub fn depth(bytes: &[u8]) -> usize` | Nesting depth, counted without parsing. |
| `b64` | `encode` | `pub fn encode(bytes: impl AsRef<[u8]>) -> String` | base64url, no padding. |
| `b64` | `decode` | `pub fn decode(text: &str) -> Option<Vec<u8>>` | Refuses padding, other alphabets, non-canonical trailing bits. |
| `webauthn` | `FLAG_UP`, `FLAG_UV` | `u8 = 0x01`, `u8 = 0x04` | Authenticator data flags. |
| `webauthn` | `parse_public_key` | `pub fn parse_public_key(spki_b64: &str) -> Option<VerifyingKey>` | Parse a base64url SPKI DER P-256 key. |
| `webauthn` | `AssertionCheck` | `pub struct AssertionCheck<'a> { pub kind: &'a str, pub challenge: &'a [u8], pub origins: &'a [String], pub rp_id: &'a str }` | What an assertion must match. |
| `webauthn` | `check_client_data` | `pub fn check_client_data(client_data_json: &[u8], want: &AssertionCheck) -> bool` | Type, challenge, origin, not cross-origin. |
| `webauthn` | `verify_assertion` | `pub fn verify_assertion(public_key: &VerifyingKey, authenticator_data: &[u8], client_data_json: &[u8], signature_der: &[u8], want: &AssertionCheck) -> Result<u8, AssertionError>` | Check one ES256 assertion; returns the flags. |
| `webauthn` | `AssertionError` | `pub enum AssertionError { BadSignature, AssuranceTooLow }` | Why an assertion failed. |

## Implementing `SeenStore`

```rust
pub trait SeenStore {
    fn insert_if_new(&self, id: &str, exp: i64, now: i64) -> bool;
}
```

Your implementation must:

- record `id` and answer in one atomic step (an `INSERT` on a unique key, Redis `SET NX`), never "read, then write";
- be shared by every instance that acts on approvals;
- keep the id at least until `exp + 30` (`MAX_CLOCK_SKEW_SECS`);
- return `false` on any error.

The method takes `&self`, so the store needs interior mutability (a mutex, a connection pool). With a blocking database client, implement it directly:

```rust
// Sketch: `Db` stands for your blocking database client.
struct SqlSeenStore { db: Db }

impl phygital_key_verify::SeenStore for SqlSeenStore {
    fn insert_if_new(&self, id: &str, exp: i64, _now: i64) -> bool {
        // INSERT INTO pka_seen (id, keep_until) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING
        match self.db.execute_insert(id, exp + 30) {
            Ok(rows) => rows == 1,
            Err(_) => false, // fail closed
        }
    }
}
```

With an async client, do not block inside the trait. Let `verify` run with a store that only captures the id, then spend it yourself. Replay is the last check, so nothing else runs after the capture.

```rust
use std::sync::Mutex;
use phygital_key_verify::{ErrorCode, Expected, Keyring, Config, SeenStore, Verified, verify};

/// Records the id instead of deciding. You spend it right after `verify`.
#[derive(Default)]
struct Capture(Mutex<Option<(String, i64)>>);

impl SeenStore for Capture {
    fn insert_if_new(&self, id: &str, exp: i64, _now: i64) -> bool {
        match self.0.lock() {
            Ok(mut slot) => {
                *slot = Some((id.to_owned(), exp));
                true
            }
            Err(_) => false,
        }
    }
}

async fn verify_once(
    approval: &str, expected: &Expected<'_>, keyring: &Keyring, config: &Config, now: i64,
) -> Result<Verified, ErrorCode> {
    let capture = Capture::default();
    let ok = verify(approval, expected, keyring, &capture, config, now)?;
    let spent = capture.0.lock().ok().and_then(|mut s| s.take());
    let Some((id, exp)) = spent else { return Err(ErrorCode::Replayed) };
    // Sketch: your async atomic insert, true when the row is new.
    if !db_insert_seen(&id, exp + 30).await { return Err(ErrorCode::Replayed) }
    Ok(ok)
}
```

`db_insert_seen` is yours to write (SQL or Redis, as in [integrate](integrate.md#the-seen-store)), and must return `false` on errors.

## Implementing `UseStore`

```rust
pub trait UseStore {
    fn record_use(
        &self,
        standing_id: &str,
        use_id: &str,
        max_uses: u32,
        until: i64,
        now: i64,
    ) -> Result<u32, ErrorCode>;
}
```

| Return | When |
|---|---|
| `Ok(uses_left)` | The use was new and within `max_uses`. `uses_left` is `max_uses` minus the uses recorded so far, this one included. |
| `Err(ErrorCode::Replayed)` | This `use_id` was recorded before for this `standing_id`. |
| `Err(ErrorCode::UsedUp)` | `max_uses` uses are already recorded. |

Keep entries until `until` (the standing approval's `exp`) plus 30 seconds. Make the check and the insert one atomic step, shared by every instance and durable: this store is the only thing that enforces `max_uses` at your service. On a storage error return an `Err` (the built-in `MemoryUseStore` returns `UsedUp` when its mutex is poisoned) and log the real cause yourself. The capture pattern above works the same way for async storage: `verify_use` calls `record_use` last.

## Errors

| Function | Error type | Values |
|---|---|---|
| `verify`, `check`, `Verifier::verify`, `verify_use`, `check_standing` | `ErrorCode` | The 13 codes in [error codes](error-codes.md) (`DENIED` is never returned by this crate). |
| `history::verify_history`, `verify_history_update` | `HistoryError` | `UNSIGNED`, `WRONG_ORIGIN`, `WRONG_ROOT`, `BAD_ENTRY`, `KEYS_DO_NOT_MATCH`, `NOT_AN_UPDATE` |
| `receipt::verify`, `verify_use_receipt`, `sign` | `ReceiptError` | `MALFORMED`, `BAD_SIGNATURE`, `MISMATCH` |
| `Keyring::from_json` | `String` | A readable reason, for example `keyring: bad public key for <name>` |
| `Revocations::from_approvals`, `from_json` | none | Invalid entries are skipped. |

Bad input gives an error value, not a panic: the workspace lints deny `unwrap`, `expect`, `panic!` and unchecked indexing (`clippy::unwrap_used`, `expect_used`, `panic`, `indexing_slicing`), a property test feeds random requests to `verify`, and the fuzz targets run in CI.

## Threads

`MemorySeenStore` and `MemoryUseStore` keep their data behind a `std::sync::Mutex`, so they can be shared between threads (for example in an `Arc`). `Verifier<MemorySeenStore>` holds only owned data and that store, so one `Verifier` can serve many threads through `&self`. `set_keyring` takes `&mut self`: to swap keyrings while serving, keep the `Verifier` (or the `Keyring`) behind your own `RwLock`, or build a new one and swap an `Arc`.

## How it is tested

| Test | What it does |
|---|---|
| `tests/vectors.rs` | Runs every shared vector: 55 approvals (`test-vectors/vectors.json`), 25 key histories (`keyrings.json`), 43 standing approvals (`standing.json`); checks that every refusal code except `DENIED`, `NOT_ALLOWED`, `USED_UP` and `REVOKED` has an approval vector, and that a valid approval works once. |
| `tests/props.rs` | 6 property tests, 2000 random cases each: random text and random bytes never verify, flipping any bit of a valid approval is refused, any other request is refused, canonical form is idempotent, random requests never panic. |
| `tests/units.rs` | 17 unit tests: known answers for `request_hash` and the receipt reference, wire names of levels and codes, keyring parsing, request shapes, ids, nesting depth, `set_keyring`, standing params rules, use tokens and per-use receipts. |
| Module tests | Canonical form (7, including RFC 8785 key order and escapes), strict JSON (3), receipts (1, every refusal). |
| `fuzz/` | Fuzz targets for `verify`, the strict parser, key history and standing approvals, run in CI. |

The JavaScript verifier shares no code with this crate and passes the same vectors: [verifier-js](verifier-js.md). How vectors are built: [test vectors](../test-vectors/README.md).

## Next

- [Integrate](integrate.md)
- [Error codes](error-codes.md)
- [Receipts](receipts.md)
- [Key history](key-history.md)
