# Decisions

Each entry: what we chose, and why. Newest last.

1. **Build on WebAuthn and AP2 ideas, no new crypto format.** Passkeys already work on almost every phone, laptop and security key. AP2 (FIDO Alliance) already defines the Trusted Surface and mandates. A new format would lose.
2. **The person's device signs; the verifier checks offline.** No call home, no server to trust at check time, works between strangers.
3. **Rust for the core.** Memory-safe like Python, but strict types catch more mistakes at build time, and it ships as one small static binary with no runtime to install. One checked implementation can later be wrapped for Python and JavaScript instead of three separate rewrites. The official MCP SDK (`rmcp`) supports URL-mode elicitation.
4. **Own WebAuthn assertion parsing, RustCrypto for the math.** `webauthn-rs` is MPL-2.0 and pulls in OpenSSL. An assertion check is small: parse two fields, hash, verify ES256 with the widely used `p256` crate. Registration uses `getPublicKey()` (SPKI), so no CBOR parsing is needed.
5. **`serde_jcs` for RFC 8785.** It sorts keys by UTF-16 as the RFC requires (checked against the RFC's own example). We add NFC, integer limits and duplicate-key refusal around it.
6. **No floats, ever.** Number formatting is the most common canonicalization bug. Amounts are strings or integers.
7. **The verifier sets the level.** The requester picks a level, but the service decides what it accepts. Otherwise an agent could ask for `medium` on a production deploy.
8. **Check the key before the signature, flags after it.** The approval does not carry the public key, so the key must be found first. Flags are only trusted once the signature is valid.
9. **Two clocks.** 10 minutes to decide, 60 seconds from Approve to use. Fixing `iat` at request time would rush people.
10. **The page computes the challenge itself** from the exact claims text and refuses to sign if those claims differ from what it shows.
11. **Enrollment token in the URL fragment, from the CLI only.** Fragments are never sent to servers or logged. The agent never sees it.
12. **Later keys need approval by an existing key.** Attestation `none` means the server cannot tell a real authenticator from a software key, so trust must come from a key already trusted.
13. **Pending requests in memory, keyring and audit log on disk.** Fewer stored secrets, nothing to leak after an hour. A restart fails closed.
14. **One binary, three modes.** `mcp` (stdio, what most AI apps launch), `serve` (HTTP, many clients), `demo` (gate plus a fake service). Plus `verify` so any language can check an approval by calling a command.
15. **Exactly one signature in v0.1, a list in the format.** 2-of-3 approvals later need no format change.
16. **Signed, not encrypted, in v0.1.** End-to-end encrypted requests need the WebAuthn PRF extension, whose platform support must be checked first. Planned for v0.2.
