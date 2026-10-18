# Wycheproof ECDSA cases

Two files copied unchanged from Google's [Wycheproof](https://github.com/C2SP/wycheproof) project (`testvectors_v1/`, commit `7a325932231bd7a724c791c8b0d65ea4d6d2f435`, 2026-10-12), under the Apache License 2.0:

| File | Cases | Signature form | Used for |
|---|---|---|---|
| `ecdsa_secp256r1_sha256_test.json` | 484 | DER | WebAuthn assertions in approvals |
| `ecdsa_secp256r1_sha256_p1363_test.json` | 262 | raw r‖s (64 bytes) | Receipts and use tokens |

They are known-hard ECDSA P-256 / SHA-256 cases: bad DER, out-of-range numbers, edge-case keys and hashes, high S. Both verifiers run every case through the same signature calls they use for real approvals:

- Rust: `crates/phygital-key-verify/tests/wycheproof.rs`
- JavaScript: `verifier-js/test/wycheproof.test.js`

They are not part of the Phygital Key spec. To update them, copy the newer files from Wycheproof and change the commit above.
