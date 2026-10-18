# Security reviews

This page lists every security review of Phygital Key so far, what each one found, and what was changed. It is for anyone judging how far to trust the project, and for researchers who want to know what has already been looked at. Reviews 1 to 7 looked at the single program `phygital-key`, before the split into the base and the products in 0.4.0; file names, commands and e2e check names in them are as they were then. The reviewed gate code now lives in `crates/phygital-key-gate` and is the same in every product; the MCP tools moved to Phygital Key MCP. Review 8 covers the split and the A2A adapter.

> [!IMPORTANT]
> Phygital Key is **pre-audit**. The eight reviews below were done by AI security-auditor agents, not by a paid third-party audit firm. They found real problems, and every confirmed finding was fixed or, where a fix is not built, documented as a limit. They are no substitute for an outside audit.

**On this page**

- [What the reviews were, and were not](#what-the-reviews-were-and-were-not)
- [Review 1: before the v0.1.0 security fixes](#review-1-before-the-v010-security-fixes)
- [Review 2: v0.2.0 work](#review-2-v020-work)
- [Review 3: standing approvals](#review-3-standing-approvals)
- [Review 4: the review 3 fixes](#review-4-the-review-3-fixes)
- [Review 5: v0.3.0](#review-5-v030)
- [Review 6: the review 5 fixes](#review-6-the-review-5-fixes)
- [Review 7: signed automatic updates, JS stores, gate changes](#review-7-signed-automatic-updates-js-stores-gate-changes)
- [The documentation review](#the-documentation-review)
- [Problems CI found](#problems-ci-found)
- [What is still open](#what-is-still-open)
- [How to report](#how-to-report)

## What the reviews were, and were not

| | |
|---|---|
| Who | Independent AI security-auditor agents, each started fresh |
| Access | Read-only access to the code, plus the [spec](spec.md) and the [threat model](threat-model.md) |
| Task | Find exploitable issues against the stated guarantees |
| Outcome | Every confirmed finding fixed before the next release, with a test or [test vector](../test-vectors/README.md) added where possible |
| Not | A paid audit, a third-party audit, a certification, or a formal proof |

Each review below lists findings in the order the reviewer ranked them. Severity words (HIGH, MEDIUM, LOW) are the reviewer's own.

## Review 1: before the v0.1.0 security fixes

Date: 2026-10-17. Result: nothing critical; no way found to forge, replay or redirect a yes. Five confirmed issues, all fixed.

| # | Finding | Fix | Proof now |
|---|---|---|---|
| 1 | NFC normalization let different bytes share one approval. KELVIN SIGN (U+212A) folds to `K` and U+037E to `;`, so an approval to pay "Kirk" could pay "Kirk" spelled with a Kelvin sign. | Text that is not NFC is refused, never rewritten (decision 17). | Vectors `nfd_request_refused`, `kelvin_sign_payee_refused`, `non_nfc_claims_refused`; e2e "look-alike KELVIN SIGN payee refused at the gate (not NFC)" |
| 2 | Enrollment does not prove the public key comes from real hardware (attestation `none`). | Documented: the one-time link protects the first key; later keys need approval by a trusted key. | [Threat model, known limits](threat-model.md#known-limits) |
| 3 | In stdio mode the enrollment link went to the AI app's logs. | Never printed in `mcp` mode (decision 19). | e2e "stdio mode does not print an enrollment link" |
| 4 | Deep nesting made a request signable but never verifiable. | Depth checked when the request is created. | e2e "too deeply nested request refused before the person sees it"; vector `too_deep` |
| 5 | A local program could restart the signing step and swap claims mid-signature. | Claims stay fixed while valid (decision 20). | e2e "claims cannot be swapped while the person signs" |

Hardening added at the same time: audiences are visible ASCII only, wider look-alike highlighting on the page, Origin check on `/mcp`, no CORS on the localhost key list, a timer that prunes old requests, and the enrollment token used up at the first step.

## Review 2: v0.2.0 work

| # | Severity | Finding | Fix | Proof now |
|---|---|---|---|---|
| 1 | HIGH | Key-history rollback and fork: an old copy, or a branch signed by a removed key, passed the check. | `verify_history_update` (a refetch must extend the accepted copy), origin pinned by the checker, `iat` order, whole-card compare (decision 25). | Vectors `rollback`, `fork_by_removed_key`, `relabeled_card`, `wrong_origin`, `valid_update`; e2e "an older copy of the key list (rollback) is caught", "a key list signed on another gate is refused" |
| 2 | MEDIUM | Receipts supplied by the agent were shown unverified. | Recorded only if signed by a pinned service key; exact header bytes; the receipt must name the same approval id, request and service (decision 26). | e2e "a receipt the agent made up is refused (signature checked)", "receipt for another approval is refused" |
| 3 | MEDIUM | Removing the same key twice broke the signed history. | Refused when requested and when approved. | e2e "a second removal of the same key is refused while one waits" |
| 4 | MEDIUM | The used-ids file could forget ids after a crash. | Lock file, temp file and rename; a damaged file refuses everything. | Rust test `remembers_until_expiry_and_fails_closed` |
| 5 to 8 | LOW | Relabeled cards, origin read from the file, loose receipt checks, keyring saved before memory switched over. | All fixed. | Vectors `relabeled_card`, `wrong_origin`; Rust test `round_trip_and_refusals` |

## Review 3: standing approvals

Result: no way found to act outside what the person approved. Fixed:

| Finding | Fix | Proof now |
|---|---|---|
| A range with only a `max` allowed negative amounts | Integer limits need both `min` and `max` | Vectors `range_without_min`, `negative_amount`; e2e "a range without a minimum is refused (no negative amounts)" |
| Standing approvals were forgotten after about 70 minutes | Kept until their end | |
| A standing request could be disguised as a normal one | The `standing` type is reserved | Vector `standing_as_one_time`; e2e "a standing request disguised as a normal one is refused" |
| Receipts did not work for standing uses | One receipt per use | Rust test `use_receipts_are_bound_to_the_use`; e2e "receipt for one use of a standing approval is recorded" |

Hardening: standing approvals are always level `high` in the verifier (vector `standing_medium_never_enough`), stores keep ids 30 s past expiry against clocks that step back, safer seen-file paths, atomic standing setup, and the page checks the signed end date.

## Review 4: the review 3 fixes

| Finding | Fix |
|---|---|
| Unapproved standing requests could fill memory for 30 days | Forgotten an hour after their deadline; a cap of 5000 held requests |
| A per-use receipt check accepted a receipt with no use id | The use id is required (Rust test `use_receipts_are_bound_to_the_use`) |

## Review 5: v0.3.0

Scope: the MCP access token, signed revocations, the JavaScript checker and the secure install.

| # | Severity | Finding | Fix | Proof now |
|---|---|---|---|---|
| 1 | HIGH | The gate listened on `127.0.0.1` only. Another program could open `[::1]` on the same port and serve a look-alike page with the same passkey origin. | The gate holds both addresses and refuses to start if either is taken; in `mcp` mode the page and MCP stop together. | Secure install "refuses while another program holds the address", "only the gate user listens (IPv4 and IPv6)" |
| 2 | MEDIUM | The JavaScript checker accepted a level like `toString` (a built-in of JavaScript objects). | Own-property check. | JS test "a level that is a JavaScript built-in name is refused" |
| 3 | MEDIUM | A person's signed revocation could be blocked (limits filled, deny, timeout). | Revocations bypass the limits, can be retried, and stay one per standing approval. | e2e "the signed revocation is published" covers the normal path; no test fills the limits first |
| 4 | MEDIUM | `sudo phygital-key install-service` installed and ran a file you (and so an agent) could have changed. | The install downloads and checks the official release itself (now against a signed release list, decision 36), installs into root-only folders and refuses user-owned binaries. The recommended path is the pinned script from a trusted terminal. | Secure install "refuses a binary the user could have changed", "program owned by root", "this user cannot change the program" |
| 5 | LOW | The JS revocation reader threw on junk; a byte-order mark was accepted; a damaged `revocations.json` was overwritten; removing a key silently un-revoked. | Fixed, or documented (services keep every revocation they accepted). | JS tests "junk in a revocation list is ignored, never thrown", "a byte-order mark before the approval JSON is refused" |

Hardening: no secrets in `serve` logs, `Bearer` accepted in any case, a cap of 1000 revocations read, `verify()` refuses `revoke`-type requests, exact use-token header bytes, more systemd sandboxing.

## Review 6: the review 5 fixes

| Finding | Fix |
|---|---|
| Revocation re-requests could flood memory | One revocation request per standing approval, re-armed, never duplicated |
| The Linux listener check compared truncated user names | Compares the uid |
| Relative `--binary` paths | Refused (secure install "refuses a relative --binary") |
| Log folder handling | Fixed |
| Version and port input | Checked |
| Tarball extraction | Made safe |
| `launchctl bootstrap` | Retried |
| One listener could keep serving when the other stopped | Listeners stop together, and on SIGTERM (e2e "stdio mode stops cleanly when told to (SIGTERM)") |
| No way to update to a named release | `--release` added |

## Review 7: signed automatic updates, JS stores, gate changes

Scope: the signed automatic updates and the release process, the async seen stores and use stores of the JavaScript checker, and the gate changes made for v0.3.0. Result: 2 high, 3 medium, 3 low and 4 informational findings, each fixed or documented. After the review, the owner moved the release key into GitHub (decision 36), which supersedes the fixes for M2 and M3.

| # | Severity | Finding | Fix | Proof now |
|---|---|---|---|---|
| H1 | HIGH | Root used the caller's environment. On macOS `sudo` keeps `HOME` (and `PATH` when no `secure_path` is set), so `curl ... \| sudo bash`, `sudo <prefix>/bin/phygital-key install-service` and `phygital-key-admin update` ran root's `curl` with the user's `~/.curlrc` (which can make curl write any file as root), and could run a `curl`, `awk` or `openssl` planted in a folder the user can write early in `PATH` (such as `/opt/homebrew/bin`). This was already true of the v0.3.0 install path before automatic updates. | `install-service` starts bash with an empty environment (system `PATH`, root's `HOME`, `LANG=C`, the `PKA_*` values). The script first sets that `PATH` and `HOME`, clears `CURL_HOME`, `XDG_CONFIG_HOME`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `CURL_CA_BUNDLE`, `OPENSSL_CONF`, `OPENSSL_MODULES`, `BASH_ENV`, `ENV`, `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_INSERT_LIBRARIES`, `DYLD_LIBRARY_PATH`, and sets `umask 022` (this covers `curl ... \| sudo bash` too). Every `curl` starts with `-q`, also in `phygital-key update`. The admin helper sets a system `PATH` before any `sudo`. | Secure install "root ignores the user's ~/.curlrc", "root ignores programs early in the user's PATH" |
| H2 | HIGH | An update restart frees the gate's port for a moment. A program running as the user could take it, and the update then stopped the gate and left that program holding `http://localhost:<port>` with nobody watching. | During the unattended update only, a program (not the gate user) found listening on the port is killed (`kill -9`) and the gate started again, up to 5 times; the result line ends with `; stopped programs that took the port of the gate during the restart: [uid U, pid P]`. If it still fails, the gate is stopped (fail closed) and the status says so. Manual installs keep refusing with the message, because the person is there. The daily job's plist on macOS is now 0600, so other users cannot read its time. Documented limit: passing the open port from launchd or systemd to the gate is not built (the gate forbids unsafe code), so a fast program could win the moment again, and would be killed again. | The code (`gate_up` in `install-service.sh`); no CI check |
| M1 | MEDIUM | An agent could hold back updates by keeping one request waiting. | The daily job waits only for active standing approvals, which need a real passkey approval. The 7-day clock starts at the first wait on the installed version, and a newer release does not restart it (`<prefix>/update-postponed` holds `<unix time> <installed version>`). Waiting requests are lost on an update restart, and the agent asks again. Documented limit: a person's active standing approval can delay a security update by up to 7 days. | The code (step 0 of `install-service.sh`); no CI check |
| M2 | MEDIUM | The release key signed whatever files GitHub held for the draft, so a draft file could be swapped before signing. | First fixed with a key on the owner's computer: a signing script checked each file's build provenance attestation against the owner's own clone of the tag before signing. Status: superseded: owner chose GitHub-held key with approval (the approval step is off for now, decision 36); see [threat model](threat-model.md#the-update-channel). There is no draft any more: the release run hashes and signs the files it built itself. | `environment: release` in `.github/workflows/release.yml`; runs only at a real release |
| M3 | MEDIUM | The key passphrase sat in the macOS Keychain, readable without a prompt by any program running as the maintainer (an agent included). | First fixed with no Keychain and a passphrase typed at each signing. Status: superseded: owner chose GitHub-held key with approval (the approval step is off for now, decision 36); see [threat model](threat-model.md#the-update-channel). There is no key file, passphrase or Keychain entry on any computer: the key is only the GitHub environment secret `RELEASE_SIGNING_KEY`, given only to runs from `v*` tags. | `scripts/release-key.sh` (deletes its local copy); not testable in CI |
| L1 | LOW | The switch back to the previous program was skipped when the install refused after the swap. | Covered by the H2 fix for unattended updates: the program on the port is stopped and the gate started again. An install you run yourself still stops the gate and tells you why. | As H2 |
| L2 | LOW | `phygital-key update` could write the new program through a file or link already at its staging path. | It writes to a freshly created file (`create_new`, mode 0755), never through a file or link that was there. | The code (`update.rs`); no automated test |
| L3 | LOW | `audit-check` passed when `keyring.json` existed but could not be read or was damaged. | It fails with `<path>: damaged keyring (...)` (or the read error). Only "no log and no keyring" (or a keyring with no keys) passes, with `no audit log yet: nothing has happened on this gate`. | Run by hand; no automated test |
| I | INFO | The shell (`openssl`) path checked `release.txt` less strictly than the program. | The secure installer applies the same strict format: LF only, a final LF, at least one file line, exact `<64 hex>  phygital-key-<version>-<name>.tar.gz` or `.zip` lines, no name twice. | The code (`verify_release` in `install-service.sh`) |
| I | INFO | No release key was pinned yet, so every install and update was refused (fails closed). | The owner makes the key with `scripts/release-key.sh` before the first signed release; a test fails while the key block is empty. | `installer_pins_at_least_one_release_key` |
| I | INFO | High-S ECDSA signatures are accepted. | No change: no security effect. | |
| I | INFO | A freeze (an old signed list served forever) cannot be seen, because the signed list has no expiry. | Documented limit ([threat model](threat-model.md#the-update-channel)). | |

## Review 8: the base and product split, the A2A adapter, the sync workflow

Scope: the split into the shared base and the two products, the new A2A 1.0 adapter, the installer and release scripts made from base templates, two products on one computer, and the daily base sync. Live tests with a real passkey on both products. Result: 0 critical, 0 high, 2 medium, 3 low and 4 informational findings. No way found to forge, replay, redirect or self-approve a yes through A2A, and an A2A task never reaches COMPLETED without the gate's approval string.

| ID | Severity | Finding | Fix |
|---|---|---|---|
| M1 | MEDIUM | The daily sync could commit any file the new base wrote into the product, with a write token present while the base's code ran; the quick installer was served from `main`. | The sync now builds and tests in a read-only job with no stored credentials; a second job saves only `Cargo.lock` and the five template files, refuses any other change and any change to `release-keys.pem`. `install.sh` and `install-service.sh` are attached to each signed release, and the docs install from `releases/latest/download`. |
| M2 | MEDIUM | An agent holding the A2A token could grow task histories, clone them in `ListTasks` and open hundreds of streams, until the gate ran out of memory (a restart ends standing approvals). | Follow-up messages keep only their text, capped; per-task and store-wide history caps; `ListTasks` copies only the page and omits history unless asked; at most 32 open streams. |
| L1 | LOW | With `--no-token`, any local program could list A2A tasks and read approvals. | The A2A release build refuses `--no-token`; the help text says what an open endpoint allows. |
| L2 | LOW | The installer did not check the folders above the data and log folders; a leftover 0.3 install could own them. | Every parent folder must be root's alone, and the installer refuses to run next to a 0.3 secure install, with the command to remove it. |
| L3 | LOW | Both products sign for the site name `localhost`. | Documented: one verifier, one keyring, one origin per gate. Every gate now refuses any `phygital-key:keyring:` audience, not only its own. |
| I1 to I4 | INFO | Product values not checked; public routes not enforced as read-only; a `tck` build could be mistaken for a release; one release key for both products would let one sign the other's lists. | Product values are checked at start and in the installers; public routes answer only GET and HEAD; the `tck` feature is never built for releases; each product makes its own release key. |

## The testing round (2026-10-18)

Not a review by a reading agent: outside test suites and tools run against the code, each finding fixed and turned into a test. Details and counts are on [testing](testing.md).

| Tool or test | What it found | Fix |
|---|---|---|
| Google's Wycheproof (746 P-256, 210 ML-DSA-65 cases) | Nothing: both verifiers pass every case | Kept as tests in both verifiers |
| SimpleWebAuthn, an independent WebAuthn library | Nothing: it agrees on every approval vector, except two cases where we are stricter on purpose | Kept as the `crosscheck` CI job |
| cargo-mutants (341 mutants on the verifier) | 14 checks no test pinned down: exact size limits, store memory at the exact second, revocation lists read from JSON, a keyring swapped into a live verifier, authenticator data one byte short | `tests/edges.rs`; a second run caught all 14 |
| A new secure-install check that starts the daily update job through systemd and launchd | **The daily update job never ran.** systemd and launchd give it no `HOME`, and the program stops without one. Running the same command by hand with `sudo` worked, which is all the old test did | The units set `HOME`; the test now starts the job the way the timer does |
| Abuse tests against a live gate | **Idle connections were never closed**, and launchd gives a service 256 open files, so a program on the same computer could keep the approval page from loading | A 10-second header timeout on every connection, idle ones included; 8192 open files |
| GitHub settings | Private vulnerability reporting was off in both products, although their security policies point to it | Turned on |
| zizmor (workflow analysis) | 0 high. Medium: Dependabot took new versions at once; the sync job kept git credentials on disk; a third-party action ran in the job that holds the release key; broad Scorecard permissions. The A2A product had no Dependabot file | All fixed; zizmor runs in CI |
| ShellCheck | No real bug; three unquoted names | Quoted; ShellCheck runs in CI |
| `systemd-analyze security` | The root update service had 2 sandbox settings | 16 now; CI keeps both units at their level |
| The installer's release-list check, run on its own (`tests/release-lists.sh`) | Three bugs in the new v2 code before it shipped: an empty array that stops macOS's bash 3.2, a missing memory file that stopped the script, and a `grep -q` in a pipe that could make a refused ML-DSA check look like an older program | Fixed before release |
| Firefox and WebKit, 10-minute fuzzing per target, SLSA provenance check | Nothing | Kept: browsers job, weekly fuzzing |

## The documentation review

While this documentation was written for v0.3.0, its pages were checked claim by claim against the code. That is not one of the security reviews, but it found one fail-open bug and several places where the program and its docs disagreed. All were fixed in the code before the release (see the 0.3.0 entry in the [changelog](../CHANGELOG.md)).

| # | Finding | Fix | Proof now |
|---|---|---|---|
| 1 | The JavaScript checker did not wait for an async seen store or use store (one backed by a database). The unresolved answer counted as "first use", so a replay could get through. | `verify` and `verifyUse` await both stores; only an exact `true` from `insertIfNew` means first use; `recordUse` must return a safe integer from 0 to below `maxUses`, or the result is `MALFORMED`; `revoked.has` may be async too. | JS tests "async stores are awaited: a database-style seen store still stops a replay", "async stores are awaited: a database-style use store still counts and stops reuse" |
| 2 | The 30-a-minute request limit had no test | An end-to-end check sends requests until one is refused, and checks that it is the 31st | e2e "the 31st request in a minute is refused (flood limit)", "the gate still answers after the flood limit" |
| 3 | The MCP server named itself after the MCP library, not after Phygital Key | Server name `phygital-key`, title `Phygital Key`, version = the program's version | e2e "MCP server names itself phygital-key with its version" |
| 4 | After approving a standing approval, the page and the `next` text read like a one-time approval | The page states the end date and the use cap; `next` explains `use_standing_approval` and both headers | e2e "after approving, the page gives the end date and use cap (no one-time wording)" |
| 5 | The page of a revoked standing approval did not clearly say that the revocation was signed | The page shows "Revoked" and that the service refuses it once it reads the revocation list | e2e "the revoked standing approval's page says revoked and signed" |
| 6 | `audit-check` passed when the audit log was missing | With a keyring present and no log it fails (exit 1): "no audit log at <path>, but a key is enrolled: the log was deleted or moved" | Run by hand; no automated test |
| 7 | `trust-service --key` did not take `@file` like the other key options | `@file` accepted | Run by hand; no automated test |

## Problems CI found

The automatic tests found real problems too:

| Found by | Problem | Fix |
|---|---|---|
| The macOS secure-install test | The installer accepted a gate after waiting out a program that held the port first | Listeners are checked on every attempt, with timeouts |
| The end-to-end test hanging | A stop signal left the stdio MCP side running | Fixed, and the check "stdio mode stops cleanly when told to (SIGTERM)" proves the program exits |

## What is still open

Nothing confirmed by a review is open without a fix or a documented limit. These remain, by design, by decision or because they cannot be tested yet. They are listed in the [threat model](threat-model.md#known-limits):

- [ ] An outside, paid security audit (the project stays labeled pre-audit until then)
- [ ] Approvals are signed, not encrypted (decision 33)
- [ ] An approval is a bearer for its life (60 s by default, at most 300 s); binding it to the requester's key is planned, not built
- [ ] The gate machine draws the approval page, and authenticators do not show what they sign: a gate machine under someone else's control could show one request and sign another. Approving on a second device that computes the challenge itself is planned, not built
- [ ] Use tokens already handed out work for up to 60 s after a revoke, until the service picks up the signed revocation
- [ ] No hardware-attestation check (decision 35)
- [ ] The audit log detects edits, not a full rewrite or a cut-off end
- [ ] The limit of 50 waiting requests has no automated test (the 30-a-minute limit now has one: e2e "the 31st request in a minute is refused (flood limit)")
- [ ] No secure install for Windows
- [ ] The download path is untested in CI: CI installs the secure install from a root-owned local build (`--binary`). Installing from a published, signed release was done by hand for both products, but a real automatic update to a newer release has not run yet, and the ML-DSA signing in a release run has been tried only locally with test keys.
- [ ] Each product's ML-DSA release key is made once by the owner (`scripts/release-key.sh --pq`). Until it is pinned, release lists carry the P-256 signature only.
- [ ] The gate's port is free for a moment at each update restart. A program that takes it is killed and the gate started again, but handing the open port from launchd or systemd to the gate is not built (review 7, H2). The kill-and-restart has no CI check.
- [ ] A person's active standing approval can hold back a security update for up to 7 days (review 7, M1). The waiting logic has no CI check.
- [ ] The fixes for review 7 L2 and L3 have no automated test.

## How to report

Please do not open a public issue. Use [private vulnerability reporting](https://github.com/Phygital-Key/phygital-key/security/advisories/new). A failing test vector is the most helpful report. Details, scope and what to include are in [SECURITY.md](../SECURITY.md).

## Next

- [Threat model](threat-model.md): attackers, defences and the tests that prove them
- [Testing](testing.md): every test layer, with numbers
- [Decisions](decisions.md): why each design choice was made
