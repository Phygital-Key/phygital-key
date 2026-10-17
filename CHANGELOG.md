# Changelog

## 0.1.0 (2026-10-17)

First public version. Pre-audit.

- Spec PKA v0.1 (`docs/spec.md`) and 48 shared test vectors.
- `phygital-key-verify`: pure, offline verifier library in Rust.
- `phygital-key`: MCP gate (stdio and HTTP), approval page, enrollment with one-time links, add-key approval by an existing key, hash-chained audit log, command-line verifier, and a one-command demo.
- Property tests and an end-to-end red-team suite with a real browser and virtual passkey.
