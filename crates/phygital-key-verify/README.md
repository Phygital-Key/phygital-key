# phygital-key-verify

Check a [Phygital Key](../../README.md) approval in Rust: a person's signed yes, made with a passkey, fingerprint, face or hardware key, for one exact request. A service calls it before it acts, and acts only on `Ok`.

The check is pure and offline: no network, no file access, no global state, and no clock unless you use the `Verifier` helper. It fails closed: anything missing, expired, reused, changed or unclear is a refusal with one error code. Approvals are signed and verified (ES256 over P-256, SHA-256, RFC 8785 canonical JSON), not encrypted.

Pre-audit: no outside security audit has been done yet.

## Install

```toml
[dependencies]
phygital-key-verify = { git = "https://github.com/s17sh70x2jfl031ag/phygital-key-mcp", tag = "v0.3.0" }
serde_json = "1"
```

Rust 1.89 or later, edition 2024. `#![forbid(unsafe_code)]`. Licence Apache-2.0.

## Example

```rust
use phygital_key_verify::{Config, Keyring, Level, Verifier};

let keyring = Keyring::from_json(&std::fs::read_to_string("keyring.json")?)?; // from `phygital-key keys`
let config = Config { origins: vec!["https://approve.example.com".into()], rp_ids: vec!["approve.example.com".into()] };
let verifier = Verifier::new("https://deploy.example.com", keyring, config);

// The request you are about to run, built from your own data.
let request = serde_json::json!({"type": "deploy", "params": {"service": "api", "env": "production", "version": "2.4.1"}});
let ok = verifier.verify(&approval, &request, Level::High)?; // Err(ErrorCode) on any doubt
println!("approved by {} (id {})", ok.approver, ok.id);     // ...now deploy
```

`Verifier::new` keeps used ids in memory for one process. With several instances, pass a shared, durable store to `Verifier::with_store`, or call `verify(approval, &expected, &keyring, &seen, &config, now)` with your own `SeenStore` and clock.

`examples/basic.rs` runs the first shared test vector twice and shows the second use refused (`cargo run -p phygital-key-verify --example basic` in a clone).

## What it checks

In order, first failure wins: well formed (`APPROVAL_REQUIRED`, `MALFORMED`), audience (`WRONG_AUDIENCE`), time (`EXPIRED`), the exact request (`REQUEST_MISMATCH`), level (`ASSURANCE_TOO_LOW`), trusted key (`UNTRUSTED_KEY`), the WebAuthn assertion (`BAD_SIGNATURE`), user presence and verification (`ASSURANCE_TOO_LOW`), single use (`REPLAYED`).

## What else is in the crate

| Module | For |
|---|---|
| root | `verify`, `check`, `Verifier`, `Expected`, `Config`, `Keyring`, `KeyCard`, `Level`, `Verified`, `ErrorCode`, `SeenStore`, `MemorySeenStore` |
| `standing` | Standing approvals: `verify_use` with use tokens, a `UseStore` that counts uses, `Revocations` read from the gate's signed list (`NOT_ALLOWED`, `USED_UP`, `REVOKED`) |
| `receipt` | Signed receipts from services: `sign`, `verify`, `verify_use_receipt` |
| `history` | Signed key history: check a published keyring from one pinned fingerprint, and refuse rollbacks on refetch |
| `canonical`, `strict`, `b64`, `webauthn` | Canonical form and `request_hash`, strict JSON, base64url, one ES256 assertion |

## Tested

It passes all 123 shared test vectors in `test-vectors/` (55 approvals, 25 key histories, 43 standing approvals), the same ones the independent [JavaScript verifier](../../verifier-js/README.md) passes, plus property tests (6 functions, 2000 cases each), unit tests and fuzz targets.

## Read more

- [Rust verifier reference](../../docs/verifier-rust.md): every public item with its signature, and examples
- [Integrate](../../docs/integrate.md): how a service uses it (seen stores, levels, standing approvals)
- [Error codes](../../docs/error-codes.md)
- [Spec](../../docs/spec.md) and [test vectors](../../test-vectors/README.md)
