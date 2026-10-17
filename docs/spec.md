# Phygital Key Approval (PKA), spec v0.1

**Goal.** A small open base for one thing: a signed yes from a person, for one exact request, that anyone can check. The person approves with a passkey, fingerprint, face or hardware key. Any party can verify the yes offline in a few lines of code. It works between any two sides: person to person, person to business, business to business, person to system, AI agent to service. Built on WebAuthn (FIDO2) and aligned with AP2 (now at the FIDO Alliance). License: Apache-2.0.

**Design goals, in order:** safe, private, simple to use, simple to add, runs anywhere, self-hosted by default.

The words MUST, MUST NOT and SHOULD are used as in RFC 2119. The test vectors in `test-vectors/vectors.json` are part of this spec: every implementation MUST pass every vector.

## 1. Roles

| Role | What it is | Must |
|---|---|---|
| Requester | Anyone or anything that asks for a yes: an AI agent, an app, a business, a person | Build the request. Never holds the approver's key. Never approves. |
| Relay | Passes the request to the approver and the approval back. In v0.1 this is the Gate, an MCP server | Canonicalize and hash the request, show it on the approval page, keep state. No LLM inside. Can never create a yes by itself. |
| Approver | A person, their authenticator (phone, laptop, security key) and the approval page (AP2 "Trusted Surface") | See the exact request, decide, sign with their own key. The key never leaves their device. |
| Verifier | Anyone who acts on the yes: a service, a business, another person's app, a machine | Check the approval with the verifier before acting. Never trust "they said yes" from the requester. |

One party can hold several roles. Example: a business can be both Requester and Verifier.

## 2. Adapters (how a request reaches the approver)

The base is the format and the verifier. Adapters are just doorways.

- **v0.1: MCP.** The Gate is an MCP server (stdio, or Streamable HTTP at `/mcp`). It asks the client to open the approval page with MCP URL-mode elicitation. If the client has no URL elicitation, it returns the link for the person to open. It never auto-approves.
- **Later:** plain HTTP API, email link, chat apps, QR code on a screen, SDKs for apps.

All adapters produce the same approval object and use the same verifier.

## 3. Request and approval object

### Request

A request is exactly `{"type": "<name>", "params": {...}}`:

- `type`: 1 to 64 characters from `a-z 0-9 _ . -`.
- `params`: a JSON object. Strings, integers, booleans, null, arrays and objects. **No floats**: amounts are strings (`"120.00"`) or integers in minor units (`12000`). Integers MUST be within ±(2^53 - 1).
- No secrets in `params`. They are shown on screen, stored by the relay and hashed.
- Canonical form at most 8 KB.

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
- **`signatures`** is a list. v0.1 requires exactly one. The list makes "2 of 3 approvers" possible later without changing the format.
- Unknown fields anywhere in the approval object, the claims or a signature MUST be refused.
- Duplicate JSON keys MUST be refused. Nesting deeper than 16 MUST be refused. The encoded approval MUST be at most 16 KB.

### Canonical form

`canonical(x)` = RFC 8785 (JCS) of `x`, where every string and key MUST already be in Unicode NFC. Text that is not NFC MUST be refused, never rewritten: NFC folds some single characters into others (KELVIN SIGN U+212A becomes `K`, GREEK QUESTION MARK U+037E becomes `;`), so rewriting would let two different byte strings share one approval. Floats and unsafe integers have no canonical form and MUST be refused. Lone surrogates MUST be refused. A verifier acts on exactly the bytes it verified.

### Challenge

`challenge = SHA-256("phygital-key/approval/v1" || 0x00 || canonical(claims))`

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
- **First key** is trusted directly. **Every later key** creates an `add_key` request at level `high` that an already-trusted key must approve. A new key can never approve itself.
- **Keyring:** a verifier keeps the key cards it trusts: `{"keys": [ ...key cards ]}`. The verifier only reads the keyring it is given. It never fetches anything.
- **Sharing keys:**
  - Inside one team or system: copy the keyring file, or run `phygital-key keys`.
  - Between parties: a party publishes its key cards at `https://<domain>/.well-known/phygital-key.json`. The receiving app (not the verifier) fetches it over HTTPS and stores it as a keyring. The domain is the proof of who owns the keys.
- **Lost or retired key:** remove its card from the keyring. People SHOULD enroll two keys so one loss does not lock them out.
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
4. **Request** (`REQUEST_MISMATCH`): SHA-256 of canonical(the request actually being acted on) equals `request_hash`, and `request_hash` matches `claims.request`.
5. **Level** (`ASSURANCE_TOO_LOW`): `claims.level` is at least the level the verifier requires for this request. The verifier decides the level, never the requester.
6. **Key** (`UNTRUSTED_KEY`): `credential_id` is in the keyring, its `rp_id` is allowed, its public key parses.
7. **Assertion** (`BAD_SIGNATURE`): client data `type` is `webauthn.get`, `challenge` equals the computed challenge, `origin` is allowed, not cross-origin (`crossOrigin` not true, no `topOrigin`); authenticator data RP ID hash equals SHA-256 of the key's `rp_id`; the ES256 signature over `authenticator_data || SHA-256(client_data_json)` is valid.
8. **Presence** (`ASSURANCE_TOO_LOW`): user present flag set; user verified flag set when the level is `high`.
9. **Replay** (`REPLAYED`): atomically record `id` until `exp`; refuse if already seen. Several verifier instances MUST share one store.
10. Only then act. Log `id`, `request_hash`, time, the approver's credential and the result.

**Error codes:** `APPROVAL_REQUIRED`, `MALFORMED`, `WRONG_AUDIENCE`, `EXPIRED`, `REQUEST_MISMATCH`, `BAD_SIGNATURE`, `UNTRUSTED_KEY`, `ASSURANCE_TOO_LOW`, `REPLAYED`, and `DENIED` (returned by relays when the person says no; never by `verify`).

**API:** `verify(approval, expected{aud, request, level}, keyring, seen_store, config{origins, rp_ids}, now) -> ok | error(code)`. The verifier is pure: no network, no clock reads, no global state except `seen_store`.

## 8. Levels

| Level | Examples | Requirement |
|---|---|---|
| low | Read files | No approval |
| medium | Edit code, send email, sign a small order | WebAuthn with user presence (touch) |
| high | Deploy to production, delete data, move money, sign a contract, add a key | WebAuthn with user verification (biometric or PIN) |

A security key without a PIN can only approve `medium`.

## 9. Privacy and encryption

- **Signed, not secret, in v0.1.** The approval proves who said yes to what. Anyone holding it can read the request, so keep requests to what the verifier needs.
- **In transit:** HTTPS everywhere outside localhost. Passkeys refuse to work otherwise.
- **At rest:** the relay is self-hosted by the people who use it. Pending requests live in memory only and are gone after an hour or a restart. On disk: the public keyring and the audit log (`id`, `aud`, `level`, `request_hash`, time, credential and result; never the request itself).
- **No tracking:** no analytics, no third-party scripts, no cloud account, no external fonts.
- **Approval page:** plain HTML and JS, strict CSP (`default-src 'none'`), no LLM, never framed.
- **v0.2, private mode (end-to-end encrypted requests):** the requester encrypts the request to the approver (HPKE, RFC 9180), so the relay only sees ciphertext and the request hash. The approver's decryption key comes from their passkey through the WebAuthn PRF extension. Platform support for PRF must be checked first, which is why this is not in v0.1.

## 10. Must-have rules

- Relay and approval page contain no LLM. Keys live only on the person's device.
- Exact request, exact audience, short expiry, single use. Anything unclear means no.
- Enforcement lives in the verifier's code, never in an AI's instructions. MCP tool hints (like `destructiveHint`) are advice only.
- No home-made crypto. Audited, widely used libraries for JCS, hashing and signatures.
- Constant-time comparisons for hashes and challenges.
- Never log secrets. The audit log is append-only and hash-chained.
- Test vectors are law. A spec change needs new vectors first.
- Labeled "pre-audit" until an outside security review.

## 11. Open questions

1. Small schemas and human-readable templates for common types: `deploy`, `payment`, `delete`, `send_email`, `sign_document`, `login`. Unknown types show the raw canonical fields (as v0.1 does for all types).
2. Whether the `.well-known` file itself should be signed, and how to rotate it.
3. The approval is a bearer token for up to 60 s, bound to request and audience. Fine for v0.1; binding it to the requester's key (`cnf`) is v0.2.

## 12. AP2 alignment

| PKA | AP2 |
|---|---|
| Approval object | Closed mandate |
| Approval page | Trusted Surface (must be non-agentic) |
| Verifier | Merchant / credential provider verification |
| v0.2: standing approval (rules, requester key, expiry) | Open mandate with constraints and `cnf` |
| v0.2: SD-JWT encoding for interop | SD-JWT mandates |

## 13. Tests

- **Shared vectors** (`test-vectors/`): 48 cases, every error code, every edge (exactly at `exp`, 30 s and 31 s clock drift, 300 s and 301 s life, NFD input, KELVIN SIGN payee, key order, floats, duplicate keys, deep nesting, lone surrogates, look-alike letters, flipped signature bit, wrong origin, wrong RP ID, wrong type, wrong challenge, cross-origin frame, signature over other claims, untrusted key, missing presence, missing verification, replay, prompt-injection text).
- **Property tests:** random text and bytes never verify; changing any bit of a valid approval is refused; any other request is refused; canonical form is idempotent.
- **End-to-end and red team** (`tests/e2e/`): real browser with a virtual passkey, real MCP client, demo service, and every attack an agent can try through the gate and the page.

## 14. Out of scope for v0.1

Standing approvals, 2 of 3 approvers (format ready), delegation between agents, private mode, payment settlement, signed receipts from the verifier, adapters other than MCP.

## 15. v0.1 deliverables

1. This spec and the test vectors.
2. Verifier library in Rust (`phygital-key-verify`), later wrapped for Python and JavaScript so every language uses the same checked code. Any language can already call `phygital-key verify` today.
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
