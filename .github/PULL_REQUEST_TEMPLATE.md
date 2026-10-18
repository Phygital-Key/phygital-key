## What and why

<!-- What this changes, and why. Link the issue if there is one. -->

## Checklist

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
- [ ] Test vectors regenerated, and `git diff --exit-code test-vectors` shows only the changes I meant
- [ ] A change to what is accepted or refused has new vectors first, and lands in both verifiers (Rust and `verifier-js`)
- [ ] `cd verifier-js && npm test` passes
- [ ] Gate, approval page or template changes: each product's end-to-end tests pass against this change (for Phygital Key MCP, its `tests/e2e`), with a new check there where it fits
- [ ] `cargo deny check` passes
- [ ] No new dependency, or it was discussed in an issue first (licence on the `deny.toml` allowlist)
- [ ] Docs updated where people will notice the change, and `CHANGELOG.md` has a line
- [ ] No em dashes in docs or messages
- [ ] No secrets, tokens, real private keys or release keys anywhere in the change
- [ ] Template changes (`crates/phygital-key-gate/assets/*.sh`) keep the `# BEGIN PRODUCT` and `# BEGIN RELEASE KEYS` blocks, so `product-files` can fill them
- [ ] Shared behaviour is changed here, not in a product
- [ ] The NOTICE credit is unchanged

## Effect on safety

<!-- Does this touch a defence in docs/threat-model.md? Which test or vector proves it still holds? -->
