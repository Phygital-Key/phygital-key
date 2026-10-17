## What and why

<!-- What this changes, and why. Link the issue if there is one. -->

## Checklist

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass (`installer_pins_at_least_one_release_key` fails until release keys are pinned; see CONTRIBUTING.md, Releasing)
- [ ] Test vectors regenerated, and `git diff --exit-code test-vectors` shows only the changes I meant
- [ ] A change to what is accepted or refused has new vectors first, and lands in both verifiers (Rust and `verifier-js`)
- [ ] `cd verifier-js && npm test` passes
- [ ] Gate or approval page changes: end-to-end tests pass (`tests/e2e`), with a new check where it fits
- [ ] `cargo deny check` passes
- [ ] No new dependency, or it was discussed in an issue first (licence on the `deny.toml` allowlist)
- [ ] Docs updated where people will notice the change, and `CHANGELOG.md` has a line
- [ ] No em dashes in docs or messages
- [ ] No secrets, tokens, real private keys or release keys anywhere in the change
- [ ] Installer changes keep the `# BEGIN RELEASE KEYS` block the same in `crates/phygital-key/assets/install-service.sh` and `install.sh`
- [ ] The NOTICE credit is unchanged

## Effect on safety

<!-- Does this touch a defence in docs/threat-model.md? Which test or vector proves it still holds? -->
