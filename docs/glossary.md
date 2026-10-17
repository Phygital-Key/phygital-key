# Glossary

Every term the Phygital Key docs use, in alphabetical order, each in one to three plain sentences with a link to the page that explains it. Other pages link here the first time they use a term.

### AP2

The Agent Payments Protocol, an open protocol for AI agents that pay, now at the FIDO Alliance. Phygital Key borrows its ideas (a non-agentic Trusted Surface, mandates, receipts that point at an approval by hash) but does not export AP2 formats yet. See [AP2](ap2.md).

### Approval

The person's signed yes for one exact request, for one audience, at one level, for a short time, usable once. It is a base64url JSON object with `claims` and one WebAuthn signature, sent as an `approval` field or a `Phygital-Approval` header. See [spec](spec.md) and [how it works](how-it-works.md).

### Approval page

The web page the gate serves where the person reads a request and approves with their key or denies. It shows only the canonical request built by the gate and refuses to sign claims that differ from what it shows. See [approval page](approval-page.md).

### Attestation

An optional certificate in which an authenticator's maker vouches for its model. Phygital Key asks for attestation `none`, so the gate cannot tell a hardware key from a software key; the one-time enrollment link and approvals by existing keys provide the trust instead. See [WebAuthn](webauthn.md#registration).

### Audience

The `aud` claim: who will act on the approval, such as `https://deploy.example.com`. It is 1 to 256 visible ASCII characters, and a verifier refuses an approval whose audience is not its own (`WRONG_AUDIENCE`). See [spec](spec.md).

### Automatic update

The secure install's daily job (`io.phygitalkey.update` on macOS, `phygital-key-update.timer` on Linux) that installs a newer release only if its release list is signed by a pinned release key, and switches back to the previous program if the new gate does not start. On by default; `--no-auto-update` turns it off. See [architecture](architecture.md#the-daily-job).

### Canonical form

The one exact byte string for a JSON value: RFC 8785 (JCS) over text already in NFC, integers only within ±(2^53−1), no floats. Hashes and signatures are always over the canonical form. See [canonical form](canonical-form.md).

### Challenge

The 32 bytes the person's key signs: SHA-256 of `phygital-key/approval/v1`, a zero byte, and the canonical claims. The approval page computes it itself in the browser. See [canonical form](canonical-form.md#the-challenge).

### Elicitation

An MCP feature that lets a server ask the client to show something to the person. The gate uses URL-mode elicitation to offer the approval page link; "accept" there only means the page was opened, never a yes. See [connect your AI](connect-your-ai.md).

### ES256

The signature algorithm used everywhere in Phygital Key: ECDSA on the P-256 curve with SHA-256. Passkeys, hardware keys, receipts and use tokens all use it. See [WebAuthn](webauthn.md#the-signature).

### Fingerprint

The base64url SHA-256 of a key's SPKI public key. Pinning the first key's fingerprint is enough to check a whole signed key history. See [key history](key-history.md).

### Gate

The `phygital-key` program: the MCP server plus the approval page, with the state behind them. It shows requests and collects signatures but can never create a yes by itself. See [architecture](architecture.md).

### JCS

The JSON Canonicalization Scheme, RFC 8785: keys sorted by UTF-16 code units, no whitespace, fixed string escaping. Phygital Key adds the NFC and integer rules around it. See [canonical form](canonical-form.md).

### Key

A passkey or hardware key that can sign approvals. Its private half never leaves the person's device; the gate and verifiers hold only the public half. See [keys and devices](keys-and-devices.md).

### Key card

The public record of one key: `credential_id`, `public_key` (SPKI), `rp_id`, `name`, `created`. Verifiers trust keys by their cards. See [WebAuthn](webauthn.md#the-key-card).

### Key history

The list of signed `add_key` and `remove_key` approvals kept in a keyring, oldest first, starting with the first key approving itself. It lets anyone check every listed key from one pinned fingerprint and spot an old copy. See [key history](key-history.md).

### Keyring

The set of key cards a verifier trusts, as `{"keys": [...]}`, optionally with `origin` and signed `history`. Whoever can write a verifier's keyring decides which keys count. See [key history](key-history.md).

### Level

How strongly the person proved it was them: `medium` (a touch, user presence) or `high` (fingerprint, face or PIN, user verification). The verifier decides which level each action needs. See [WebAuthn](webauthn.md#levels).

### MCP

The Model Context Protocol, the standard way AI apps call tools. The gate is an MCP server with six tools, none of which can approve. See [tools](tools.md).

### NFC

Unicode Normalization Form C, the standard composed form of text. Phygital Key refuses text that is not already NFC instead of rewriting it, because NFC folds some characters into others (KELVIN SIGN into `K`). See [canonical form](canonical-form.md#nfc-refused-never-rewritten).

### Origin

The scheme, host and port of a web page, such as `https://approve.example.com`. The browser writes the approval page's origin into what the key signs, and the verifier accepts only its allowed origins. See [WebAuthn](webauthn.md#clientdatajson).

### P-256

The elliptic curve (also called secp256r1 or prime256v1) used for every key and signature in Phygital Key. See [WebAuthn](webauthn.md#the-key-card).

### Passkey

A WebAuthn key kept by a phone, laptop or password manager, often synced across the person's devices. It signs after a touch, fingerprint, face or PIN. See [keys and devices](keys-and-devices.md).

### Receipt

The service's signed answer after acting: `success` or `error`, pointing at the approval by hash. It is a compact JWS signed with the service's own key, and the gate records it only if that key is pinned. See [receipts](receipts.md).

### Release key

The ECDSA P-256 key that signs each release's release list. It is kept encrypted on the maintainer's computer, never on GitHub, with a second backup key kept offline; both public keys are pinned in `install-service.sh` and `install.sh`. See [architecture](architecture.md#pinned-release-keys).

### Release list

`release.txt`: each release's version and the SHA-256 of every release file, in an exact line format, with its signature in `release.txt.sig`. Installers and updaters take the SHA-256 only from a signed list. See [architecture](architecture.md#releasetxt).

### Request

What the person approves: exactly `{"type": "<name>", "params": {...}}`, with strings, integers, booleans, null, arrays and objects, never floats. It is shown, logged by hash and signed, so it must hold no secrets. See [spec](spec.md).

### Requester

Whoever asks for a yes: usually an AI agent, but also any app, business or person. It builds the request, never holds the person's key and never approves. See [how it works](how-it-works.md#the-four-roles).

### Revocation

The person's signed one-time approval of `{"type": "revoke", "params": {"standing_id": ...}}`, published at `/.well-known/phygital-key-revoked.json`. Services that pick it up refuse that standing approval with `REVOKED`. See [revocations](revocations.md).

### RP ID

The relying party id: the domain a WebAuthn key belongs to, such as `approve.example.com` or `localhost`. Browsers use a key only on its own RP ID, and the verifier checks its hash in the authenticator data. See [WebAuthn](webauthn.md#authenticatordata).

### Seen store

Where a verifier records approval ids it has accepted, so each works once (`REPLAYED` the second time). Several instances of one service must share one store. See [verifier (Rust)](verifier-rust.md).

### SPKI

SubjectPublicKeyInfo: the standard DER encoding of a public key with its algorithm. A key card's `public_key` is a P-256 SPKI (91 bytes), base64url. See [WebAuthn](webauthn.md#the-key-card).

### Standing approval

A one-time `high` approval of limits rather than one action: the same type of action, exact fixed fields, varying fields within ranges or lists, a maximum number of uses, at most 30 days. Each use needs a use token. See [standing approvals](standing-approvals.md).

### stdio

Standard input and output: the MCP transport most AI apps use when they start the gate themselves (`phygital-key mcp`). Only the app that started the gate can talk to it, so no access token is needed. See [connect your AI](connect-your-ai.md).

### Streamable HTTP

The MCP transport over HTTP. `phygital-key serve` offers it at `/mcp`, protected by an access token and refusing any browser `Origin`. See [architecture](architecture.md#the-http-guard).

### Trusted Surface

AP2's name for the screen where a person sees and confirms what an agent wants, which must not be run by an agent. The approval page plays this role. See [AP2](ap2.md).

### Use store

Where a service counts uses of each standing approval, refusing a use id seen before (`REPLAYED`) or one past `max_uses` (`USED_UP`). It must be durable and shared by all instances. See [standing approvals](standing-approvals.md).

### Use token

A short ES256 JWS (`typ` `pka-use+jwt`) that the gate signs for one use of a standing approval, for one exact request, living at most 60 seconds. The gate's use key lives only in memory and is thrown away on revoke. See [standing approvals](standing-approvals.md).

### User presence

The authenticator's UP flag (`0x01`): a person touched the key. It is required for every approval and is all `medium` needs. See [WebAuthn](webauthn.md#authenticatordata).

### User verification

The authenticator's UV flag (`0x04`): the person proved who they are with a fingerprint, face or PIN. It is required for `high`. See [WebAuthn](webauthn.md#authenticatordata).

### Verifier

The service that acts on an approval and checks it first, with its own audience, the exact request it is about to run, the level it needs and its own keyring. Two independent verifiers exist, in Rust and JavaScript. See [verifier (Rust)](verifier-rust.md) and [verifier (JavaScript)](verifier-js.md).

### WebAuthn

The web standard that lets a page ask a passkey or hardware key to sign, with the key bound to one site. Phygital Key uses its assertions as approvals. See [WebAuthn](webauthn.md).

## Next

- [How it works](how-it-works.md)
- [Architecture](architecture.md)
- [Spec](spec.md)
