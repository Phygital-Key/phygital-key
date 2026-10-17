# Security policy

Phygital Key is **pre-audit**: no outside security review has happened yet. Please treat it that way.

## Reporting a problem

Please do not open a public issue for a security problem. Use GitHub's private reporting instead:
**Security → Report a vulnerability** on this repository.

Include what you did, what happened, and what you expected. A failing test vector is the most helpful report.

We aim to reply within 7 days and to publish a fix and a new test vector for every confirmed issue.

## Scope

In scope: the verifier (`crates/phygital-key-verify`), the gate and approval page (`crates/phygital-key`), the spec (`docs/spec.md`) and the test vectors.

Before reporting, read [docs/threat-model.md](docs/threat-model.md). It lists what the project trusts (the person's device, the machine running the gate, the verifier's keyring) and the known limits of v0.1.

## Supported versions

Only the latest release gets fixes while the project is at 0.x.
