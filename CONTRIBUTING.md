# Contributing

Thanks for helping. Phygital Key is free and open source under the [Apache-2.0](LICENSE) licence. Bug reports, fixes, test vectors and clearer docs are all welcome. Small, focused pull requests are the easiest to review.

## Reporting a problem

Click **New issue** on GitHub and pick a form:

| Form | Use it when |
|---|---|
| **Something doesn't work** | You hit a bug or an error |
| **Question** | You want to know how to do something |
| **Idea or feature** | You want something new or different |
| **Report a security problem, privately** | Someone could make a service act without a real yes. Only the maintainer sees it. See [SECURITY.md](SECURITY.md). |

Issues are public. Never paste your MCP access token, an enrollment link or a private key.

## Ground rules

1. **Test vectors are law.** A change to what a verifier accepts or refuses starts with new or changed vectors in `crates/phygital-key-testgen`, then code.
2. **Both verifiers, every time.** The Rust verifier and the JavaScript checker share no code, only [the spec](docs/spec.md) and `test-vectors/`. A change to what is accepted or refused lands in both, in the same pull request.
3. **Fail closed.** If something is unclear, the answer is no.
4. **No home-made crypto.** Use the existing RustCrypto crates in Rust and WebCrypto in JavaScript. `unsafe` code is forbidden in the workspace.
5. **No new dependencies without discussion.** Open an issue first. The licence must be on the `deny.toml` allowlist (Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib, CDLA-Permissive-2.0), and the crate must be maintained. The JavaScript checker has no dependencies and stays that way.
6. **No LLM** in the gate or the approval page, ever.
7. **Never weaken a defence** listed in the [threat model](docs/threat-model.md) without a test, a vector where it applies, and a clear note in the pull request.

Read [AGENTS.md](AGENTS.md) for how the code is laid out.

## Setup

You need Rust (stable, at least 1.89; the workspace uses edition 2024) and Node 20 or newer for the JavaScript checker and the end-to-end tests. Fuzzing needs nightly Rust.

```bash
git clone https://github.com/s17sh70x2jfl031ag/phygital-key-mcp
cd phygital-key-mcp
cargo build --workspace
```

## Run every check

These are the same checks CI runs on every push and pull request (see [testing](docs/testing.md)). Run them before you open a pull request.

Formatting, lints and tests:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test` includes `installer_pins_at_least_one_release_key`, which fails until release keys are pinned in the installers (see [releasing](#releasing)). Every other test must pass.

Test vectors are up to date and deterministic:

```bash
cargo run -p phygital-key-testgen -- test-vectors && git diff --exit-code test-vectors
```

Licences, advisories, bans and sources:

```bash
cargo deny check
```

Fuzzing (needs nightly Rust and `cargo install cargo-fuzz`). CI runs each of the four targets for 60 seconds:

```bash
cd fuzz && cargo +nightly fuzz run verify -- -max_total_time=60
```

Replace `verify` with `strict_json`, `keyring_history` or `standing` for the others.

JavaScript checker (must pass the same vectors):

```bash
cd verifier-js && npm test
```

End-to-end and red team, with a real Chromium, a virtual passkey and the official MCP SDK (uses ports 17468 to 17488):

```bash
cargo build --release -p phygital-key
cd tests/e2e && npm ci && npx playwright install chromium
PKA_BIN=../../target/release/phygital-key node run.mjs
```

The secure install, including its automatic updates, is tested for real in CI on macOS and Linux (`tests/secure-install.sh`, 30 checks). It needs password-less sudo, so it runs only on CI machines. Do not set up password-less sudo on your own computer to run it.

CI also checks the examples:

```bash
node --check examples/node-service/server.mjs
cargo build --manifest-path examples/rust-service/Cargo.toml
```

## Changing the spec

1. Write the new rule in [docs/spec.md](docs/spec.md), with MUST or MUST NOT.
2. Add vectors for it in `crates/phygital-key-testgen`: at least one case that passes and one that is refused, at the exact edge.
3. Regenerate `test-vectors/` and commit the result.
4. Change the Rust verifier (`crates/phygital-key-verify`) until every vector passes.
5. Change the JavaScript checker (`verifier-js`) until every vector passes.
6. If the gate or the approval page is affected, add an end-to-end check in `tests/e2e/run.mjs`.
7. Write the reason in [docs/decisions.md](docs/decisions.md) when the change is a design choice.

## Releasing

Only the maintainer can make a release that installs, because only the maintainer holds the release key. Installers and the secure install's daily update job accept a release only if its file list (`release.txt`) is signed by a key pinned in `crates/phygital-key/assets/install-service.sh` and `install.sh` (decision 36 in [decisions](docs/decisions.md)).

```mermaid
flowchart LR
  T["Push tag vX.Y.Z"] --> CI["release.yml: test, build 5 targets, build records"]
  CI --> D["Draft release with .sha256 files"]
  D --> S["Maintainer's own terminal: scripts/sign-release.sh vX.Y.Z, passphrase typed"]
  S --> P["release.txt and .sig uploaded, release published"]
```

Once, before the first signed release, on the maintainer's own Mac:

- [ ] Run `scripts/release-key.sh` yourself, in a terminal (it refuses to run without one). It makes two P-256 keys in `~/.phygital-key-release` (a signing key and a backup key), each encrypted with AES-256 under a passphrase you type (at least 16 characters), and pins both public keys in the two installers. Keep both passphrases in your password manager, never in the macOS Keychain, on the Mac in plain text or on GitHub. A program running as you, an AI agent included, can then copy the key files but cannot use them.
- [ ] Move `backup-key.pem` off the Mac (into your password manager as an attachment, or onto a USB stick), as the script says.
- [ ] Commit the two changed installers.

For each release:

1. Update the version (`Cargo.toml`, `verifier-js/package.json`, `CITATION.cff`, the default `VERSION` in `crates/phygital-key/assets/install-service.sh`) and add a `CHANGELOG.md` entry. CI must be green on `main`.
2. Push the tag `vX.Y.Z`. `.github/workflows/release.yml` runs `cargo test` on each runner, builds the five targets, writes a `.sha256` for each archive, publishes a build provenance attestation (build record) for each archive with `actions/attest-build-provenance`, and creates a draft release. Drafts are invisible to installers and to the daily update job.
3. On the maintainer's Mac, in your own terminal, run `scripts/sign-release.sh vX.Y.Z` from a clone that has the tag (needs `gh`, which also checks the build records, `openssl` and the release key). It reads the tag's commit from your clone, downloads the draft's files, computes each SHA-256 itself and compares it with the workflow's `.sha256`, and runs `gh attestation verify` on each file: it must have been built by `.github/workflows/release.yml` from `refs/tags/vX.Y.Z` at that commit. Then it asks for the signing key passphrase, signs `release.txt`, checks the signature against the keys pinned in `install-service.sh`, uploads `release.txt` and `release.txt.sig`, and only then publishes the release as the latest. If any step fails, the release stays a draft. Never run it from an AI agent's session or let an agent type the passphrase: the script refuses to run without a terminal for that reason.
4. Write the package manifests with `scripts/package-manifests.sh vX.Y.Z` (Homebrew formula, Scoop manifest, winget manifests).

> [!CAUTION]
> Never put a release key or its passphrase in GitHub (not in secrets, not in a workflow, not in the repository) or in the macOS Keychain. The signature is what stops someone who takes over the GitHub account from shipping an update that installs as root. A key in GitHub would give them that power, and a passphrase in the Keychain would let any program running as you sign.

## Docs style

Docs are read by people who approve actions, by developers and by AI agents.

- Plain, calm, exact. Short sentences, everyday words, "you". Explain a technical word the first time, or link the [glossary](docs/glossary.md).
- Every claim must be true today and traceable to the code or a command you ran. Show the check, the number or the command instead of praise words.
- Say what it does not do, near where it matters (signed, not encrypted; pre-audit).
- No em dashes. Use a period, comma, colon or parentheses.
- One term for one thing: approval, request, gate, verifier, key, keyring, standing approval, use token, revocation, receipt.
- Write the name exactly "Phygital Key". The program is `phygital-key`.
- Relative links between docs, and every link must resolve.

## Commits and pull requests

- One topic per commit. Start the summary with the area and a colon, then what changed, for example `Install: refuse a foreign listener before starting`.
- Say why in the body when it is not obvious.
- In the pull request, fill in the checklist and say which vectors or checks you added.

## Credit and licence

By contributing, you agree that your work is shared under the [Apache-2.0](LICENSE) licence, the same as the rest of the project. The [NOTICE](NOTICE) file and its line "Created by s17sh70x2jfl031ag" must stay in every copy and every modified version (Apache-2.0, section 4(d)); please do not remove or change it. [CITATION.cff](CITATION.cff) says how to cite the project.

## Security problems

Please report privately. See [SECURITY.md](SECURITY.md).
