# Contributing

Thanks for helping. Small, focused pull requests are easiest to review.

## Ground rules

1. **Test vectors are law.** A change to what the verifier accepts or refuses needs new or changed vectors first (`crates/phygital-key-testgen`), then code.
2. **Fail closed.** If something is unclear, the answer is no.
3. **No home-made crypto.** Use the existing RustCrypto crates. Ask before adding any dependency (license must be Apache-2.0, MIT, BSD or similar; it must be maintained).
4. **No LLM** in the gate or the approval page, ever.
5. **Plain words** in docs and messages. The people approving are not always engineers.

## Run the checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p phygital-key-testgen -- test-vectors && git diff --exit-code test-vectors
cargo deny check
```

Fuzzing (needs nightly Rust and `cargo install cargo-fuzz`):

```bash
cd fuzz && cargo +nightly fuzz run verify -- -max_total_time=60
```

End-to-end tests (needs Node 20+):

```bash
cargo build --release -p phygital-key
cd tests/e2e && npm ci && npx playwright install chromium
PKA_BIN=../../target/release/phygital-key node run.mjs
```

JavaScript checker (needs Node 20+; must pass the same vectors):

```bash
cd verifier-js && npm test
```

The secure install is tested for real in CI on macOS and Linux (`tests/secure-install.sh`, needs password-less sudo, so only on CI machines).

## Two implementations

The Rust verifier and the JavaScript verifier share no code, only `docs/spec.md` and `test-vectors/`. A change to what is accepted or refused must land in both, with new vectors first.

## Security problems

Please report privately. See [SECURITY.md](SECURITY.md).
