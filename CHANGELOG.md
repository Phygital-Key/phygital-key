# Changelog

## 0.2.0 (2026-10-17)

- **Signed key list.** Every key added or removed is an approval signed by a key already trusted; the first key signs itself. `phygital-key keys --check <fingerprint>` checks a whole keyring from one pinned fingerprint. Keys can be removed from the home page (needs approval). 14 new test vectors in `test-vectors/keyrings.json`.
- **Used approvals remembered by the command line** (`seen-ids.jsonl`, locked while checked), so `phygital-key verify` refuses a second use.
- **Fuzzing** of the verifier, the strict parser and the key history (`fuzz/`, runs in CI), and mutation testing of the verifier.
- **Signed receipts:** a service answers "done" or "failed" with a standard ES256 JWT that points at the approval by hash (AP2 style). The demo service returns them, the agent passes them back with the new `record_receipt` tool, the approval page shows "the service reported: done", and `phygital-key receipt-check` verifies them.
- **More edge test cases** from mutation testing: exact size limits (8192-byte request, 4096-byte client data, 16384-character approval), and five more key-history cases. 54 approval vectors and 19 keyring vectors in total.
- **Installers:** Homebrew tap, a checksum-checking install script, and a Scoop bucket for Windows. winget manifests are generated in `packaging/winget/`.

## 0.1.0 (2026-10-17)

First public version. Pre-audit.

- Spec PKA v0.1 (`docs/spec.md`) and 48 shared test vectors.
- `phygital-key-verify`: pure, offline verifier library in Rust.
- `phygital-key`: MCP gate (stdio and HTTP), approval page, enrollment with one-time links, add-key approval by an existing key, hash-chained audit log, command-line verifier, and a one-command demo.
- Property tests and an end-to-end red-team suite with a real browser and virtual passkey.
