# Roadmap

What is done, what was decided against for now (and why), and what might come later. Nothing here has a date, and nothing under "maybe later" is promised. To ask for something, open a [feature request](https://github.com/Phygital-Key/phygital-key-mcp/issues/new?template=feature_request.yml), or help build it ([CONTRIBUTING.md](../CONTRIBUTING.md)).

## Done

| Version | Headline |
|---|---|
| **v0.3.0** | Signed automatic updates: every release carries a release list (`release.txt`, file names and SHA-256 sums) signed with an ECDSA P-256 release key that only a release run approved by the repository owner can use; the secure install checks it once a day, installs only a newer signed release and switches back if the new gate does not start (on by default, `--no-auto-update` turns it off); `phygital-key update` for installs in your own folders. Secure install: `sudo phygital-key install-service` runs the gate as its own locked-down system user (launchd on macOS, systemd with hardening on Linux), tested for real on both in CI. Signed revocations for standing approvals (new code `REVOKED`). An access token on the HTTP MCP endpoint, on by default. An independent JavaScript checker with no dependencies that passes every shared test vector, with sync or async (database) seen stores and use stores. Three more security reviews (5, 6 and 7), every finding fixed or documented as a limit. |
| **v0.2.0** | Standing approvals: approve once, with limits, for up to 30 days; one-minute use tokens; new codes `NOT_ALLOWED` and `USED_UP`. A signed key history, checked from one fingerprint, with rollbacks and forks refused. Used approvals remembered by `phygital-key verify`. Signed receipts from services. Fuzzing and mutation testing. Homebrew, an install script that checks the SHA-256, and Scoop. |
| **v0.1.0** | The spec (`pka.1`) and shared test vectors. The Rust verifier library (`phygital-key-verify`). The gate: MCP over stdio and HTTP, the approval page, one-time enrollment links, adding a key only with an existing key's approval, a hash-chained audit log, `phygital-key verify` and the one-command demo. End-to-end and red-team tests with a real browser and a virtual passkey. |

The full list is in `CHANGELOG.md` at the root of the repository.

## Decided: not now

Each of these was thought through and left out on purpose. The decision number points to the reasoning in [decisions](decisions.md).

| What | Why not now | Decision |
|---|---|---|
| End-to-end encrypted requests | The approval page is served by the gate, so encrypting a request against the gate only helps when the page comes from somewhere the gate cannot change. On your own computer, the AI is already on that machine. It belongs with a hosted gate, if one is ever built: a pinned approval page plus a device-held decryption key. Until then approvals are signed and verified, not secret. | [33](decisions.md) |
| Hardware attestation check | Proving "certified hardware key" means checking certificate chains against FIDO's metadata, code where mistakes are common, for a setting few would use. Phone and laptop passkeys never provide the proof. A service that wants only hardware keys can trust only those keys in its keyring today. | [35](decisions.md) |
| AP2 / SD-JWT export | For a plain AP2 verifier, trust would move from the person's passkey to a gate key, which would then be a valuable target. The mapping is written down in [ap2.md](ap2.md); the code waits for someone who needs it. | [28](decisions.md) |

## Maybe later

| What | Where it stands today |
|---|---|
| **2 of 3 approvers** (several people sign one approval) | The format already holds a list of signatures, so no format change is needed. Version 0.3.0 requires exactly one. Two people can already approve the same request as two separate approvals. |
| **Secure install on Windows** | The gate, the checker and the demo run on Windows. The separate-user install exists for macOS (launchd) and Linux (systemd) only. |
| **More adapters** (plain HTTP API, email link, chat apps, a QR code on a screen) | MCP is the only way a request reaches the gate. An outside party or a device without an MCP client cannot send a request on its own. |
| **Packages on crates.io and npm** | The Rust library installs from the git repository, and the JavaScript checker lives in the `verifier-js/` folder ([verifier-js](verifier-js.md)). Neither is published to a registry. |
| **An A2A (Agent2Agent) version**, carrying approvals inside A2A messages | Not built. An approval is a signed string that can already travel in any message, and the checkers run in any agent; what is missing is an A2A adapter. |
| **An outside security audit** | Pre-audit. Seven internal security reviews (AI security auditor agents) were run and every finding fixed or documented as a limit; no paid outside firm has reviewed it. See [security reviews](security-reviews.md). |
| **Binding an approval to the requester's key** (`cnf`) | An approval is a bearer token for its life (60 seconds by default, 300 at most), bound to one request and one audience. Binding it to the agent's own key is an open question in the [spec](spec.md). Standing approvals already bind each use to a key the gate holds (`use_key`). |
| **Templates for common request types** (`deploy`, `payment`, `send_email`) | The page shows every canonical field for every type. Friendlier layouts per type are an open question in the [spec](spec.md). |

## Next

- [Decisions](decisions.md)
- [Use cases](use-cases.md)
- [Questions and answers](faq.md)
- [Security reviews](security-reviews.md)
