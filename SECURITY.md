# Security policy

Phygital Key lets a person approve exact AI actions with a passkey or hardware key, and lets any service check that approval. A flaw here could let an action run without the person's yes, so reports are welcome.

This policy covers the base (this repository: the verifiers, the gate engine, the spec, the test vectors and the installer templates) and the products built on it, [Phygital Key MCP](https://github.com/Phygital-Key/phygital-key-mcp) and [Phygital Key A2A](https://github.com/Phygital-Key/phygital-key-a2a). Most of the code that guards an approval lives here, so report here unless the problem is only in one product's agent protocol.

Phygital Key is **pre-audit**: nine internal reviews by AI security-auditor agents and one round of outside test suites, every finding fixed or documented as a limit, and no paid third-party audit yet. See [docs/security-reviews.md](docs/security-reviews.md).

## Supported versions

Only the latest release of each product gets security fixes while the project is at 0.x. A base fix reaches each product's `main` within a day (its daily `sync-base` job, once the product's tests pass) and you with that product's next signed release.

| Version | Security fixes |
|---|---|
| Phygital Key MCP 0.4.x (`phygital-key-mcp`) | Yes |
| Phygital Key A2A (`phygital-key-a2a`, new) | Its latest release |
| Phygital Key 0.3.x and older (the program `phygital-key`, before the split into base and products) | No. Move to Phygital Key MCP: it has a new program name, data folder and secure-install names (see [updates](docs/updates.md#from-03-to-the-products)) |

How fixes reach you, for Phygital Key MCP (Phygital Key A2A uses the same commands with its own names):

| You installed with | Updates |
|---|---|
| The secure install | Daily and automatic (on by default): only a newer release whose file list is signed by a pinned release key. Update now with `phygital-key-mcp-admin update`; `phygital-key-mcp-admin status` shows the last result. |
| `install.sh` or `cargo install` | `phygital-key-mcp update` (checks the same signature) |
| Homebrew | `brew upgrade phygital-key-mcp` |
| Scoop (Windows) | `scoop update phygital-key-mcp` |
| Container (`ghcr.io/phygital-key/phygital-key-mcp`) | Pull the new image (`docker pull ghcr.io/phygital-key/phygital-key-mcp:latest`, or the new `vX.Y.Z` tag) and start a new container on the same volume. Nothing inside the container updates it ([how](docs/own-server.md#run-it-in-a-container)). |

### How a release is signed

Each product signs its own releases, in its own repository, with its own key. The base makes no releases.

1. The repository owner pushes a tag `vX.Y.Z` in the product's repository. A repository ruleset lets only repository admins create, update or delete `v*` tags. CI runs the tests, builds every archive and publishes a build record for each (a build provenance attestation: which workflow built which file from which commit). Anyone can check one with `gh attestation verify <file> --repo Phygital-Key/phygital-key-mcp`.
2. The `publish` job gets the release key, the secret `RELEASE_SIGNING_KEY` of that repository's environment `release`, which only runs from `v*` tags can use. For now the environment needs no approval, so the job does not wait for the owner: the tag push alone starts signing. The owner plans to turn the approval back on as the project grows.
3. The job computes every SHA-256 itself, signs `release.txt`, checks the signature against the keys pinned in that commit, and publishes the release with `release.txt` and `release.txt.sig`.
4. After that, the `container` job builds the container image from the two Linux archives of the same release, pushes it as `vX.Y.Z` and `latest`, and publishes a build provenance attestation for it: `gh attestation verify oci://ghcr.io/phygital-key/phygital-key-mcp:vX.Y.Z --repo Phygital-Key/phygital-key-mcp`. The image itself is not in `release.txt` and is not signed with the release key.

The release key exists only as that GitHub secret: no copy on any computer, no passphrase. So a swapped file, a mirror, a stolen workflow token or a pull request cannot make an update that installs. You can turn automatic updates off. See [docs/updates.md](docs/updates.md), and [the update channel](docs/threat-model.md#the-update-channel) for what the signature protects and what it does not.

## Reporting a problem

Please do not open a public issue, discussion or pull request for a security problem.

1. Open [private vulnerability reporting](https://github.com/Phygital-Key/phygital-key/security/advisories/new) (the **Security** tab, then **Report a vulnerability**). Only the maintainer sees it.
2. Include what is listed below.

There is no email contact. Use private reporting.

## What to include

- The product and version (`phygital-key-mcp --version`) and how you run it: `stdio`, `serve`, `demo`, the secure install on macOS or Linux (with automatic updates on or off), the container image (with its tag), or Windows.
- Your operating system, and your AI app if it matters.
- What you found, and which part it is in if you know: the verifier (`crates/phygital-key-verify`), the JavaScript checker (`verifier-js`), the gate engine and approval page (`crates/phygital-key-gate`), an installer template, a product's agent protocol, the spec or the test vectors.
- Steps to reproduce it.
- What an attacker gains, and what they need first (for example a program on the gate machine, control of the AI agent, or a copy of an approval).

A failing [test vector](test-vectors/README.md) is the most helpful report: an input that one or both verifiers accept but should refuse, or the reverse.

## What not to send

- Your access token (`phygital-key-mcp token`), an enrollment link, or anything from `enroll-token.json`.
- A real release key. To show an update problem, sign a made-up `release.txt` with a fresh test key.
- Private keys of any kind. Use the public test keys in `test-vectors/keys/test-keys.json`, or a fresh test data folder (`phygital-key-mcp --home /tmp/pk-test demo`).
- Requests that carry real secrets or personal data. Approvals are signed, not encrypted: anyone who sees one can read its request.

## What happens next

We aim to reply within 7 days, and to publish a fix and a new test vector for every confirmed issue. Please keep the problem private until a fixed release is out. If you would like credit, say which name or GitHub account to use; there is no paid bug bounty.

## Testing in good faith

If you research in good faith and follow this policy, we will not take action against you. Test only on your own machines and your own gate, or ones you have permission to test. Do not access other people's data or approvals, and stop and report as soon as you see any. Do not run denial-of-service tests against anyone else's gate or service.

## Scope

In scope:

- Any way to make a service act without a valid yes from the person: forging, changing, replaying, stretching or redirecting an approval, a use token or a revocation.
- Any input where the Rust verifier and the JavaScript checker disagree.
- Any way for an agent to approve, to enroll a key, or to add a key to the keyring without approval by a trusted key, through any product's agent protocol.
- Any way for the approval page to show one request while the key signs another, outside the limits in the threat model.
- Any way around the [secure install](docs/secure-install.md) guarantee: the AI, acting as you, changing the gate's program, reading its data or writing its keys, without the administrator password.
- Any way to make the secure install, `install.sh` or `update` install a release that is not signed by a pinned release key, or make the daily update job install an older version.
- Any way to read a product's `RELEASE_SIGNING_KEY` or get a release signed without pushing a `v*` tag from a repository admin account.
- Secrets (access token, enrollment token) leaking into logs, pages or agent answers.
- Receipts or revocations accepted when they should not be.

Out of scope:

- What the [threat model](docs/threat-model.md) lists as trusted or as a known limit: a compromised device, authenticator or gate machine; the 60-second window after a revoke; signed-not-encrypted requests; no hardware attestation; an audit log rewritten in full; updates held back (a freeze); a bad release that was signed with a release key and starts.
- A service that does not call the verifier, or calls it with the agent's idea of the audience, request or level.
- A person approving a harmful request after reading it.
- An AI model following instructions hidden in data. Phygital Key's answer is that text never counts as a yes; better page warnings are welcome as normal issues.
- A local program denying or cancelling a waiting request (a nuisance; it cannot approve).
- Problems in your AI app, browser, passkey provider or operating system. Please report those to their makers.
- Reports from automated scanners without an attack that works.

## Read first

- [Threat model](docs/threat-model.md): what is protected, from whom, and which test proves each defence
- [Security reviews](docs/security-reviews.md): what has been found and fixed
- [Testing](docs/testing.md): every test layer and how to run it
