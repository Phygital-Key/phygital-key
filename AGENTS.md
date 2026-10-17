# AGENTS.md

Notes for AI coding agents (Claude Code, Codex, Cursor, Copilot and others) and human contributors who change this repository. Read this before you change code, tests or docs.

> [!NOTE]
> If you are an AI agent **using** the Phygital Key tools (asking a person to approve an action, not editing this code), read [docs/agents.md](docs/agents.md) and the [MCP tools reference](docs/tools.md) instead. The short version: you can ask, you can never approve; send the exact approved request; on `denied`, `expired` or `cancelled`, do not act.

## What this is

Phygital Key gives a person's signed yes, for one exact request, that anyone can check. The person approves with a passkey or hardware key (WebAuthn, ES256 over P-256); the service that acts verifies the approval itself, offline, before it does anything. Approvals are signed and verified, not encrypted.

Three parts live here, all Apache-2.0:

- **The format and its test vectors** (`docs/spec.md`, `test-vectors/`): format `pka.1`, canonical form RFC 8785 (JCS) over NFC text with integers only, challenge `SHA-256("phygital-key/approval/v1" || 0x00 || JCS(claims))`.
- **Two verifiers that share no code:** Rust (`crates/phygital-key-verify`) and JavaScript (`verifier-js/`, WebCrypto only, no dependencies). Both must pass every shared vector (123: 55 approvals, 25 key histories, 43 standing approvals).
- **The gate** (`crates/phygital-key`, the program `phygital-key`): an MCP server (stdio, or Streamable HTTP at `/mcp` with a bearer token) plus the approval page, one binary. Six MCP tools; none can approve. It also has `serve`, `demo`, `verify`, `keys`, `enroll`, `install-service`, `update` and other commands ([docs/reference.md](docs/reference.md)). Releases are signed: installers and updates check `release.txt` against pinned release keys ([docs/updates.md](docs/updates.md)).

Free and open source under Apache-2.0. Status: pre-audit. No outside paid audit yet; internal security reviews only ([docs/security-reviews.md](docs/security-reviews.md)).

## Layout

```
Cargo.toml                         workspace: three crates, shared dependency versions, unsafe_code = "forbid"
Cargo.lock                         locked dependency versions (CI builds with --locked)
deny.toml                          cargo-deny: allowed licences, advisories, bans, sources
LICENSE                            Apache-2.0
NOTICE                             the credit line "Created by s17sh70x2jfl031ag" (must stay)
README.md                          front page
CHANGELOG.md                       release notes
CONTRIBUTING.md                    ground rules and the commands to run
SECURITY.md                        how to report a security problem privately
CITATION.cff                       how to cite the project
install.sh                         user install script: downloads a release, checks the signed release list (needs openssl) and the SHA-256, writes ~/.local/bin/phygital-key; holds a copy of the pinned release keys
bucket/phygital-key.json           Scoop manifest
packaging/winget/0.2.0/*.yaml      winget manifests (installer, locale, version), generated, not submitted
scripts/package-manifests.sh       writes the Homebrew, Scoop and winget manifests for a published release
scripts/release-key.sh             maintainer only, once, in the maintainer's own terminal: makes the release signing key and the backup key (passphrases typed), pins both public keys in the two installers
scripts/sign-release.sh            maintainer only, per release, in their own terminal: checks the build records against the local tag commit, writes and signs release.txt (passphrase typed), checks it, uploads it, publishes the release
.gitignore                         build output, node_modules, test results, logs
.github/workflows/ci.yml           CI on every push to main and every pull request (jobs listed below)
.github/workflows/release.yml      on a v* tag: tests, builds the binaries for 5 targets, publishes a build record (attestation) per archive, makes a DRAFT release (never published by CI)
.github/ISSUE_TEMPLATE/*.yml       issue forms (bug, feature, question) and their config
.github/PULL_REQUEST_TEMPLATE.md   pull request checklist

crates/phygital-key-verify/        the Rust verifier library (pure: no network, no clock reads)
  Cargo.toml                       crate manifest
  README.md                        crate readme
  src/lib.rs                       public API, re-exports, constants
  src/model.rs                     data shapes: levels, claims, approval object, key cards, config
  src/verify.rs                    verify(): every check of spec section 7, in order, fail closed
  src/canonical.rs                 canonical form: NFC required (never rewritten), integers only, then JCS
  src/strict.rs                    strict JSON: duplicate keys and nesting deeper than 16 refused
  src/b64.rs                       base64url without padding, strict on decode
  src/webauthn.rs                  the small part of WebAuthn a verifier needs: one ES256 assertion
  src/history.rs                   signed key history: verify_history, verify_history_update, fingerprints
  src/standing.rs                  standing approvals, use tokens, revocations, verify_use
  src/receipt.rs                   service receipts (compact JWS, typ pka-receipt+jwt): sign, verify
  examples/basic.rs                verify the first shared vector, the way a service would
  tests/vectors.rs                 runs every shared test vector
  tests/props.rs                   property tests: random input never verifies, any change is refused
  tests/units.rs                   small exact checks added where mutation testing found gaps

crates/phygital-key/               the gate: MCP server, approval page, CLI
  Cargo.toml                       crate manifest (rmcp 3.5, axum, RustCrypto)
  src/main.rs                      CLI (clap), settings, the modes mcp, serve, demo and the other commands
  src/mcp.rs                       the six MCP tools and the server instructions (INSTRUCTIONS)
  src/state.rs                     the gate's state: requests, statuses, keyring, enrollment, standing approvals, revocations, receipts
  src/web.rs                       the approval page's HTTP routes and JSON API, Host and Origin checks, CSP and other headers
  src/demo.rs                      the demo service: verifies approvals like a real service, signs receipts
  src/audit.rs                     append-only, hash-chained audit log
  src/seen.rs                      file seen store for `phygital-key verify` (lock file, temp file and rename)
  src/update.rs                    signed releases: strict release.txt parser, signature check against the pinned keys, `phygital-key update`
  src/util.rs                      time, randomness, dates, owner-only files
  assets/index.html                home page: trusted keys, fingerprint, remove buttons
  assets/approve.html              approval page for one request
  assets/enroll.html               add-a-key page
  assets/app.js                    the pages' script (no framework, no third-party code)
  assets/style.css                 page styles, dark and light
  assets/fonts.css                 font faces, served by the gate itself
  assets/fonts/*                   Anybody, Hanken Grotesk and JetBrains Mono (woff2) with their OFL licence files
  assets/brand/*.svg               logo marks used by the pages
  assets/install-service.sh        the secure install script (system user, launchd or systemd, daily signed update job), embedded in the binary; its `# BEGIN RELEASE KEYS` block is the one list of pinned release public keys (a test checks install.sh has the same block)

crates/phygital-key-testgen/       writes the shared test vectors
  Cargo.toml                       crate manifest
  src/main.rs                      deterministic generator: fixed TEST ONLY keys, RFC 6979 signatures

test-vectors/                      the shared vectors (part of the spec)
  README.md                        vector format
  vectors.json                     approval cases
  keyrings.json                    key-history cases
  standing.json                    standing-approval cases
  keys/test-keys.json              TEST ONLY private keys (public on purpose; never trust them)

verifier-js/                       the independent JavaScript verifier (Node 20+ and browsers)
  package.json, README.md, LICENSE, NOTICE
  src/index.js                     public API
  src/verify.js                    the checks of spec section 7
  src/strict.js                    strict JSON and base64url
  src/canonical.js                 canonical form and SHA-256
  src/history.js                   signed key history
  src/standing.js                  standing approvals, use tokens, revocations
  test/vectors.test.js             9 tests: every shared vector, single use, strict parsing, and that async seen and use stores are awaited

fuzz/                              cargo-fuzz targets (nightly)
  Cargo.toml, .gitignore
  fuzz_targets/verify.rs           random and mutated approvals
  fuzz_targets/strict_json.rs      strict parser and canonical form
  fuzz_targets/keyring_history.rs  random keyrings
  fuzz_targets/standing.rs         random standing approvals and use tokens

examples/                          runnable examples, each run against a real `phygital-key demo` (CI checks the code builds and the YAML parses)
  README.md                        what each example shows and how to run it
  agent-prompts.md                 instructions to paste into an AI agent
  node-service/                    a deploy service in plain Node, checked with the JavaScript verifier
  rust-service/                    a database worker that checks approvals with phygital-key-verify (its own Cargo.toml, not in the workspace)
  ci-approval/                     a GitHub Actions deploy job that checks the approval with `phygital-key verify`

tests/
  e2e/run.mjs                      end-to-end and red-team checks (108): release binary, Chromium with a virtual passkey, official MCP SDK, demo service
  e2e/package.json, package-lock.json   pinned MCP SDK and Playwright
  secure-install.sh                CI-only check of the secure install (needs password-less sudo)

docs/                              user and developer docs (plain words, GitHub Markdown)
  spec.md                          the format and the checks (normative, with the vectors)
  decisions.md                     numbered decisions and why
  reference.md                     commands, tools, routes, files, limits
  threat-model.md                  what it defends against and what it does not
  agents.md, tools.md              for agents using the gate, and the MCP tool contract
  secure-install.md, updates.md    the secure install, and signed automatic updates
  testing.md, security-reviews.md  every test suite with its numbers, and the internal reviews
  *.md                             the other guides (getting started, integrate, how it works, error codes, glossary and more)
  images/*.png                     screenshots of the real pages
  brand/*.svg                      logos
```

## Rules that must never break

These are the product. A change that weakens one is a security bug.

1. **Test vectors first.** A change to what a verifier accepts or refuses needs new or changed vectors before code: change `crates/phygital-key-testgen`, regenerate, then change the code until they pass. Never edit the vector JSON by hand, and never change a vector to make code pass.
2. **Both verifiers, every time.** The Rust and JavaScript verifiers share only `docs/spec.md` and `test-vectors/`. A change in one lands in the other in the same pull request, and both pass every vector. Never make one import or copy from the other.
3. **No LLM in the gate or the approval page, ever.** No model calls, no AI-written text on the page. The page shows the canonical request built by the gate, never prose from the agent.
4. **No tool can approve.** The MCP tools can ask, wait, cancel, record a receipt and use a standing approval. Only a signature from the person's trusted key makes an approval, and the gate checks it fully before it keeps it. Do not add a tool, flag, route or test hook that approves, signs for the person or skips the check.
5. **Fail closed.** Anything unclear, malformed, unknown or out of range is refused. Unknown fields are refused. Text that is not NFC is refused, never normalized. Floats are refused. A restart forgets pending requests and ends standing approvals.
6. **No home-made crypto.** Use the RustCrypto crates already in the workspace (`p256`, `sha2`, `subtle`), `serde_jcs` and `unicode-normalization`; in JavaScript only WebCrypto. Compare secrets, hashes and challenges in constant time.
7. **No new dependencies without asking.** Open an issue first. The licence must be Apache-2.0, MIT, BSD or similar (`deny.toml`), and the crate must be maintained. `cargo deny check` must pass.
8. **Never log or print secrets.** The enrollment token is shown only by `phygital-key enroll` (and the demo), never in `mcp` or `serve` output. The audit log never stores the request itself. The MCP token lives only in `<home>/mcp-token` (0600).
9. **The verifier decides.** Audience, the exact request and the level are set by the service, never taken from the requester. Keep `verify` pure: no network, no clock reads, no global state except the seen store.
10. **Reserved names stay reserved.** Request types `standing` and `revoke`, and the audience `phygital-key:keyring:<origin>`, are made only by the gate.
11. **Release signing keys never go into the repository or GitHub.** Not in a commit, not in GitHub Actions secrets, not in a workflow, not in an issue. They live encrypted on the maintainer's own computer, under passphrases the maintainer types and keeps in a password manager (never in the macOS Keychain), with a backup key offline. Never run `scripts/release-key.sh` or `scripts/sign-release.sh`, never ask for a passphrase, and never add a way to store one: both scripts refuse to run without a terminal so that an agent cannot use them. Only their public halves are in the repository, in the `# BEGIN RELEASE KEYS` blocks of `crates/phygital-key/assets/install-service.sh` and `install.sh`. CI only makes a draft release; the maintainer signs and publishes it with `scripts/sign-release.sh`. Never add a step that signs, or a secret that could sign, in CI.
12. **Keep the credit.** `NOTICE` and `verifier-js/NOTICE` keep the line "Created by s17sh70x2jfl031ag". Never write any real person's name anywhere, and add no AI attribution lines to docs, commits or release notes.
13. **Docs in plain words, no em dashes.** Not one em dash in any doc, page text or tool message. Follow the existing voice: short sentences, exact facts, every command one you ran. Say "pre-audit" until an outside security review exists. Relative links must resolve.

## Build and test

Run these before you finish (from `CONTRIBUTING.md`):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p phygital-key-testgen -- test-vectors && git diff --exit-code test-vectors
cargo deny check
```

JavaScript verifier (Node 20+), same vectors:

```bash
cd verifier-js && npm test
```

End-to-end and red-team checks (Node 20+, a release build, Chromium from Playwright):

```bash
cargo build --release -p phygital-key
cd tests/e2e && npm ci && npx playwright install chromium
PKA_BIN=../../target/release/phygital-key node run.mjs
```

It prints one `PASS` or `FAIL` line per check (108 checks) and exits non-zero on any failure. It starts its own gate on port 17468 and the demo service on 17469 (and stdio gates on 17478 and 17488), in a temporary data folder.

Fuzzing (nightly Rust and `cargo install cargo-fuzz`):

```bash
cd fuzz && cargo +nightly fuzz run verify -- -max_total_time=60
```

The other targets are `strict_json`, `keyring_history` and `standing`.

CI (`.github/workflows/ci.yml`) runs on every push to `main` and every pull request:

| Job | What it runs |
|---|---|
| test (Linux, macOS, Windows) | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, then regenerates the vectors with `cargo run -q --release -p phygital-key-testgen --locked -- test-vectors` and fails on `git diff --exit-code test-vectors` |
| secure install (Linux, macOS) | builds the release binary and runs `tests/secure-install.sh` (real system user, real launchd or systemd) |
| JavaScript checker | `npm test` in `verifier-js` on Node 24 |
| examples (build and syntax) | `node --check examples/node-service/server.mjs`, `cargo build --manifest-path examples/rust-service/Cargo.toml`, parses `examples/ci-approval/example-deploy.yml` |
| end-to-end and red team | release build, `npm ci --no-fund --no-audit`, `npx playwright install --with-deps chromium`, `node run.mjs` |
| fuzz | each of the 4 targets for 60 seconds on nightly |
| licenses and advisories | `cargo-deny` |

Do not run `tests/secure-install.sh` on your own machine: it needs password-less sudo and installs a system service. Third-party actions are pinned to a commit; keep them pinned.

The release workflow (`.github/workflows/release.yml`) runs `cargo test --workspace --locked` on each of the 5 targets, builds the binaries with a `.sha256` file each, publishes a build provenance attestation per archive (`actions/attest-build-provenance`, which needs `id-token: write` and `attestations: write` in that job), and makes a draft release. Drafts are invisible to installers and to automatic updates. The maintainer then runs `scripts/sign-release.sh vX.Y.Z` in their own terminal on their own Mac: it checks every build record with `gh attestation verify` against the commit of the tag in the maintainer's clone, then signs `release.txt` and publishes. The Rust test `installer_pins_at_least_one_release_key` (in `src/update.rs`) fails until `scripts/release-key.sh` has pinned the release keys.

## Changing the spec safely

The approval format is used by services you cannot see. An approval made by one version must mean the same thing to every verifier.

1. **Ask first** for anything that changes signed bytes or their meaning: claims fields, the challenge prefix, the canonical form, the use-token or receipt headers, error codes. Open an issue and describe the case.
2. **Write the spec change** in `docs/spec.md` (and a numbered entry in `docs/decisions.md`: what you chose and why).
3. **Add vectors first** in `crates/phygital-key-testgen/src/main.rs`, one per new edge (exact limits, one past the limit, the attack it stops). Regenerate with `cargo run -p phygital-key-testgen -- test-vectors` and run it twice: the output must be byte for byte the same.
4. **Watch both verifiers fail** on the new vectors (`cargo test --workspace`, `cd verifier-js && npm test`).
5. **Change both verifiers** until every vector passes in both.
6. **Check the gate and the page** if they build or show what changed (`src/state.rs`, `assets/app.js`), and add an end-to-end check in `tests/e2e/run.mjs` for anything an agent could try.
7. **Update the docs** in the same pull request: `docs/reference.md`, the user guides that mention it, and `CHANGELOG.md`.

Changing an MCP tool (its name, arguments, results, `next` texts or error texts) also means: update `INSTRUCTIONS` in `src/mcp.rs` if the flow changes, [docs/tools.md](docs/tools.md) and [docs/agents.md](docs/agents.md), and the end-to-end checks `MCP lists exactly 6 tools, none can approve` and `MCP server names itself phygital-key with its version`.

## Next

- [CONTRIBUTING.md](CONTRIBUTING.md): ground rules and checks.
- [docs/spec.md](docs/spec.md): the format and every check.
- [docs/decisions.md](docs/decisions.md): why things are the way they are.
- [SECURITY.md](SECURITY.md): report a security problem privately.
