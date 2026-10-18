# WebAuthn and passkeys

This page explains what WebAuthn and passkeys are, then exactly what the verifier checks in each signature, byte by byte. It is for implementers, reviewers and anyone who wants to know why a phishing page cannot collect a yes. The code is `crates/phygital-key-verify/src/webauthn.rs` (checks) and `verify.rs` (order); the browser side is `crates/phygital-key-gate/assets/app.js` (the same in every product).

## On this page

- [In plain words](#in-plain-words)
- [What the verifier checks](#what-the-verifier-checks)
- [The key card](#the-key-card)
- [Registration](#registration)
- [Approving from a phone (cross-device)](#approving-from-a-phone-cross-device)
- [Levels](#levels)
- [What WebAuthn does not give](#what-webauthn-does-not-give)

## In plain words

WebAuthn is the web standard (W3C, with FIDO2's CTAP for the device side) that lets a website ask your device to prove it is you, with a key that never leaves the device. A passkey is a WebAuthn key kept by your phone, laptop or password manager, often synced between your devices. A hardware key (a YubiKey, for example) is a WebAuthn key on a separate device.

Three properties matter here:

- **The private key stays in the authenticator.** The website only ever sees the public key and signatures.
- **Each key belongs to one site.** The key is made for a relying party id (RP ID), a domain such as `approve.example.com`. The browser refuses to use it on any other site, and puts the page's origin into what it signs.
- **The device says how you proved it was you.** A flag says a person was present (a touch). Another says the person was verified (fingerprint, face or PIN).

Phygital Key uses WebAuthn for one thing: the gate's approval page asks your key to sign a [challenge](glossary.md#challenge) that is a hash of the exact claims. No one else can make that signature.

## What the verifier checks

`verify` runs its checks in a fixed order (spec section 7). Steps 6 to 8 are the WebAuthn part:

| Step | Check | Error |
|---|---|---|
| 6. Key | `credential_id` is in the keyring; the card's `rp_id` is in the verifier's allowed RP IDs; the card's public key parses as P-256 | `UNTRUSTED_KEY` |
| 7. Assertion | client data, authenticator data and signature, below | `BAD_SIGNATURE` |
| 8. Presence | user present flag set; user verified flag also set when the claims' level is `high` | `ASSURANCE_TOO_LOW` |

The key is found before the signature is checked, because an approval does not carry its public key. The flags are read only after the signature is valid, because unsigned flags prove nothing (decision 8).

### clientDataJSON

The browser builds this JSON and the authenticator signs its SHA-256 hash. The verifier checks, in order (`check_client_data`):

| Field | Rule |
|---|---|
| (size) | At most 4096 bytes (vector `client_data_exactly_4096_bytes` passes, `client_data_4097_bytes` is `BAD_SIGNATURE`) |
| (parse) | Strict JSON: no duplicate keys, no lone surrogates, nesting at most 16. Unknown extra fields are allowed: browsers add them. |
| `type` | Exactly `webauthn.get` for an approval (`webauthn.create` is a registration: vector `wrong_type`) |
| `challenge` | base64url-decodes to exactly the challenge the verifier computed from the claims, compared in constant time (vector `wrong_challenge`) |
| `origin` | Exactly one of the verifier's allowed origins, for example `https://approve.example.com` (vector `wrong_origin`) |
| `crossOrigin` | Absent or `false`. `true`, or any string, is refused (vector `cross_origin_frame`). |
| `topOrigin` | Must be absent. Present means the page ran inside a frame. |

The last two stop a page framed inside another site from collecting a yes. The gate also sends `frame-ancestors 'none'` and `X-Frame-Options: DENY`, so the real page cannot be framed in the first place.

### authenticatorData

The authenticator builds these bytes. The verifier reads only the first 37:

| Offset | Length | Field | Checked |
|---|---|---|---|
| 0 | 32 | `rpIdHash`: SHA-256 of the RP ID | Must equal SHA-256 of the key card's `rp_id`, constant time (vector `wrong_rp_id`) |
| 32 | 1 | `flags` | UP and UV, after the signature |
| 33 | 4 | `signCount`, big-endian | Not used |
| 37 | any | Attested credential data, extensions | Not read (covered by the signature) |

Fewer than 37 bytes is `BAD_SIGNATURE`.

The flags byte:

| Bit | Mask | Name | Used |
|---|---|---|---|
| 0 | `0x01` | UP, user present | Required for every approval (vector `missing_user_presence`) |
| 2 | `0x04` | UV, user verified | Required when the level is `high` (vector `missing_user_verification_on_high`) |
| 3 | `0x08` | BE, backup eligible | Not read |
| 4 | `0x10` | BS, backed up | Not read |
| 6 | `0x40` | AT, attested credential data | Not read |
| 7 | `0x80` | ED, extension data | Not read |

$$\text{medium ok} \iff \text{flags} \mathbin{\&} \texttt{0x01} \neq 0 \qquad \text{high ok} \iff \text{flags} \mathbin{\&} \texttt{0x05} = \texttt{0x05}$$

**Why the counter is not used.** `signCount` was meant to reveal a cloned authenticator: each signature should carry a higher count. Synced passkeys are copied on purpose between devices, and many report `0` every time, so a counter check would refuse real people or give false alarms. Replays are stopped by the single-use claims `id` and the verifier's [seen store](glossary.md#seen-store) instead.

### The signature

The authenticator signs the authenticator data followed by the SHA-256 of the client data, with ES256: ECDSA on the P-256 curve, with SHA-256 as the message hash.

$$m = \text{authenticatorData} \,\|\, \mathrm{SHA256}(\text{clientDataJSON})$$

$$\mathrm{valid} \iff \mathrm{ECDSA\text{-}Verify}_{\text{P-256}}\big(\text{pk},\ \mathrm{SHA256}(m),\ (r, s)\big)$$

WebAuthn returns the signature DER-encoded: `SEQUENCE { INTEGER r, INTEGER s }`, usually 70 to 72 bytes. The verifier parses it with `Signature::from_der` from the RustCrypto `p256` crate and verifies with the same crate. A signature that does not parse, or does not verify, is `BAD_SIGNATURE` (vectors `flipped_signature_bit`, `signature_from_other_claims`). No signature math is written by hand.

Receipts and use tokens are signed differently: they are JWS, where the ES256 signature is the raw 64 bytes `r || s`, not DER. See [receipts](receipts.md).

Because the challenge is inside the signed client data, and the challenge is a hash of the canonical claims, one signature binds:

$$\text{signature} \rightarrow \text{clientDataJSON} \rightarrow \text{challenge} \rightarrow \mathrm{canonical}(\text{claims}) \rightarrow \{\text{request}, \texttt{aud}, \text{level}, \texttt{iat}, \texttt{exp}, \texttt{id}\}$$

<details>
<summary>The assertion in the <code>valid_medium</code> test vector</summary>

clientDataJSON (decoded):

```json
{"challenge":"F8G3lstZ0FiIgdmhUz8JTz_w63xHE9jUx2ERuO8aAFs","crossOrigin":false,"origin":"https://approve.example.com","type":"webauthn.get"}
```

authenticatorData, 37 bytes in hex:

```text
fe980bb874053c9785b3e03e1f7c42abf45f63cec7c3b6b72f7c782ebeeb10a9  rpIdHash = SHA-256("approve.example.com")
01                                                                flags: UP
00000000                                                          signCount
```

The signature is 70 bytes of DER starting `30 44 02 20`. In `valid_high` the flags byte is `05` (UP and UV).

</details>

## The key card

The gate stores, and a verifier trusts, one public key card per key:

```json
{ "credential_id": "vSzUGSD7ABGzgRrk5FijAg", "public_key": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE...", "rp_id": "approve.example.com", "name": "My phone", "created": "2026-10-17" }
```

| Field | What it is |
|---|---|
| `credential_id` | The id the authenticator gave the key, base64url. The approval names it. |
| `public_key` | SubjectPublicKeyInfo (SPKI) DER of the P-256 public key, base64url |
| `rp_id` | The RP ID the key was made for. Must be in the verifier's allowed list. |
| `name`, `created` | For people only. They are still compared whole when a key history is checked. |

A P-256 SPKI is 91 bytes (122 base64url characters):

| Bytes | Content |
|---|---|
| `30 59` | SEQUENCE, 89 bytes |
| `30 13 06 07 2a 86 48 ce 3d 02 01 06 08 2a 86 48 ce 3d 03 01 07` | Algorithm: id-ecPublicKey, curve prime256v1 (P-256) |
| `03 42 00` | BIT STRING, 66 bytes, no unused bits |
| `04` + 32 + 32 | Uncompressed point: X, then Y |

The key's [fingerprint](glossary.md#fingerprint) is the base64url SHA-256 of those 91 bytes:

$$\text{fingerprint} = \mathrm{base64url}\big(\mathrm{SHA256}(\text{SPKI DER})\big)$$

## Registration

Adding a key uses WebAuthn's `create` call. The enroll page asks for:

| Option | Value |
|---|---|
| `pubKeyCredParams` | `[{type: "public-key", alg: -7}]` (ES256 only) |
| `attestation` | `"none"` |
| `authenticatorSelection` | `residentKey: "preferred"`, `userVerification: "preferred"` |
| `excludeCredentials` | Keys already enrolled, so the device does not make a duplicate |
| `challenge` | 32 random bytes from the gate |
| `user.id` | 16 random bytes |

The page reads the public key with `getPublicKey()`, which gives the SPKI directly, so neither the page nor the gate parses CBOR (decision 4). The page refuses an authenticator whose `getPublicKeyAlgorithm()` is not `-7`. The gate checks the registration's client data (`type` `webauthn.create`, its challenge, its origin, not cross-origin), that the key is P-256 and that the credential id is 1 to 1023 bytes.

**What attestation `none` means.** Attestation is an optional certificate in which the device maker vouches for the authenticator model. Phones and password managers do not provide it, so the gate asks for none. The gate therefore cannot tell a hardware key from a software key, and it does not need the registration's own signature. Trust comes from elsewhere:

- the first key is added only through the one-time link printed in your own terminal, and then signs its own `add_key` with a normal approval (`webauthn.get`), which proves the key works;
- every later key must be approved at level `high` by a key that is already trusted.

A service that wants only hardware keys can trust only those keys in its keyring (decision 35). See [keys and devices](keys-and-devices.md) and [key history](key-history.md).

## Approving from a phone (cross-device)

On a computer without a passkey or hardware key, the browser offers to use a phone: it shows a QR code, the phone scans it, the two connect through the platform's hybrid transport (which also checks over Bluetooth that the phone is nearby), and the phone signs. Phygital Key needs no code for this. The signature is the same kind of assertion: the browser on the computer still writes the page's origin into the client data, and the phone's key is still bound to the gate's RP ID, so every check above is unchanged.

## Levels

| Level | Authenticator flags | The page asks for | Examples |
|---|---|---|---|
| `medium` | UP | `userVerification: "preferred"` | Edit code, send a message, a small order, a revocation |
| `high` | UP and UV | `userVerification: "required"` | Production deploy, delete data, move money, add or remove a key, any standing approval |

The verifier, not the requester, decides which level an action needs: it passes its own level to `verify` and refuses lower claims (`ASSURANCE_TOO_LOW`). Independently, the flags must prove what the claims say: a `high` claim without the UV flag is refused even if the service only asked for `medium`. A security key without a PIN can only approve `medium`.

## What WebAuthn does not give

The authenticator does not show what it signs: it signs a hash. The person sees the request only on the approval page, which the gate serves. Whoever controls the gate host could show one request and ask the key to sign another. See the [approval page](approval-page.md#what-the-page-cannot-protect-against) and the [threat model](threat-model.md).

## Next

- [Canonical form](canonical-form.md): the claims bytes inside the challenge
- [Approval page](approval-page.md): the browser side of signing
- [Key history](key-history.md): how key cards are trusted over time
- [Error codes](error-codes.md): every code a verifier returns
