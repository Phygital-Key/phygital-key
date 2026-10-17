# Phygital Key Approval (PKA), spec (format `pka.1`, project v0.3)

**Goal.** A small open base for one thing: a signed yes from a person, for one exact request, that anyone can check. The person approves with a passkey, fingerprint, face or hardware key. Any party can verify the yes offline in a few lines of code. It works between any two sides: person to person, person to business, business to business, person to system, AI agent to service. Built on WebAuthn (FIDO2) and aligned with AP2 (now at the FIDO Alliance). License: Apache-2.0.

**Design goals, in order:** safe, private, simple to use, simple to add, runs anywhere, self-hosted by default.

The words MUST, MUST NOT and SHOULD are used as in RFC 2119. The test vectors in `test-vectors/vectors.json` are part of this spec: every implementation MUST pass every vector.

## 1. Roles

| Role | What it is | Must |
|---|---|---|
| Requester | Anyone or anything that asks for a yes: an AI agent, an app, a business, a person | Build the request. Never holds the approver's key. Never approves. |
| Relay | Passes the request to the approver and the approval back. Today this is the Gate, an MCP server | Canonicalize and hash the request, show it on the approval page, keep state. No LLM inside. Can never create a yes by itself. |
| Approver | A person, their authenticator (phone, laptop, security key) and the approval page (AP2 "Trusted Surface") | See the exact request, decide, sign with their own key. The key never leaves their device. |
| Verifier | Anyone who acts on the yes: a service, a business, another person's app, a machine | Check the approval with the verifier before acting. Never trust "they said yes" from the requester. |

One party can hold several roles. Example: a business can be both Requester and Verifier.

## 2. Adapters (how a request reaches the approver)

The base is the format and the verifier. Adapters are the doorways.

- **MCP (the only adapter today).** The Gate is an MCP server (stdio, or Streamable HTTP at `/mcp`). It asks the client to open the approval page with MCP URL-mode elicitation. If the client has no URL elicitation, it returns the link for the person to open. It never auto-approves. Over HTTP, `/mcp` needs a bearer token (`Authorization: Bearer <token>`, made once and kept in the gate's data folder; `phygital-key mcp-token` prints it) and refuses any request that carries a browser `Origin`. `--open-mcp` turns the token off. stdio needs no token: only the app that started the gate can talk to it.
- **Later:** plain HTTP API, email link, chat apps, QR code on a screen, SDKs for apps.

All adapters produce the same approval object and use the same verifier.

## 3. Request and approval object

### Request

A request is exactly `{"type": "<name>", "params": {...}}`:

- `type`: 1 to 64 characters from `a-z 0-9 _ . -`.
- `params`: a JSON object. Strings, integers, booleans, null, arrays and objects. **No floats**: amounts are strings (`"120.00"`) or integers in minor units (`12000`). Integers MUST be within ±(2^53 - 1).
- No secrets in `params`. They are shown on screen, stored by the relay and hashed.
- Canonical form at most 8192 bytes.

### Approval object

JSON, carried as base64url (no padding) in an `approval` field or a `Phygital-Approval` header.

```json
{
  "claims": {
    "v": "pka.1",
    "id": "<128-bit random, base64url, 22 characters>",
    "aud": "https://deploy.example.com",
    "request": { "type": "deploy", "params": { "service": "api", "env": "production", "version": "2.4.1" } },
    "request_hash": "<base64url SHA-256 of canonical(request)>",
    "level": "high",
    "iat": 1760700000,
    "exp": 1760700060
  },
  "signatures": [
    {
      "credential_id": "<base64url>",
      "authenticator_data": "<base64url>",
      "client_data_json": "<base64url>",
      "signature": "<base64url DER ECDSA P-256>"
    }
  ]
}
```

- **`aud`** names who will act on the yes. Any stable identifier: a URL, a domain, a business id or a person's key id. 1 to 256 visible ASCII characters (`!` to `~`), so look-alikes and invisible characters cannot hide in it.
- **`level`** is `medium` or `high`. `low` means no approval is needed, so it is never signed.
- **`signatures`** is a list. Format `pka.1` requires exactly one. The list makes "2 of 3 approvers" possible later without changing the format.
- Unknown fields anywhere in the approval object, the claims or a signature MUST be refused.
- Duplicate JSON keys MUST be refused. Nesting deeper than 16 MUST be refused. The encoded approval MUST be at most 16384 bytes.

### Canonical form

`canonical(x)` = RFC 8785 (JCS) of `x`, where every string and key MUST already be in Unicode NFC. Text that is not NFC MUST be refused, never rewritten: NFC folds some single characters into others (KELVIN SIGN U+212A becomes `K`, GREEK QUESTION MARK U+037E becomes `;`), so rewriting would let two different byte strings share one approval. Floats and unsafe integers have no canonical form and MUST be refused. Lone surrogates MUST be refused. A verifier acts on exactly the bytes it verified.

### Challenge

`challenge = SHA-256("phygital-key/approval/v1" || 0x00 || canonical(claims))`

A verifier's `verify` MUST refuse a request of type `revoke` (`MALFORMED`): a revocation is never an action. Implementations MUST refuse a UTF-8 byte-order mark before the JSON (one approval, one encoding).

The approver's device signs this through WebAuthn, so the signature covers the exact request, audience, level and expiry. Algorithm: ES256 (ECDSA P-256 with SHA-256) is required.

## 4. Timing

Two clocks, so people are not rushed:

- **Pending request:** waits up to 10 minutes on the relay for the person to open the page and decide.
- **Approval:** `iat` and `exp` are set when the person presses Approve, right before the key prompt. Default life 60 s, max 300 s. Single use.

## 5. Keys: enrolling and finding them

- **Enroll:** the person registers a passkey or security key on the approval page. The page sends the credential id and the public key (SubjectPublicKeyInfo, from `getPublicKey()`), and the relay stores a public **key card**. Attestation is `none` (what phones and password managers give), so the relay cannot prove the key lives in hardware: the one-time link is what protects the first key.

  ```json
  { "credential_id": "...", "public_key": "<base64url SPKI DER>", "rp_id": "approve.example.com", "name": "My phone", "created": "2026-10-17" }
  ```

- **Enrollment links** are made by the person in their own terminal (`phygital-key enroll`). The gate never prints a token in `mcp` mode, where its output goes to the AI app's logs. The token travels in the link's `#fragment`, which browsers never send to servers. It is used up when the person starts enrolling and the session lasts at most 10 minutes.
- **Every key change is an approval.** Adding a key is a request `{"type": "add_key", "params": <key card>}`; removing one is `{"type": "remove_key", "params": {"credential_id", "name"}}`. Both are level `high`, with audience `phygital-key:keyring:<origin>`, so a change approved on one gate never counts on another.
  - The **first key** signs its own `add_key` (it proves it works, and starts the history).
  - Every **later change** must be signed by a key that is trusted at that point. A new key can never approve itself. The last key cannot be removed.
- **Signed key history.** The keyring keeps every key-change approval, oldest first, in `history`, plus the `origin` they were signed on:

  ```json
  { "keys": [ ...key cards ], "origin": "https://approve.example.com", "history": [ "<approval>", "<approval>" ] }
  ```

  **Checking a keyring** (`verify_history(keyring, root_fingerprint, origin)`): `origin` is given by the checker (never taken from the file) and MUST equal the keyring's; the first entry MUST be `add_key` of the key whose fingerprint (base64url SHA-256 of its SPKI) is pinned, signed by itself; each later entry MUST be a valid `add_key` or `remove_key` approval signed by a key trusted at that point (every verifier check except expiry, which is checked as of the entry's own `iat`); `iat` MUST never go back; no entry may appear twice; and replaying the history MUST give exactly the listed key cards (every field). Anything else is refused (`UNSIGNED`, `WRONG_ORIGIN`, `WRONG_ROOT`, `BAD_ENTRY`, `KEYS_DO_NOT_MATCH`). Test vectors: `test-vectors/keyrings.json`.

  **Refetching** (`verify_history_update(accepted, new, root_fingerprint, origin)`): a valid history proves every listed key was approved, but an *old* copy (from before a key was removed) is valid too. Whoever fetches a keyring more than once MUST keep the last one they accepted and take a new one only if it starts with exactly the same entries (`NOT_AN_UPDATE` otherwise). That stops rollbacks and forks signed by a removed key.

  So a keyring can be trusted with one short fingerprint, learned once (in person, on a call, on a business card), plus the last copy you accepted. Whoever controls the website or the file cannot add a key without a signature from a key that was trusted at that point, and cannot bring back a removed key.
- **Keyring:** a verifier keeps the key cards it trusts: `{"keys": [ ...key cards ]}`. The verifier only reads the keyring it is given. It never fetches anything.
- **Sharing keys:**
  - Inside one team or system: copy the keyring file, or run `phygital-key keys`.
  - Between parties: a party publishes its keyring (cards plus signed history) at `https://<domain>/.well-known/phygital-key.json`. The receiving app (not the verifier) fetches it over HTTPS, checks the history against the pinned root fingerprint and its expected origin, and stores it. Later fetches go through `verify_history_update` against the stored copy.
- **Lost or retired key:** remove it with a `remove_key` approval from another trusted key. People SHOULD enroll two keys so one loss does not lock them out. If every key is lost, start a new keyring with a new root fingerprint.
- **Synced passkeys:** `signCount` is not reliable, so it is never used. The single-use `id` stops replays.

## 6. Flow

1. Requester asks the relay: `request_approval(request, aud, level)`.
2. Relay canonicalizes and hashes the request, stores it as `pending`, and sends the approver to the approval page. In MCP, "Accept" only means the person opened the page. It is not a yes.
3. The page shows the request as built by the relay from the canonical form, never text written by the requester. Every field is shown, nothing truncated, plus the audience, the level and the time left. Keys that are not plain words are quoted. Common invisible, direction-changing and mixed-script characters, and odd spacing, are highlighted (a help, not a guarantee: the real protection is that the exact bytes are signed and checked).
4. The person presses Approve or Deny. On Approve, the relay fixes the claims (`iat`, `exp`) and returns their canonical text. The page checks that these claims say exactly what it shows, computes the challenge itself from that text, and asks the authenticator to sign. On a desktop without a key, the browser offers a QR code to approve with a phone (built into passkeys).
5. The relay runs every verifier check (except replay) on the signature before it keeps the approval. A Deny is recorded and returned as a refusal.
6. Requester fetches the result with `get_approval(id)` and sends the approval with the request.
7. Verifier runs `verify` and acts only on success.

## 7. Verifier checks, in order, fail closed

The first failing check returns its code.

1. **Present and well formed** (`APPROVAL_REQUIRED` if empty, else `MALFORMED`): size, base64url, strict JSON, no unknown fields, `v` is `pka.1`, `id` is 128-bit base64url, request shape, canonical form exists, exactly one signature, all fields decode.
2. **Audience** (`WRONG_AUDIENCE`): `aud` equals the verifier's own id.
3. **Time** (`EXPIRED`): `iat` at most 30 s in the future, `now < exp`, `iat < exp`, `exp - iat <= 300`.
4. **Request** (`REQUEST_MISMATCH`): SHA-256 of canonical(the request being acted on) equals `request_hash`, and `request_hash` matches `claims.request`.
5. **Level** (`ASSURANCE_TOO_LOW`): `claims.level` is at least the level the verifier requires for this request. The verifier decides the level, never the requester.
6. **Key** (`UNTRUSTED_KEY`): `credential_id` is in the keyring, its `rp_id` is allowed, its public key parses.
7. **Assertion** (`BAD_SIGNATURE`): client data `type` is `webauthn.get`, `challenge` equals the computed challenge, `origin` is allowed, not cross-origin (`crossOrigin` not true, no `topOrigin`); authenticator data RP ID hash equals SHA-256 of the key's `rp_id`; the ES256 signature over `authenticator_data || SHA-256(client_data_json)` is valid.
8. **Presence** (`ASSURANCE_TOO_LOW`): user present flag set; user verified flag set when the level is `high`.
9. **Replay** (`REPLAYED`): atomically record `id` until `exp`; refuse if already seen. Several verifier instances MUST share one store.
10. Only then act. Log `id`, `request_hash`, time, the approver's credential and the result.

**Error codes:** `APPROVAL_REQUIRED`, `MALFORMED`, `WRONG_AUDIENCE`, `EXPIRED`, `REQUEST_MISMATCH`, `BAD_SIGNATURE`, `UNTRUSTED_KEY`, `ASSURANCE_TOO_LOW`, `REPLAYED`, `NOT_ALLOWED`, `USED_UP` and `REVOKED` (standing approvals, section 7c), and `DENIED` (returned by relays when the person says no; never by `verify`).

**API:** `verify(approval, expected{aud, request, level}, keyring, seen_store, config{origins, rp_ids}, now) -> ok | error(code)`. The verifier is pure: no network, no clock reads, no global state except `seen_store`.

## 7b. Receipts

After acting, a verifier MAY answer with a **receipt**: a compact JWS signed with the verifier's own ES256 key (a service is not a person, so a normal key is fine), header exactly `{"alg":"ES256","typ":"pka-receipt+jwt"}`, payload:

```json
{ "v": "pka.receipt.1", "iss": "<verifier aud>", "approval_id": "<claims.id>",
  "reference": "<base64url SHA-256 of the exact approval string>",
  "request_hash": "<claims.request_hash>", "status": "success", "iat": 1760700005 }
```

`status` is `success` or `error` (then `error` may hold a short code). A receipt for one use of a standing approval also carries `use_id`. Unknown fields are refused. Checking a receipt (`receipt::verify`) needs the service's public key, its `iss`, and the approval it is about. Like AP2 receipts, it points at the approval by hash. A service publishes its receipt key at `/.well-known/phygital-key-service.json` (`{"iss", "receipt_public_key"}`). The header MUST be exactly the canonical bytes above, `success` MUST NOT carry `error`, and `approval_id`, `request_hash` and `iss` MUST match the approval's own claims.

The agent carries receipts back to the gate (`record_receipt`), so the agent could make one up. The gate therefore records a receipt **only if it is signed by a receipt key pinned for that service** (`phygital-key trust-service --aud ... --key ...`, kept in `services.json`; the demo pins its own service). Only then does the person see "the service reported: done". Anyone holding the service key can check a receipt too (`phygital-key receipt-check`).

## 7c. Standing approvals

A standing approval lets an agent do the same kind of action a few times, within limits, without asking each time. It is deliberately narrow (AP2's "open mandate" idea, without a policy language).

1. **The person approves**, at level `high`, a request of type `standing` (reserved: never an action):

   ```json
   {"type": "standing", "params": {
      "for_type": "payment",
      "fixed": {"to": "ACME Ltd", "currency": "EUR"},
      "vary": {"amount_cents": {"min": 1, "max": 2000}, "memo": {"one_of": ["coffee", "lunch"]}},
      "max_uses": 5,
      "use_key": "<base64url SPKI of a P-256 key made by the gate for this approval>"}}
   ```

   Its `exp` is when the permission ends: at most 30 days after `iat`. `max_uses` is 1 to 1000; `fixed` and `vary` must not share a field; integer limits need **both** `min` and `max` (so a forgotten lower limit can never allow negative amounts), with `min` not above `max`; `one_of` holds 1 to 20 strings. Standing approvals are always level `high`, whatever the service asks. A standing approval never passes as a one-time approval, and a one-time approval never passes as a standing one; a gate refuses a `standing` request made any other way than as a standing approval.
2. **Each use** is a use token: an ES256 compact JWS, header exactly `{"alg":"ES256","typ":"pka-use+jwt"}`, payload `{"v":"pka.use.1","standing_id","id","aud","request","request_hash","iat","exp"}`, signed with `use_key`, living at most 60 s and never past the standing approval's `exp`. The gate keeps `use_key` in memory only and signs a token only for a request inside the limits.
3. **The verifier** (`verify_use(standing, use_token, expected, keyring, use_store, revocations, config, now)`) checks the person's standing approval (all the checks of section 7, with the 30-day life), that it is not revoked (`REVOKED`), the use token (header exactly the canonical bytes above) (signature by `use_key`, same `standing_id`, same audience, time), that the request it is about to run is exactly the one in the token, that it has exactly the `fixed` fields with the same values and the `vary` fields within their limits and nothing else (`NOT_ALLOWED`), and counts uses itself: a use id seen before is `REPLAYED`, more than `max_uses` is `USED_UP`.

**Revoking** has two steps.

1. On the gate, right away: the person presses Revoke on the approval page (or the agent calls `cancel_approval`). The gate throws `use_key` away, so no new use tokens can be made. A restart of the gate also ends every standing approval it holds (fail closed).
2. At the service: the gate then asks the person to sign a **revocation**, a one-time approval (level `medium` or higher) of the reserved request `{"type": "revoke", "params": {"standing_id": "<id>"}}` with the standing approval's audience. Signed revocations are published at `/.well-known/phygital-key-revoked.json` as `{"revocations": ["<approval>", ...]}`. A service fetches that list now and then (it is data; the verifier never fetches) and builds `Revocations::from_approvals(list, aud, keyring, config)`: only revocations signed by a trusted key, for this service, about a standing id, count, each checked as of its own `iat` (a revocation never expires). Anything else in the list is ignored, which is safe because an entry can only ever add a revocation. `verify_use` then refuses that standing approval with `REVOKED`, including use tokens handed out before the revoke.

Until the service picks up the signed revocation, use tokens already handed out still work for up to 60 seconds (and an agent could have collected every remaining use right before the revoke). Services MUST count uses in a durable store shared by all their instances, and keep entries a little past expiry (so a clock that steps back cannot reset the count).

**Receipts for standing uses** carry the use token's `use_id`, and `request_hash` is that use's request. They are checked with `receipt::verify_use_receipt` (the gate records one per use token it handed out).

Test vectors: `test-vectors/standing.json`.

## 8. Levels

| Level | Examples | Requirement |
|---|---|---|
| low | Read files | No approval |
| medium | Edit code, send email, sign a small order | WebAuthn with user presence (touch) |
| high | Deploy to production, delete data, move money, sign a contract, add a key | WebAuthn with user verification (biometric or PIN) |

A security key without a PIN can only approve `medium`.

## 9. Privacy and encryption

- **Signed, not secret.** The approval proves who said yes to what. Anyone holding it can read the request, so keep requests to what the verifier needs.
- **In transit:** HTTPS everywhere outside localhost. Passkeys refuse to work otherwise.
- **At rest:** the relay is self-hosted by the people who use it. Requests live in memory only. They are forgotten an hour after their 10-minute deadline (an approved standing approval, an hour after its end date), or at a restart. On disk: the public keyring, the audit log (`id`, `aud`, `level`, `request_hash`, time, credential and result; never the request itself), signed revocations, pinned service receipt keys and the MCP access token.
- **No tracking:** no analytics, no third-party scripts, no cloud account, no external fonts.
- **Network:** the gate makes no outgoing calls. The secure install's daily update job fetches the signed release list from github.com once a day ([decision 36](decisions.md)); `phygital-key update` and `install.sh` contact GitHub only when run.
- **Approval page:** plain HTML and JS, strict CSP (`default-src 'none'`), no LLM, never framed.
- **No encrypted requests for now** ([decision 33](decisions.md)). The approval page is served by the gate, so encrypting a request against the gate helps only if the page comes from somewhere the gate cannot change. If a hosted gate is ever built, private mode would come with it: a pinned approval page and a device-held decryption key.

## 10. Must-have rules

- Relay and approval page contain no LLM. Keys live only on the person's device.
- Exact request, exact audience, short expiry, single use. Anything unclear means no.
- Enforcement lives in the verifier's code, never in an AI's instructions. MCP tool hints (like `destructiveHint`) are advice only.
- No home-made crypto. Widely used libraries for JCS, hashing and signatures (RustCrypto `p256`, `sha2`, `serde_jcs`; WebCrypto in JavaScript).
- Constant-time comparisons for hashes and challenges.
- Never log secrets. The audit log is append-only and hash-chained.
- Test vectors are law. A spec change needs new vectors first.
- Labeled "pre-audit" until an outside security review.

## 11. Open questions

1. Small schemas and human-readable templates for common types: `deploy`, `payment`, `delete`, `send_email`, `sign_document`, `login`. Unknown types show the raw canonical fields (as the page does today for all types).
2. Whether to also publish the root fingerprint through DNS or `did:web`, so it can be learned without meeting.
3. The approval is a bearer token for 60 s by default (300 s at most), bound to request and audience. Binding it to the requester's key (`cnf`) is not built; standing approvals already bind each use to a key the gate holds (`use_key`).

## 12. AP2 alignment

The full mapping, including Mastercard's Verifiable Intent and what an AP2 export would cost, is in [`docs/ap2.md`](ap2.md).

| PKA | AP2 |
|---|---|
| Approval object | Closed mandate |
| Approval page | Trusted Surface (must be non-agentic) |
| Verifier | Merchant / credential provider verification |
| Standing approval (section 7c: limits, `use_key`, end date) | Open mandate with constraints and `cnf` |
| SD-JWT encoding (not built, [decision 28](decisions.md)) | SD-JWT mandates |

## 13. Tests

- **Shared vectors** (`test-vectors/`): 55 approval cases, 25 key-history cases and 43 standing-approval cases, every error code, every edge (exact size limits, exactly at `exp`, 30 s and 31 s clock drift, 300 s and 301 s life, NFD input, KELVIN SIGN payee, key order, floats, duplicate keys, deep nesting, lone surrogates, look-alike letters, flipped signature bit, wrong origin, wrong RP ID, wrong type, wrong challenge, cross-origin frame, signature over other claims, untrusted key, missing presence, missing verification, replay, prompt-injection text).
- **Property tests** (2000 cases each): random text and bytes never verify; changing any bit of a valid approval is refused; any other request is refused; canonical form is idempotent; `verify` never panics on random requests.
- **End-to-end and red team** (`tests/e2e/`, 108 checks): real browser with a virtual passkey, real MCP client, demo service, and every attack an agent can try through the gate and the page.

## 14. Out of scope for now

2 of 3 approvers (format ready), delegation between agents, private mode, payment settlement, an AP2 / SD-JWT export (see `docs/ap2.md`), adapters other than MCP (an A2A version is planned). See the [roadmap](roadmap.md).

## 15. Deliverables

1. This spec and the test vectors.
2. Verifier library in Rust (`phygital-key-verify`), an independent JavaScript verifier (`verifier-js/`) that passes the same vectors, and `phygital-key verify` for any other language.
3. Gate (MCP server, Rust, on the official `rmcp` SDK) and approval page, shipped as one program (`phygital-key`).
4. Attack test suite.
5. A self-contained demo anyone can run in one command (`phygital-key demo`): a fake deploy, payment and message service that only acts with a valid approval.

## Sources

- AP2 specification: https://ap2-protocol.org/ap2/specification/
- MCP elicitation spec: https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation
- WebAuthn Level 3: https://www.w3.org/TR/webauthn-3/
- RFC 8785 (JCS): https://www.rfc-editor.org/rfc/rfc8785
- HPKE (RFC 9180): https://www.rfc-editor.org/rfc/rfc9180
- WebAuthn PRF extension: https://w3c.github.io/webauthn/#prf-extension
- notary-mcp: https://github.com/salmanneedsajob/notary-mcp
- CoSig: https://pypi.org/project/cosig/
- SynAuth MCP: https://pypi.org/project/synauth-mcp/
