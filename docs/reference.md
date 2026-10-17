# Reference

## Commands

Global options (all commands): `--home <folder>` (data folder, default `~/.phygital-key`, env `PHYGITAL_KEY_HOME`), `--port <n>` (default 7468, env `PHYGITAL_KEY_PORT`), `--bind <addr>` (default `127.0.0.1`; on loopback the gate holds both `127.0.0.1` and `[::1]`), `--origin <url>` (public address of the approval page, default `http://localhost:<port>`; must be `https` unless it is localhost), `--open-mcp` (no access token on `/mcp`), `--lifetime <s>` (approval life, 1 to 300, default 60).

| Command | What it does |
|---|---|
| `phygital-key mcp` | The gate for one AI app over stdio (most apps start it this way). The approval page runs alongside; if the page cannot start, the gate stops. |
| `phygital-key serve` | The gate over HTTP: approval page at `/`, MCP at `/mcp` (needs the access token). Never prints secrets, so it is safe to log. |
| `phygital-key demo` | The gate plus a fake service on port + 1 (`/deploy`, `/payment`, `/send_message`) that only acts with a valid approval. Prints the enrollment link and the MCP line. |
| `phygital-key enroll` | Prints a one-time link to add a key (works once, for 10 minutes). |
| `phygital-key mcp-token` | Prints the HTTP MCP access token and the line to paste into an AI app. |
| `phygital-key keys` | Prints the keyring (public key cards and signed history) and the first key's fingerprint. `--check <fingerprint>` checks the history; with `--file <keyring>` also give `--signed-on <origin>`; `--previous <keyring>` refuses anything that is not an extension of the copy you accepted. |
| `phygital-key verify` | Checks an approval: `--approval`, `--request`, `--aud`, `--level` (default high), `--keyring`, `--seen-file`. Exit 0 ok, 1 refused. `@file` reads a value from a file. |
| `phygital-key receipt-check` | Checks a service's receipt: `--receipt`, `--key` (service receipt key), `--iss`, `--approval`. |
| `phygital-key trust-service` | Pins a service's receipt key (`--aud`, `--key`) in `services.json`, so the gate shows only receipts that service signed. |
| `phygital-key audit-check` | Checks the audit log's hash chain. |
| `phygital-key install-service` | Secure install as a separate system user (macOS, Linux; needs sudo). Downloads and checks the official release unless `--binary <root-owned path>`. `--release vX.Y.Z` to update, `--uninstall`, `--purge`. |

After the secure install: `phygital-key-admin enroll | token | keys | audit-check | status | logs` (each asks for your administrator password). `status` also checks that only the gate listens on its port.

## MCP tools

| Tool | Arguments | Result |
|---|---|---|
| `request_approval` | `request` `{type, params}`, `aud`, `level` (`medium` or `high`) | `id`, `approve_url`, `status: pending` |
| `get_approval` | `id`, `wait_seconds` (0 to 120) | `status` (`pending`, `approved`, `denied`, `cancelled`, `expired`), and `approval` when approved |
| `cancel_approval` | `id` | Withdraws a waiting request, or revokes an approved standing approval (returns `sign_url` for the person) |
| `record_receipt` | `id`, `receipt` | Records a service receipt signed by its pinned key |
| `request_standing_approval` | `for_type`, `fixed`, `vary` (`{"min", "max"}` or `{"one_of"}`), `max_uses` (1 to 1000), `days` (1 to 30), `aud` | `id`, `approve_url` |
| `use_standing_approval` | `id`, `request` | `approval`, `use_token` (send both), `uses_left` |

Reserved: request types `standing` and `revoke`, and the gate's own audience `phygital-key:keyring:<origin>` (key changes). The gate makes these itself; an agent asking for them is refused.

## HTTP endpoints (the gate)

| Path | What |
|---|---|
| `/` | Home page: trusted keys, remove buttons, key fingerprint |
| `/r/<id>` | Approval page for one request |
| `/enroll#token=...` | Add a key |
| `/.well-known/phygital-key.json` | Public key cards and signed key history |
| `/.well-known/phygital-key-revoked.json` | Signed revocations of standing approvals |
| `/mcp` | MCP over HTTP (`Authorization: Bearer <token>`, no browser Origin) |
| `/api/...` | The page's own API (same-origin only) |

## Files in the data folder

| File | What |
|---|---|
| `keyring.json` | Public key cards, origin and signed key history |
| `audit.log` | Hash-chained log of requests, approvals, denials, key changes, receipts (never the request itself) |
| `revocations.json` | Signed revocations of standing approvals |
| `services.json` | Pinned service receipt keys |
| `mcp-token` | The HTTP MCP access token |
| `enroll-token.json` | A one-time enrollment token (deleted once used) |
| `seen-ids.jsonl` | Approvals already used by `phygital-key verify` |

## Limits

| Limit | Value |
|---|---|
| Approval life | 60 s by default, 300 s at most |
| Time to decide | 10 minutes |
| Standing approval life | 30 days at most |
| Use token life | 60 s, never past the standing approval |
| Encoded approval | 16384 characters |
| Canonical request | 8192 bytes |
| Client data | 4096 bytes |
| JSON nesting | 16 levels |
| New requests | 30 per minute, 50 waiting, 5000 held (revocations are always let through) |
| Revocation list | 1000 entries read |

Formats and checks in detail: [spec](spec.md).
