# Changelog

## 0.2.0 (2026-10-17)

- **Standing approvals:** approve once, with limits (exact fixed fields, integer ranges, short string lists, a use cap, up to 30 days). The gate signs a one-minute use token per action with a key it keeps only in memory; the service checks both with `verify_use` and counts uses. Revoke on the approval page. New tools `request_standing_approval` and `use_standing_approval`; new codes `NOT_ALLOWED` and `USED_UP`; 38 test vectors in `test-vectors/standing.json`. Integer limits need both `min` and `max`; standing approvals are always level high; receipts per use (`verify_use_receipt`).
- **Signed key list.** Every key added or removed is an approval signed by a key already trusted; the first key signs itself. `phygital-key keys --check <fingerprint>` checks a whole keyring from one pinned fingerprint. Keys can be removed from the home page (needs approval). Refetched keyrings must extend the copy accepted before (`--previous`), so old copies and forks by removed keys are refused; the origin is pinned by the checker. 25 test vectors in `test-vectors/keyrings.json`.
- **Used approvals remembered by the command line** (`seen-ids.jsonl`: locked, written through a temp file, fails closed if damaged), so `phygital-key verify` refuses a second use.
- **Fuzzing** of the verifier, the strict parser and the key history (`fuzz/`, runs in CI), and mutation testing of the verifier.
- **Signed receipts:** a service answers "done" or "failed" with a standard ES256 JWT that points at the approval by hash (AP2 style). The demo service returns them, the agent passes them back with the new `record_receipt` tool, and the gate shows "the service reported: done" only for receipts signed by a pinned service key (`phygital-key trust-service`). `phygital-key receipt-check` verifies them.
- **More edge test cases** from mutation testing: exact size limits (8192-byte request, 4096-byte client data, 16384-character approval), and five more key-history cases. 55 approval vectors in total.
- **Four independent security reviews** of the new code; all findings fixed (key-list rollback and forks, made-up receipts, double key removal, crash-safe used-ids file, save-then-swap keyring, strict receipt checks, standing ranges without a minimum, standing approvals kept until their end, reserved `standing` type, a cap on stored requests).
- **Installers:** Homebrew tap, a checksum-checking install script, and a Scoop bucket for Windows. winget manifests are generated in `packaging/winget/`.

## 0.1.0 (2026-10-17)

First public version. Pre-audit.

- Spec PKA v0.1 (`docs/spec.md`) and 48 shared test vectors.
- `phygital-key-verify`: pure, offline verifier library in Rust.
- `phygital-key`: MCP gate (stdio and HTTP), approval page, enrollment with one-time links, add-key approval by an existing key, hash-chained audit log, command-line verifier, and a one-command demo.
- Property tests and an end-to-end red-team suite with a real browser and virtual passkey.
