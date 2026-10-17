# Rust example: a database worker that checks approvals

A small Rust program that runs database jobs only with a valid approval for that exact job, using the [`phygital-key-verify`](../../docs/verifier-rust.md) library and `serde_json`, nothing else. Nothing real is deleted.

It reads one job per line on stdin and prints one JSON result per line:

```json
{"approval": "<approval string>", "request": {"type": "delete_rows", "params": {"database": "shop", "table": "sessions", "older_than": "2026-01-01"}}}
```

The worker decides the level by job type (`delete_rows` and `drop_table` need `high`, `vacuum` needs `medium`) and refuses any other type (`UNKNOWN_JOB`). A line that is not JSON gets `BAD_JOB`. It keeps used approval ids in memory, so each approval runs one job while the process lives. In a real worker the request comes from your own job queue, never from the approval.

This folder is its own Cargo package: the empty `[workspace]` table in `Cargo.toml` keeps it out of the repository's workspace, and it depends on the library by path.

## Build

From the repository root:

```bash
cargo build --manifest-path examples/rust-service/Cargo.toml
```

## Run it

| Variable | Default | Meaning |
|---|---|---|
| `KEYRING` | (required) | The keyring file to trust, e.g. `~/.phygital-key/keyring.json`. Read once at start |
| `AUD` | `worker:db-maintenance` | This worker's audience id |
| `GATE_ORIGIN` | `http://localhost:7468` | The approval page origin. Its host name is the allowed relying party id |

1. Start the demo gate (`phygital-key demo`) and add your key with the link it prints.
2. Ask your AI app: *"Ask me to approve `{"type":"delete_rows","params":{"database":"shop","table":"sessions","older_than":"2026-01-01"}}` with audience worker:db-maintenance at level high, then give me the approval string."*
3. Approve on the page, then put the approval in a jobs file and run the worker:

```bash
APPROVAL='<approval string>'
printf '{"approval":"%s","request":{"type":"delete_rows","params":{"database":"shop","table":"sessions","older_than":"2026-01-01"}}}\n' "$APPROVAL" > jobs.jsonl
KEYRING=~/.phygital-key/keyring.json cargo run --manifest-path examples/rust-service/Cargo.toml < jobs.jsonl
```

## What you should see

A real run with six jobs: the approved job, the same job again, the same approval for another table, an unknown job type, a job with no approval, and a line that is not JSON (gate on port 18600 with `--lifetime 300`, so `GATE_ORIGIN=http://localhost:18600`):

```text
worker ready: aud worker:db-maintenance, approvals from http://localhost:18600; one JSON job per line
{"approval_id":"u1hn7NY-z62h058QHBo-_A","approved_by":"Test laptop","done":"delete_rows (example: nothing real happened)","ok":true,"request_hash":"sm4GTc1whCdMDyaXnti9yTzBSfSzxnVBhq8ZItxg-kI"}
{"error":"REPLAYED","ok":false}
{"error":"REQUEST_MISMATCH","ok":false}
{"error":"UNKNOWN_JOB","ok":false}
{"error":"APPROVAL_REQUIRED","ok":false}
{"error":"BAD_JOB","ok":false}
```

## Before real use

- With several workers, use `Verifier::with_store` and a `SeenStore` backed by a shared, durable store.
- Check the keyring's signed history before trusting it: `phygital_key_verify::history::verify_history` the first time, against the person's fingerprint and the gate's origin, then `verify_history_update` against the copy you accepted for every newer copy. Restart the worker (or call `Verifier::set_keyring`) when the keyring changes.
- Log the approval id, request hash and approver with each job.

## Next

- [Node example](../node-service/)
- [CI example](../ci-approval/)
- [All examples](../README.md)
