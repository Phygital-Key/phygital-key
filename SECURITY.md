# Security policy

Phygital Key lets a person approve exact AI actions with a passkey or hardware key, and lets any service check that approval. A flaw here could let an action run without the person's yes, so reports are welcome.

Phygital Key is **pre-audit**: seven internal reviews by AI security-auditor agents, every finding fixed or documented as a limit, and no paid third-party audit yet. See [docs/security-reviews.md](docs/security-reviews.md).

## Supported versions

Only the latest release gets security fixes while the project is at 0.x.

| Version | Security fixes |
|---|---|
| 0.3.x | Yes |
| 0.2.x and older | No, please update |

How fixes reach you:

| You installed with | Updates |
|---|---|
| The secure install | Daily and automatic (on by default): only a newer release whose file list is signed by a pinned release key. Update now with `phygital-key-admin update`; `phygital-key-admin status` shows the last result. |
| `install.sh` or `cargo install` | `phygital-key update` (checks the same signature) |
| Homebrew | `brew upgrade phygital-key` |
| Scoop (Windows) | `scoop update phygital-key` |

Automatic updates and `phygital-key update` arrived in 0.3.0, so the first release they can install is the one after 0.3.0.

### How a release is signed

1. A pushed tag `vX.Y.Z` makes CI build every archive, publish a build record for each (a build provenance attestation: which workflow built which file from which commit), and create a draft release that no installer or updater can see.
2. The maintainer runs `scripts/sign-release.sh vX.Y.Z` in their own terminal, never from an AI agent's session (the script refuses to run without a terminal). With `gh attestation verify` it checks that each file was built by `.github/workflows/release.yml` from that tag at the commit the maintainer's own clone has for it, so `gh` is needed. It computes every SHA-256 itself and asks for the key passphrase.
3. It signs `release.txt`, checks the signature against the pinned keys, uploads it and publishes the release.

The release keys are files encrypted with passphrases the maintainer types and keeps in a password manager. Neither the keys nor the passphrases are ever in the macOS Keychain or on GitHub, so a copy of the key files without the passphrase is useless. See [docs/updates.md](docs/updates.md), and [the update channel](docs/threat-model.md#the-update-channel) for what the signature protects and what it does not.

## Reporting a problem

Please do not open a public issue, discussion or pull request for a security problem.

1. Open [private vulnerability reporting](https://github.com/s17sh70x2jfl031ag/phygital-key-mcp/security/advisories/new) (the **Security** tab, then **Report a vulnerability**). Only the maintainer sees it.
2. Include what is listed below.

There is no email contact. Use private reporting.

## What to include

- The version (`phygital-key --version`) and how you run it: `mcp`, `serve`, `demo`, the secure install on macOS or Linux (with automatic updates on or off), or Windows.
- Your operating system, and your AI app if it matters.
- What you found, and which part it is in if you know: the verifier (`crates/phygital-key-verify`), the JavaScript checker (`verifier-js`), the gate and approval page (`crates/phygital-key`), the secure install, the spec or the test vectors.
- Steps to reproduce it.
- What an attacker gains, and what they need first (for example a program on the gate machine, control of the AI agent, or a copy of an approval).

A failing [test vector](test-vectors/README.md) is the most helpful report: an input that one or both verifiers accept but should refuse, or the reverse.

## What not to send

- Your MCP access token (`phygital-key mcp-token`), an enrollment link, or anything from `enroll-token.json`.
- A real release key or its passphrase. To show an update problem, sign a made-up `release.txt` with a fresh test key.
- Private keys of any kind. Use the public test keys in `test-vectors/keys/test-keys.json`, or a fresh test data folder (`phygital-key --home /tmp/pk-test demo`).
- Requests that carry real secrets or personal data. Approvals are signed, not encrypted: anyone who sees one can read its request.

## What happens next

We aim to reply within 7 days, and to publish a fix and a new test vector for every confirmed issue. Please keep the problem private until a fixed release is out. If you would like credit, say which name or GitHub account to use; there is no paid bug bounty.

## Testing in good faith

If you research in good faith and follow this policy, we will not take action against you. Test only on your own machines and your own gate, or ones you have permission to test. Do not access other people's data or approvals, and stop and report as soon as you see any. Do not run denial-of-service tests against anyone else's gate or service.

## Scope

In scope:

- Any way to make a service act without a valid yes from the person: forging, changing, replaying, stretching or redirecting an approval, a use token or a revocation.
- Any input where the Rust verifier and the JavaScript checker disagree.
- Any way for an agent to approve, to enroll a key, or to add a key to the keyring without approval by a trusted key.
- Any way for the approval page to show one request while the key signs another, outside the limits in the threat model.
- Any way around the [secure install](docs/secure-install.md) guarantee: the AI, acting as you, changing the gate's program, reading its data or writing its keys, without the administrator password.
- Any way to make the secure install, `install.sh` or `phygital-key update` install a release that is not signed by a pinned release key, or make the daily update job install an older version.
- Secrets (MCP token, enrollment token) leaking into logs, pages or tool answers.
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
