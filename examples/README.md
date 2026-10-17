# Examples

Small, runnable examples of a service that checks a person's approval before it acts, plus prompts for AI agents. Each one was run against a real `phygital-key demo` with a real approval: approved gives OK, a changed request gives `REQUEST_MISMATCH`, a second use gives `REPLAYED`. Nothing real is deployed or deleted.

| Example | What it shows | Needs | Run |
|---|---|---|---|
| [`node-service/`](node-service/) | An HTTP deploy service in plain Node (`node:http`) that checks the `Phygital-Approval` header with the JavaScript checker | Node 20+ | `KEYRING=~/.phygital-key/keyring.json node examples/node-service/server.mjs` |
| [`rust-service/`](rust-service/) | A database worker that reads jobs on stdin and runs each only with a valid approval, using `phygital-key-verify` | Rust 1.89+ | `KEYRING=~/.phygital-key/keyring.json cargo run --manifest-path examples/rust-service/Cargo.toml < jobs.jsonl` |
| [`ci-approval/`](ci-approval/) | A GitHub Actions deploy job that checks the keyring and the approval with `phygital-key verify` before deploying | GitHub Actions (try the check locally with `phygital-key` and `jq`) | Copy `example-deploy.yml` into `.github/workflows/` |
| [`agent-prompts.md`](agent-prompts.md) | Instructions for AI agents: when to ask for approval and how to behave with the result | An AI app connected to the gate | Paste into `CLAUDE.md`, `AGENTS.md` or a system prompt |

## Before you start

Every example needs a gate with at least one key:

```bash
phygital-key demo
```

It serves the approval page on `http://localhost:7468`, prints a one-time link to add your passkey or security key, and runs its own fake service on port 7469. The keyring the examples trust is `~/.phygital-key/keyring.json`. To get approvals, connect an AI app to the gate (`phygital-key mcp-token` prints how) and ask it; see [things to ask your AI](../docs/examples.md).

The Node and Rust examples take the gate's origin from `GATE_ORIGIN` (default `http://localhost:7468`); the CI workflow sets it in its `env` block. Each example uses its own audience. An approval works only for the audience it names, so ask for the one the example prints.

## Next

- [Check approvals in your service](../docs/integrate.md)
- [Use cases](../docs/use-cases.md)
- [Error codes](../docs/error-codes.md)
