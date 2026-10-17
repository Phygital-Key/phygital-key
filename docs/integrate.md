# Check approvals in your service

Your service is the **verifier**: it checks the person's signed approval before it acts. The check is offline and pure. It fails closed.

## What you need

| Thing | Where it comes from |
|---|---|
| Your **audience id** (`aud`) | You choose it: usually your URL, e.g. `https://deploy.example.com`. Agents must ask for approvals with this exact value. |
| The **keyring** (trusted public keys) | `phygital-key keys` on the gate, or the gate's `/.well-known/phygital-key.json`. Check it first (see [keys](#trusting-keys)). |
| The **config** | The gate's approval page origin and its host: `origins: ["https://approve.example.com"]`, `rp_ids: ["approve.example.com"]` (for a local gate: `http://localhost:7468` and `localhost`). |
| A **seen store** | Remembers used approval ids until they expire. In memory for one process; a shared, durable store (database, Redis `SET NX`) when you run several instances. |
| The **level** each action needs | You decide, not the agent: `medium` (touch) or `high` (fingerprint, face or PIN). |

The request you check is the one **you are about to run**, built from your own data, as `{"type": "<action>", "params": {...}}`. Never take it from the approval itself.

## Rust

```toml
[dependencies]
phygital-key-verify = { git = "https://github.com/s17sh70x2jfl031ag/phygital-key-mcp" }
serde_json = "1"
```

```rust
use phygital_key_verify::{Config, Keyring, Level, Verifier};

let verifier = Verifier::new(
    "https://deploy.example.com",
    Keyring::from_json(&std::fs::read_to_string("keyring.json")?)?,
    Config { origins: vec!["https://approve.example.com".into()], rp_ids: vec!["approve.example.com".into()] },
);
let request = serde_json::json!({"type": "deploy", "params": {"service": "api", "env": "production", "version": "2.4.1"}});
match verifier.verify(&approval, &request, Level::High) {
    Ok(ok) => { /* deploy, then log ok.id and ok.approver */ }
    Err(code) => { /* refuse: code is e.g. REQUEST_MISMATCH */ }
}
```

For full control (your own clock and seen store) use `phygital_key_verify::verify(approval, &Expected { aud, request, level }, &keyring, &seen, &config, now)`.

## JavaScript (Node 20+ or browser)

The independent JavaScript checker lives in [`verifier-js/`](../verifier-js). No dependencies.

```js
import { verify, MemorySeenStore, keyringFrom } from "phygital-key-verify";

const result = await verify(
  approval,
  { aud: "https://deploy.example.com", request, level: "high" },
  keyringFrom(JSON.parse(keyringJson)),
  seen,                                   // new MemorySeenStore(), or your own with insertIfNew(id, exp, now)
  { origins: ["https://approve.example.com"], rp_ids: ["approve.example.com"] },
  Math.floor(Date.now() / 1000),
);
if (!result.ok) refuse(result.error);
```

## Any language: the command line

```bash
phygital-key --origin https://approve.example.com verify \
  --approval "$APPROVAL" \
  --request '{"type":"deploy","params":{"service":"api","env":"production","version":"2.4.1"}}' \
  --aud https://deploy.example.com --level high --keyring keyring.json
```

Exit code 0 and `{"ok": true, ...}` means go; exit code 1 and `{"ok": false, "error": "..."}` means no. Used approvals are remembered in `seen-ids.jsonl` (locked, crash-safe), so each works once on this machine.

## Error codes

| Code | Meaning |
|---|---|
| `APPROVAL_REQUIRED` | No approval was given |
| `MALFORMED` | Bad encoding, unknown fields, floats, too big or too deep, wrong kind of approval |
| `WRONG_AUDIENCE` | Approved for another service |
| `EXPIRED` | Too old, not valid yet, or lives too long |
| `REQUEST_MISMATCH` | The person approved a different request |
| `BAD_SIGNATURE` | The signature, origin or challenge does not check out |
| `UNTRUSTED_KEY` | Signed by a key that is not in your keyring |
| `ASSURANCE_TOO_LOW` | Only a touch, but you need fingerprint, face or PIN |
| `REPLAYED` | Already used |
| `NOT_ALLOWED` | Outside a standing approval's limits |
| `USED_UP` | A standing approval has no uses left |
| `REVOKED` | The person revoked this standing approval |

## Standing approvals

The agent sends two headers: `Phygital-Approval` (the person's standing approval) and `Phygital-Use` (a one-minute token for this one request).

```rust
use phygital_key_verify::standing::{MemoryUseStore, Revocations, verify_use};

let uses = MemoryUseStore::new();             // use a durable, shared store in production (trait UseStore)
let mut revoked = Revocations::none();
// Now and then: fetch https://<gate>/.well-known/phygital-key-revoked.json and keep what you accept.
revoked.merge(Revocations::from_json(&fetched, aud, &keyring, &config));

let expected = Expected { aud, request: &request, level: Level::High };
let ok = verify_use(&standing, &use_token, &expected, &keyring, &uses, &revoked, &config, now)?;
```

JavaScript: `verifyUse(standing, useToken, expected, keyring, uses, revokedSet, config, now)` and `revocationsFrom(list, aud, keyring, config)`.

Rules for services:

- **Count uses yourself, durably and shared** across instances.
- **Keep every revocation you accepted.** Whoever serves the list could leave entries out later.
- **Refresh the revocation list often** (every minute is good); use tokens already handed out can still be used until you do.

## Receipts (optional)

After acting, answer with a signed receipt so the person sees "done" on their approval page:

```rust
use phygital_key_verify::receipt::{Receipt, reference_of, sign};

let receipt = Receipt {
    v: "pka.receipt.1".into(), iss: aud.into(), approval_id: ok.id.clone(),
    reference: reference_of(&approval), request_hash: ok.request_hash.clone(),
    status: "success".into(), error: None, use_id: None, iat: now,
};
let jws = sign(&receipt, &service_signing_key)?;   // your service's own P-256 key
```

Publish your receipt public key at `/.well-known/phygital-key-service.json` (`{"iss": ..., "receipt_public_key": ...}`). The person pins it on their gate with `phygital-key trust-service --aud <aud> --key <key>`. For a standing approval, set `use_id` to the use token's id and `request_hash` to that use's request.

## Trusting keys

A keyring fetched from a gate carries a signed history of every key change. Check it against the person's fingerprint (shown on their gate's home page), and keep the copy you accepted:

```bash
phygital-key keys --check <fingerprint> --file new.json --signed-on https://approve.example.com --previous accepted.json
```

In code: `phygital_key_verify::history::verify_history_update(&accepted, &new, fingerprint, origin)` (JavaScript: `verifyHistoryUpdate`).

## Checklist

- [ ] Build the request from your own data, not from the approval.
- [ ] Set the level per action yourself.
- [ ] Use a shared, durable seen store (and use store for standing approvals).
- [ ] Check the keyring's history against the person's fingerprint; keep the last accepted copy.
- [ ] Refresh and keep revocations if you accept standing approvals.
- [ ] Log `id`, `request_hash`, the approver and the result.
