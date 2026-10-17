# Node example: a deploy service that checks approvals

A minimal HTTP service in plain Node (`node:http`, no dependencies). `POST /deploy` runs only with a valid approval for that exact deploy, checked by the independent JavaScript checker in [`verifier-js/`](../../docs/verifier-js.md). Nothing real is deployed.

What it does on each call:

1. Builds the request from its own fields only: `{"type": "deploy", "params": {"service", "env", "version"}}`. Any other field in the body is refused (`BAD_BODY`).
2. Reads the keyring file (on every call, so a changed file counts at once; a missing file trusts no key).
3. Checks the `Phygital-Approval` header for its own audience, that exact request and level `high` (this service decides the level).
4. Deploys only on success, and logs the approval id, the request hash and who approved.

## Run it

You need Node 20 or later and `phygital-key` ([getting started](../../docs/getting-started.md)).

**1. Start the demo gate** in one terminal. It serves the approval page on port 7468 and its own fake service on 7469:

```bash
phygital-key demo
```

The first time, it prints a one-time link: open it and add your passkey or security key. It also prints how to connect an AI app (or run `phygital-key mcp-token`).

**2. Start this service** in a second terminal, from this folder:

```bash
KEYRING=~/.phygital-key/keyring.json node server.mjs
```

It listens on `127.0.0.1:7600` with the audience `http://localhost:7600`. `GET /` answers with that audience and what `POST /deploy` needs.

| Variable | Default | Meaning |
|---|---|---|
| `KEYRING` | (required) | The keyring file to trust. The gate's is `~/.phygital-key/keyring.json`; `phygital-key keys` prints it |
| `PORT` | `7600` | Port to listen on |
| `AUD` | `http://localhost:<PORT>` | This service's audience id |
| `GATE_ORIGIN` | `http://localhost:7468` | The approval page origin. Its host name is the allowed relying party id |

**3. Ask your AI app** (connected to the demo gate):

> Ask me to approve deploying api 2.4.1 to production with audience http://localhost:7600 at level high. When I approve, send it with: curl -X POST http://localhost:7600/deploy -H "Phygital-Approval: <approval>" -d '{"service":"api","env":"production","version":"2.4.1"}'

The approval page opens. Approve with your fingerprint, face or PIN. The AI then calls the service.

## What you should see

These are the responses from a real run (approval from `phygital-key demo` with a virtual passkey, gate on port 18600 with `--lifetime 300`, this service on 18650 through `PORT`, `AUD` and `GATE_ORIGIN`):

| Call | Response |
|---|---|
| No `Phygital-Approval` header | `403 {"ok":false,"error":"APPROVAL_REQUIRED"}` |
| Approved `2.4.1`, body says `2.4.2` | `403 {"ok":false,"error":"REQUEST_MISMATCH"}` |
| Extra field `"force":"yes"` in the body | `400 {"ok":false,"error":"BAD_BODY"}` |
| An approval made for another audience | `403 {"ok":false,"error":"WRONG_AUDIENCE"}` |
| The approved deploy | `200 {"ok":true,"done":"Deployed api 2.4.1 to production (example: nothing real happened)","approval_id":"UIFNQkCmmSR2jNGgoazUHA","approved_by":"Test laptop"}` |
| The same approval again | `403 {"ok":false,"error":"REPLAYED"}` |

And the service log:

```text
example deploy service on http://localhost:18650, aud http://localhost:18650, trusting approvals from http://localhost:18600
refused api 2.4.1: APPROVAL_REQUIRED
refused api 2.4.2: REQUEST_MISMATCH
refused api 2.4.1: WRONG_AUDIENCE
deploy api 2.4.1 to production: approved by Test laptop (id UIFNQkCmmSR2jNGgoazUHA, request_hash hUXvlpWHC4xarFlhKwaU26H6q05m3gBpMoyE_YKRK7o)
refused api 2.4.1: REPLAYED
```

A failed check does not use up the approval: after the `REQUEST_MISMATCH` and `WRONG_AUDIENCE` attempts, the right deploy still went through once.

## Before real use

- Keep used ids in a shared, durable store (a database, Redis `SET NX`) if you run more than one instance. `MemorySeenStore` forgets on restart and is per process. `verify` awaits the store, so an async `insertIfNew` works. It must be atomic, and only an exact `true` counts as a first use.
- Check the keyring's signed history against the person's fingerprint before you trust it, and keep the copy you accepted: `phygital-key keys --check <fingerprint> --file keyring.json --signed-on <gate origin>`. See [key history](../../docs/key-history.md).
- Serve it over HTTPS (a proxy in front) if anything outside this machine calls it.
- For standing approvals, add `verifyUse` and the revocation list. See [integrate](../../docs/integrate.md).

## Next

- [Rust example](../rust-service/)
- [CI example](../ci-approval/)
- [All examples](../README.md)
