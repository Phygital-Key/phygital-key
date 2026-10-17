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
17. **Refuse text that is not NFC instead of normalizing it** (from the security review). Normalizing folds characters like KELVIN SIGN into `K`, so a service could act on bytes that differ from what the person saw. Refusing keeps "what was signed" and "what is run" byte for byte the same.
18. **Audiences are visible ASCII.** URLs, domains and ids fit; invisible and look-alike characters do not.
19. **No token in `mcp` mode output, token used up at the first step** (from the security review). AI apps keep the server's stderr in logs an agent may read.
20. **Claims stay fixed while valid.** A second "start" returns the same claims, so another program cannot swap them while the person is signing.
21. **Keep SHA-256 and ES256.** SHA-256 has no known practical attack (128-bit collision strength), and every passkey, phone and security key signs with ES256, which uses SHA-256 inside the authenticator. A "stronger" hash (SHA-512, SHA-3) would add no real safety and would break support on common devices. Revisit when post-quantum signatures reach WebAuthn authenticators.
22. **Signed key history instead of a signed file.** Each add or remove is a normal approval, kept in the keyring. One pinned fingerprint checks it all, rotation is just more entries, and no new signing format is needed.
23. **The first key signs itself.** It proves the key works before it is trusted, and gives the history a root.
24. **Fuzzing and mutation testing** (ideas from `ap2-evidence-pack`): coverage-guided fuzzing of the verifier, the parser and the key history, and `cargo-mutants` to prove each check is covered by a test.
25. **Key history needs stored state** (security review 2). A valid history proves keys were approved, not that it is the latest. Refetching takes only extensions of the last accepted copy; the origin is pinned by the checker, never read from the file; cards are compared whole.
26. **Receipts count only when signed by a pinned service key.** The agent carries them, so an unchecked one proves nothing.
27. **Standing approvals stay narrow:** exact fixed values, integer ranges or short string lists, a use cap, at most 30 days, one gate-held use key per approval kept in memory. No policy language. Services count uses themselves.
28. **No AP2 export yet.** It would move trust from the passkey to a gate key for plain AP2 verifiers. The mapping is written down in `docs/ap2.md`; the code waits for a real user.
29. **Revocations are signed by the person, not the gate.** A revoke is one more touch, so services can trust it with the keyring they already have, and no gate key is needed. Invalid entries in the published list are ignored: an entry can only add a revocation.
30. **An access token on the HTTP MCP endpoint, on by default.** Any program on the machine could otherwise create requests. stdio needs none.
31. **A second verifier, in JavaScript, with no dependencies.** It shares only the spec and the vectors with the Rust one. Two implementations agreeing on every vector is how unclear spec text is found.
32. **Secure install as a separate system user** (launchd, systemd). It is the simplest way to put the gate out of reach of an agent that runs as you, without a second machine.
