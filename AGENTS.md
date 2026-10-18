# AGENTS.md

Notes for AI coding agents (Claude Code, Codex, Cursor, Copilot and others) and human contributors who change this repository, the Phygital Key base. Read this before you change code, tests or docs.

> [!NOTE]
> If you are an AI agent **using** Phygital Key (asking a person to approve an action, not editing this code), read the guide of the product you are connected to: for Phygital Key MCP, its [agents guide](https://github.com/Phygital-Key/phygital-key-mcp/blob/main/docs/agents.md) and [MCP tools reference](https://github.com/Phygital-Key/phygital-key-mcp/blob/main/docs/tools.md). The short version: you can ask, you can never approve; send the exact approved request; on `denied`, `expired` or `cancelled`, do not act.

## What this is

Phygital Key gives a person's signed yes, for one exact request, that anyone can check. The person approves with a passkey or hardware key (WebAuthn, ES256 over P-256); the service that acts verifies the approval itself, offline, before it does anything. Approvals are signed and verified, not encrypted.

This repository (`Phygital-Key/phygital-key`, renamed from `phygital-key-mcp`) is the **base**: everything the products share. It publishes no programs and no releases. The products are separate repositories that build on it:

| Product | Repository | Program | Agent protocol |
|---|---|---|---|
| Phygital Key MCP | [Phygital-Key/phygital-key-mcp](https://github.com/Phygital-Key/phygital-key-mcp) | `phygital-key-mcp` (0.5.0) | MCP over stdio, or Streamable HTTP at `/mcp` |
| Phygital Key A2A | [Phygital-Key/phygital-key-a2a](https://github.com/Phygital-Key/phygital-key-a2a) (new) | `phygital-key-a2a` | A2A 1.0 JSON-RPC at `/a2a`, agent card at `/.well-known/agent-card.json` |

Four parts live here, all Apache-2.0:

- **The format and its test vectors** (`docs/spec.md`, `test-vectors/`): format `pka.1`, canonical form RFC 8785 (JCS) over NFC text with integers only, challenge `SHA-256("phygital-key/approval/v1" || 0x00 || JCS(claims))`.
- **Two verifiers that share no code:** Rust (`crates/phygital-key-verify`) and JavaScript (`verifier-js/`, WebCrypto only, no dependencies). Both must pass every shared vector (141: 73 approvals, 25 key histories, 43 standing approvals).
- **The gate engine** (`crates/phygital-key-gate`, a library): the approval page, enrollment and the signed key history, one-time and standing approvals, revocations, receipts, the hash-chained audit log, the secure install, signed updates and the shared commands (`serve`, `stdio`, `demo`, `enroll`, `token`, `keys`, `verify`, `install-service`, `update` and others, [docs/reference.md](docs/reference.md)). A product gives it a `Product` (its name, program, port, repository and pinned release keys) and an `Adapter` (its agent protocol), then calls `phygital_key_gate::run`.
- **The templates** every product publishes (`crates/phygital-key-gate/assets/*.sh`): the secure installer, the user installer, the secure-install CI test and the release scripts. A product writes its own copies with `<program> product-files --out .`.

Free and open source under Apache-2.0. Status: pre-audit. No outside paid audit yet; internal security reviews only ([docs/security-reviews.md](docs/security-reviews.md)).

## Layout

```
Cargo.toml                         workspace: three crates, version 0.5.0, shared dependency versions, unsafe_code = "forbid"
Cargo.lock                         locked dependency versions (CI builds with --locked)
deny.toml                          cargo-deny: allowed licences, advisories, bans, sources
LICENSE                            Apache-2.0
NOTICE                             the credit line "Created by Phygital-Key" (must stay)
README.md                          front page: what Phygital Key is, the products, what the base holds
CHANGELOG.md                       changes to the base
CONTRIBUTING.md                    ground rules and the commands to run
SECURITY.md                        how to report a security problem privately
CITATION.cff                       how to cite the project
PRODUCT.md                         product and brand notes for design work
.gitignore                         build output, node_modules, logs
.github/workflows/ci.yml           CI on every push to main and every pull request (jobs listed below)
.github/workflows/codeql.yml       CodeQL static analysis
.github/workflows/scorecard.yml    OpenSSF Scorecard
.github/workflows/fuzz-weekly.yml  each fuzz target for 30 minutes every Sunday
.github/dependabot.yml             dependency update pull requests
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
  src/bound.rs                     agent-bound approvals (pka.2): the agent's proof, Binding, sign_proof
  src/webauthn.rs                  the small part of WebAuthn a verifier needs: one ES256 assertion
  src/history.rs                   signed key history: verify_history, verify_history_update, fingerprints
  src/standing.rs                  standing approvals, use tokens, revocations, verify_use
  src/receipt.rs                   service receipts (compact JWS, typ pka-receipt+jwt): sign, verify
  examples/basic.rs                verify the first shared vector, the way a service would
  tests/vectors.rs                 runs every shared test vector
  tests/props.rs                   property tests: random input never verifies, any change is refused
  tests/units.rs                   small exact checks added where mutation testing found gaps
  tests/edges.rs                   exact edges from the 2026-10-18 mutation run
  tests/wycheproof.rs              Google's Wycheproof P-256 cases (DER and raw), plus high-S

crates/phygital-key-gate/          the gate engine every product shares (a library, no binary)
  Cargo.toml                       crate manifest (axum, tokio, clap, RustCrypto)
  src/lib.rs                       public API: run, Product, Adapter, Gate, Settings
  src/product.rs                   Product (name, bin, id, version, repo, port, about, release_keys), the Adapter trait, and the templates filled per product (product_files)
  src/cli.rs                       the shared commands (clap), settings, env names from the product, serve, stdio, demo and the rest
  src/state.rs                     the gate's state: requests, statuses, keyring, enrollment, standing approvals, revocations, receipts, the access token
  src/web.rs                       the approval page's HTTP routes and JSON API, Host and Origin checks, CSP and other headers, the accept loop with a 10 s header timeout
  src/demo.rs                      the demo service: verifies approvals like a real service, signs receipts
  src/audit.rs                     append-only, hash-chained audit log
  src/seen.rs                      file seen store for `verify` (lock file, temp file and rename)
  src/update.rs                    signed releases: strict release.txt and release2.txt parsers, P-256 and ML-DSA-65 checks against the product's pinned keys, expiry, `update`, release-sign-pq, release-keygen-pq
  src/util.rs                      time, randomness, dates, owner-only files
  assets/index.html                home page: trusted keys, fingerprint, remove buttons
  assets/approve.html              approval page for one request
  assets/enroll.html               add-a-key page
  assets/app.js                    the pages' script (no framework, no third-party code)
  assets/style.css, fonts.css      page styles (dark and light) and font faces, served by the gate itself
  assets/fonts/*                   Anybody, Hanken Grotesk and JetBrains Mono (woff2) with their OFL licence files
  assets/brand/*.svg               logo marks used by the pages
  assets/install-service.sh        template: the secure installer (system user, launchd or systemd, daily signed update job, admin helper)
  assets/install.sh                template: the user installer (signed release list and SHA-256 checked, writes ~/.local/bin/<program>)
  assets/secure-install-test.sh    template: the secure-install CI test (36 checks on Linux, 32 on macOS), published by a product as tests/secure-install.sh
  assets/release-key.sh            template: one-time release key setup, published as scripts/release-key.sh
  assets/package-manifests.sh      template: Homebrew, Scoop and winget manifests, published as scripts/package-manifests.sh
  tests/release-lists.sh           the secure installer's release-list check against lists signed with a test key (22 checks)
  tests/update-job.sh              the daily job's standing-approval wait, port takeover and gate-listener check, with stand-ins (22 checks)

crates/phygital-key-testgen/       writes the shared test vectors
  Cargo.toml                       crate manifest
  src/main.rs                      deterministic generator: fixed TEST ONLY keys, RFC 6979 signatures

test-vectors/                      the shared vectors (part of the spec)
  README.md                        vector format
  vectors.json                     approval cases
  keyrings.json                    key-history cases
  standing.json                    standing-approval cases
  keys/test-keys.json              TEST ONLY private keys (public on purpose; never trust them)
  wycheproof/                      three Wycheproof files copied unchanged (P-256 DER and raw, ML-DSA-65), with a README; not part of the spec

verifier-js/                       the independent JavaScript verifier (Node 20+ and browsers)
  package.json, README.md, LICENSE, NOTICE
  src/index.js                     public API
  src/verify.js                    the checks of spec section 7
  src/strict.js                    strict JSON and base64url
  src/canonical.js                 canonical form and SHA-256
  src/history.js                   signed key history
  src/standing.js                  standing approvals, use tokens, revocations
  src/bound.js                     agent-bound approvals: the proof check and signProof
  test/vectors.test.js             9 tests: every shared vector, single use, strict parsing, and that async seen and use stores are awaited
  test/wycheproof.test.js          2 tests: every Wycheproof P-256 case through the checker's own signature handling

crosscheck/                        SimpleWebAuthn must agree with our verifiers on every approval vector (npm test, dev only)

fuzz/                              cargo-fuzz targets (nightly)
  Cargo.toml, .gitignore
  fuzz_targets/verify.rs           random and mutated approvals
  fuzz_targets/strict_json.rs      strict parser and canonical form
  fuzz_targets/keyring_history.rs  random keyrings
  fuzz_targets/standing.rs         random standing approvals and use tokens
  fuzz_targets/proof.rs            random proofs for a bound approval

examples/                          runnable services, each run against a product's demo (CI checks the code builds and the YAML parses)
  README.md                        what each example shows and how to run it
  node-service/                    a deploy service in plain Node, checked with the JavaScript verifier
  rust-service/                    a database worker that checks approvals with phygital-key-verify (its own Cargo.toml, not in the workspace)
  ci-approval/                     a GitHub Actions deploy job that checks the approval from the command line

docs/                              the shared guides (plain words, GitHub Markdown)
  spec.md                          the format and the checks (normative, with the vectors)
  decisions.md                     numbered decisions and why
  reference.md                     commands, routes, files, limits, names per product
  architecture.md                  the parts, and how the base and the products fit together
  threat-model.md                  what it defends against and what it does not
  secure-install.md, updates.md    the secure install, and signed automatic updates
  testing.md, security-reviews.md  every test suite with its numbers, and the internal reviews
  *.md                             the other guides (getting started, integrate, how it works, error codes, glossary and more)
  images/*.png                     screenshots of the real pages
  brand/*.svg                      logos
```

What lives in a product repository instead: its `main.rs` (the `Product` and the `Adapter`), its protocol code (for MCP, the six tools and their instructions in `src/mcp.rs`), `release-keys.pem`, the files written by `product-files` (`install-service.sh`, `install.sh`, `tests/secure-install.sh`, `scripts/release-key.sh`, `scripts/package-manifests.sh`), its end-to-end tests (`tests/e2e/`), its container (`packaging/container/`), its workflows (`ci.yml`, `release.yml`, `sync-base.yml`) and its product-only docs.

## Rules that must never break

These are the product. A change that weakens one is a security bug.

1. **Edit shared behaviour here, never in a product.** The approval page, keys, standing approvals, receipts, the audit log, the secure install, updates, the shared commands and the templates live only in this repository. A product adds its agent protocol and its name, nothing else. Never copy base code into a product to change it there, and never edit a file in a product that `product-files` writes: change the template here, and the product picks it up through `sync-base`.
2. **Test vectors first.** A change to what a verifier accepts or refuses needs new or changed vectors before code: change `crates/phygital-key-testgen`, regenerate, then change the code until they pass. Never edit the vector JSON by hand, and never change a vector to make code pass.
3. **Both verifiers, every time.** The Rust and JavaScript verifiers share only `docs/spec.md` and `test-vectors/`. A change in one lands in the other in the same pull request, and both pass every vector. Never make one import or copy from the other.
4. **No LLM in the gate or the approval page, ever.** No model calls, no AI-written text on the page. The page shows the canonical request built by the gate, never prose from the agent.
5. **Nothing can approve.** The gate's API for adapters can ask, wait, cancel, record a receipt and use a standing approval. Only a signature from the person's trusted key makes an approval, and the gate checks it fully before it keeps it. Do not add an adapter hook, tool, flag, route or test hook that approves, signs for the person or skips the check.
6. **Fail closed.** Anything unclear, malformed, unknown or out of range is refused. Unknown fields are refused. Text that is not NFC is refused, never normalized. Floats are refused. A restart forgets pending requests and ends standing approvals.
7. **No home-made crypto.** Use the RustCrypto crates already in the workspace (`p256`, `sha2`, `subtle`), `serde_jcs` and `unicode-normalization`; in JavaScript only WebCrypto. Compare secrets, hashes and challenges in constant time.
8. **No new dependencies without asking.** Open an issue first. The licence must be Apache-2.0, MIT, BSD or similar (`deny.toml`), and the crate must be maintained. `cargo deny check` must pass.
9. **Never log or print secrets.** The enrollment token is shown only by `enroll` (and the demo), never in `stdio` or `serve` output. The audit log never stores the request itself. The access token for the agent endpoint lives only in `<home>/access-token` (0600).
10. **The verifier decides.** Audience, the exact request and the level are set by the service, never taken from the requester. Keep `verify` pure: no network, no clock reads, no global state except the seen store.
11. **Reserved names stay reserved.** Request types `standing` and `revoke`, and the audience `phygital-key:keyring:<origin>`, are made only by the gate.
12. **Release keys live only in each product's GitHub environment secret.** The base has no release key and makes no releases. Each product's private key is `RELEASE_SIGNING_KEY` of the environment `release` in that product's repository, which GitHub gives only to a release run from a `v*` tag; only repository admins can push `v*` tags, and for now no approval click is needed (decision 36). Only the public half is in a product, in its `release-keys.pem` (compiled into the program) and the `# BEGIN RELEASE KEYS` blocks of its installers. Never put a private key anywhere: not in a commit, a workflow, an issue, a test or a log. Never run a product's `scripts/release-key.sh`, never approve a release run or a deployment, never push a `v*` tag unless the owner asked, and never add a secret, job or step that could sign.
13. **Keep the credit.** `NOTICE` and `verifier-js/NOTICE` keep the line "Created by Phygital-Key". Never write any real person's name anywhere, and add no AI attribution lines to docs, commits or release notes.
14. **Docs in plain words, no em dashes.** Not one em dash in any doc, page text or tool message. Follow the existing voice: short sentences, exact facts, every command one you ran. Say "pre-audit" until an outside security review exists. Shared guides live only here; product repositories link here for them. Relative links must resolve.

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

Fuzzing (nightly Rust and `cargo install cargo-fuzz`):

```bash
cd fuzz && cargo +nightly fuzz run verify -- -max_total_time=60
```

The other targets are `strict_json`, `keyring_history` and `standing`.

The base has no program, so the end-to-end checks, the secure-install test and the container check run in each product. To try a base change in a product before you merge it, build the product against your checkout (for Phygital Key MCP, see its `CONTRIBUTING.md` and `tests/e2e/`).

CI (`.github/workflows/ci.yml`) runs on every push to `main` and every pull request:

| Job | What it runs |
|---|---|
| test (Linux, macOS, Windows) | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, then regenerates the vectors with `cargo run -q --release -p phygital-key-testgen --locked -- test-vectors` and fails on `git diff --exit-code test-vectors` |
| JavaScript checker | `npm test` in `verifier-js` on Node 24 |
| examples (build and syntax) | `node --check examples/node-service/server.mjs`, `cargo build --manifest-path examples/rust-service/Cargo.toml`, parses `examples/ci-approval/example-deploy.yml` |
| fuzz | each of the 4 targets for 60 seconds on nightly |
| licenses and advisories | `cargo-deny` |
| scripts and workflows | ShellCheck on the installer templates and test scripts, `crates/phygital-key-gate/tests/release-lists.sh`, zizmor on the workflows |
| cross-check with SimpleWebAuthn | `npm ci` and `npm test` in `crosscheck` |

`codeql.yml` and `scorecard.yml` run CodeQL and OpenSSF Scorecard; `fuzz-weekly.yml` fuzzes each target for 30 minutes every Sunday. Third-party actions are pinned to a commit; keep them pinned.

## How a base change reaches the products

Each product depends on `phygital-key-gate` and `phygital-key-verify` from this repository's `main` branch, and its `Cargo.lock` pins the exact commit. Its daily `sync-base` workflow runs `cargo update -p phygital-key-gate -p phygital-key-verify`, builds, runs `product-files --out .`, and, if anything changed, runs the product's tests and end-to-end checks. Only when they pass does it commit, push to the product's `main` and start the product's full CI. So a change merged here is live in a product's `main` within a day, and in its users' hands with that product's next signed release.

## Changing the spec safely

The approval format is used by services you cannot see. An approval made by one version must mean the same thing to every verifier.

1. **Ask first** for anything that changes signed bytes or their meaning: claims fields, the challenge prefix, the canonical form, the use-token or receipt headers, error codes. Open an issue and describe the case.
2. **Write the spec change** in `docs/spec.md` (and a numbered entry in `docs/decisions.md`: what you chose and why).
3. **Add vectors first** in `crates/phygital-key-testgen/src/main.rs`, one per new edge (exact limits, one past the limit, the attack it stops). Regenerate with `cargo run -p phygital-key-testgen -- test-vectors` and run it twice: the output must be byte for byte the same.
4. **Watch both verifiers fail** on the new vectors (`cargo test --workspace`, `cd verifier-js && npm test`).
5. **Change both verifiers** until every vector passes in both.
6. **Check the gate and the page** if they build or show what changed (`crates/phygital-key-gate/src/state.rs`, `assets/app.js`), and add an end-to-end check in each product's `tests/e2e/run.mjs` for anything an agent could try.
7. **Update the docs** in the same pull request: `docs/reference.md`, the user guides that mention it, and `CHANGELOG.md`.

Changing what a product's agent tools say (for MCP: tool names, arguments, results, `next` texts) is done in that product's repository, with its own docs and end-to-end checks.

## Next

- [CONTRIBUTING.md](CONTRIBUTING.md): ground rules and checks.
- [docs/architecture.md](docs/architecture.md): the parts, and the base and product split.
- [docs/spec.md](docs/spec.md): the format and every check.
- [SECURITY.md](SECURITY.md): report a security problem privately.
