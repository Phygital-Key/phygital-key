# Security reviews

This page lists every security review of Phygital Key so far, what each one found, and what was changed. It is for anyone judging how far to trust the project, and for researchers who want to know what has already been looked at.

> [!IMPORTANT]
> Phygital Key is **pre-audit**. The six reviews below were done by AI security-auditor agents, not by a paid third-party audit firm. They found real problems, and every confirmed finding was fixed, but they are no substitute for an outside audit.

**On this page**

- [What the reviews were, and were not](#what-the-reviews-were-and-were-not)
- [Review 1: before the v0.1.0 security fixes](#review-1-before-the-v010-security-fixes)
- [Review 2: v0.2.0 work](#review-2-v020-work)
- [Review 3: standing approvals](#review-3-standing-approvals)
- [Review 4: the review 3 fixes](#review-4-the-review-3-fixes)
- [Review 5: v0.3.0](#review-5-v030)
- [Review 6: the review 5 fixes](#review-6-the-review-5-fixes)
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

## The documentation review

While this documentation was written for v0.3.0, its pages were checked claim by claim against the code. That is not one of the six security reviews, but it found one fail-open bug and several places where the program and its docs disagreed. All were fixed in the code before the release (see the 0.3.0 entry in the [changelog](../CHANGELOG.md)).

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

Nothing confirmed by a review is open. These remain, by design, by decision or because they cannot be tested yet. They are listed in the [threat model](threat-model.md#known-limits):

- [ ] An outside, paid security audit (the project stays labeled pre-audit until then)
- [ ] Approvals are signed, not encrypted (decision 33)
- [ ] An approval is a bearer for its life (60 s by default, at most 300 s); binding it to the requester's key is not built
- [ ] Use tokens already handed out work for up to 60 s after a revoke, until the service picks up the signed revocation
- [ ] No hardware-attestation check (decision 35)
- [ ] The audit log detects edits, not a full rewrite or a cut-off end
- [ ] The limit of 50 waiting requests has no automated test (the 30-a-minute limit now has one: e2e "the 31st request in a minute is refused (flood limit)")
- [ ] No secure install for Windows
- [ ] The download path is untested in CI: CI installs the secure install from a root-owned local build (`--binary`), so downloading a published release, checking its signed `release.txt` and its SHA-256, and a real automatic update have not run yet. They can run only once signed releases exist (the first real update test is the release after v0.3.0).
- [ ] The release keys are made once by the maintainer (`scripts/release-key.sh`). Until they are pinned in the installers, every download and update is refused.
- [ ] Automatic updates trust the release key: a signed release that starts is installed, and a held-back update (a freeze) cannot be seen from the machine. See [the update channel](threat-model.md#the-update-channel).

## How to report

Please do not open a public issue. Use [private vulnerability reporting](https://github.com/s17sh70x2jfl031ag/phygital-key-mcp/security/advisories/new). A failing test vector is the most helpful report. Details, scope and what to include are in [SECURITY.md](../SECURITY.md).

## Next

- [Threat model](threat-model.md): attackers, defences and the tests that prove them
- [Testing](testing.md): every test layer, with numbers
- [Decisions](decisions.md): why each design choice was made
