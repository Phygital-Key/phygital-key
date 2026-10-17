# Reference

Every command, option, MCP tool, HTTP endpoint, file and limit of Phygital Key 0.3.0 on one page, for looking things up. Outputs and examples for each command are in [command line](cli.md); every tool's schema, results and error texts are in [MCP tools reference](tools.md). Terms are in the [glossary](glossary.md).

On this page:

- [Commands](#commands)
- [Global options](#global-options)
- [Options per command](#options-per-command)
- [Environment variables](#environment-variables)
- [Exit codes](#exit-codes)
- [The admin helper](#the-admin-helper)
- [MCP server and tools](#mcp-server-and-tools)
- [HTTP endpoints of the gate](#http-endpoints-of-the-gate)
- [HTTP endpoints of the demo service](#http-endpoints-of-the-demo-service)
- [Files in the data folder](#files-in-the-data-folder)
- [Files of the secure install](#files-of-the-secure-install)
- [Limits](#limits)
- [Cryptography](#cryptography)

## Commands

| Command | What it does | Detail |
|---|---|---|
| `phygital-key mcp` | The gate for one AI app over stdio (most apps start it this way). The approval page runs alongside on `--port`; if the page cannot start, the gate stops. Never prints an enrollment link. | [cli](cli.md#mcp-serve-and-demo) |
| `phygital-key serve` | The gate over HTTP: approval page at `/`, MCP at `/mcp` (needs the access token unless `--open-mcp`). Never prints the token or a link, so its output is safe to log. | [cli](cli.md#mcp-serve-and-demo) |
| `phygital-key demo` | The gate as `serve`, plus a fake service on port + 1 that only acts with a valid approval and pins its own receipt key. Prints the MCP line with the token, and an enrollment link if no key is enrolled. | [cli](cli.md#mcp-serve-and-demo) |
| `phygital-key enroll` | Writes a one-time enrollment token and prints the link to add a key (works once, for 10 minutes). The gate must be running. | [cli](cli.md#enroll-and-mcp-token) |
| `phygital-key mcp-token` | Prints the HTTP MCP access token (made once) and the line to paste into an AI app. | [cli](cli.md#enroll-and-mcp-token) |
| `phygital-key install-service` | Secure install: the gate as its own system user (macOS, Linux; needs sudo), program checked against the signed release list, daily signed updates unless turned off. | [cli](cli.md#install-service), [secure install](secure-install.md) |
| `phygital-key update` | Replaces the program file you run with the latest signed release, if it is newer. For installs in your own folders (install script, `cargo install`). Refuses for the secure install, Homebrew and Windows, and says what to run instead. | [cli](cli.md#update), [updates](updates.md) |
| `phygital-key keys` | Prints the keyring (key cards, origin, signed history) on stdout and the first key's fingerprint on stderr; with `--check`, checks a keyring's signed history. | [cli](cli.md#keys), [key history](key-history.md) |
| `phygital-key verify` | Checks an approval exactly as a service would and spends its id in a seen file. Exit 0 means act. | [cli](cli.md#verify) |
| `phygital-key receipt-check` | Checks a service's signed receipt for a one-time approval. | [cli](cli.md#receipt-check), [receipts](receipts.md) |
| `phygital-key trust-service` | Pins a service's receipt key in `services.json`, so the gate records only receipts that key signed. | [cli](cli.md#trust-service) |
| `phygital-key audit-check` | Checks the audit log's hash chain and prints the last hash. | [cli](cli.md#audit-check), [audit log](audit-log.md) |
| `phygital-key help` | Prints the help, or the help of one command. | |

Two hidden commands exist for the secure install and are not meant to be typed: `release-check --file <release.txt> --sig <release.txt.sig>` (checks a signed release list with the pinned keys and prints its version; the install script uses it when `openssl` is missing) and `install-service --auto-update-run` (what the daily update job runs).

## Global options

Every command accepts these, before or after the command name. Commands that do not use one ignore it.

| Option | Default | Rules |
|---|---|---|
| `--home <HOME>` | `PHYGITAL_KEY_HOME`, else `$HOME/.phygital-key` (`%USERPROFILE%` on Windows) | The data folder. Created with mode 0700 when a command needs it. |
| `--port <PORT>` | `7468` (env `PHYGITAL_KEY_PORT`) | Port of the approval page, and of `/mcp` in `serve` and `demo`. The demo service uses port + 1. |
| `--bind <BIND>` | `127.0.0.1` (env `PHYGITAL_KEY_BIND`) | Address to listen on. On loopback (`127.0.0.1`, `localhost`, `::1`) the gate holds both `127.0.0.1` and `[::1]` and refuses to start if either is taken. Keep it on loopback unless a TLS proxy sits in front. |
| `--origin <ORIGIN>` | `http://localhost:<port>` (env `PHYGITAL_KEY_ORIGIN`) | Public origin of the approval page: `scheme://host[:port]`, no path or query. Must be `https` unless the host is `localhost`. Its host is the WebAuthn relying party id (rp_id). For `verify`, it also sets which approval page counts. |
| `--open-mcp` | off | Serve `/mcp` without an access token: anyone on this machine could then create requests. |
| `--lifetime <LIFETIME>` | `60` | Approval life in seconds, 1 to 300. |
| `-h`, `--help` | | Print help. |
| `-V`, `--version` | | Print the version (`phygital-key 0.3.0`). |

## Options per command

| Command | Options |
|---|---|
| `mcp`, `serve`, `demo`, `enroll`, `mcp-token`, `audit-check` | Global options only. |
| `install-service` | `--release <VERSION>` (release to install, `vX.Y.Z`, default this program's version; use it to update), `--binary <BINARY>` (install this root-owned file instead of downloading; it and every folder above it must be owned by root only), `--uninstall` (keeps keys and log), `--purge` (with `--uninstall`: also deletes the gate user, keys and log), `--auto-update` (daily signed updates, the default), `--no-auto-update` (off, kept on later installs until `--auto-update`). `--auto-update` and `--no-auto-update` cannot be combined. Of the global options only `--port` reaches the service: it always serves `http://localhost:<port>` from its own data folder, so `--home`, `--origin`, `--bind`, `--lifetime` and `--open-mcp` have no effect. |
| `update` | `--check` (only say whether a newer signed release exists). |
| `keys` | `--check <FINGERPRINT>` (check the signed history against the first key's fingerprint), `--file <FILE>` (keyring file, default `<home>/keyring.json`), `--signed-on <ORIGIN>` (origin the history must be signed on; required with `--file`, otherwise `--origin` is used), `--previous <PREVIOUS>` (the keyring accepted last time; the new one must extend it). |
| `verify` | `--approval <APPROVAL>` (or `@file`), `--request <REQUEST>` (the exact request JSON, or `@file`), `--aud <AUD>`, `--level <LEVEL>` (`medium` or `high`, default `high`), `--keyring <KEYRING>` (default `<home>/keyring.json`), `--seen-file <SEEN_FILE>` (default `<home>/seen-ids.jsonl`). The first three are required. |
| `receipt-check` | `--receipt <RECEIPT>` (or `@file`), `--key <KEY>` (service receipt key, SPKI DER base64url, or `@file`), `--iss <ISS>` (the service's audience id), `--approval <APPROVAL>` (or `@file`). All required. |
| `trust-service` | `--aud <AUD>`, `--key <KEY>` (SPKI DER base64url, or `@file`). Both required. |

`@file` reads the value from a file and trims surrounding whitespace.

## Environment variables

| Variable | Used by | Meaning |
|---|---|---|
| `PHYGITAL_KEY_HOME` | `phygital-key` | Data folder when `--home` is not given. |
| `PHYGITAL_KEY_PORT` | `phygital-key` | Default for `--port`. |
| `PHYGITAL_KEY_BIND` | `phygital-key` | Default for `--bind`. |
| `PHYGITAL_KEY_ORIGIN` | `phygital-key` | Default for `--origin`. |
| `PHYGITAL_KEY_BIN_DIR` | `install.sh` | Folder to install into, default `~/.local/bin`. |
| `PHYGITAL_KEY_VERSION` | `install.sh` | A release to install instead of the latest, for example `v0.3.0`. |

`install-service` reads none of your environment. It runs as root, so it starts its script with an empty environment: only `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, root's `HOME` (`/var/root` on macOS, `/root` on Linux), `LANG=C` and its own `PKA_VERSION`, `PKA_BINARY` and `PKA_PORT`. The script also resets `PATH` and `HOME` and clears variables such as `CURL_HOME`, `SSL_CERT_FILE`, `OPENSSL_CONF`, `BASH_ENV`, `LD_PRELOAD` and `DYLD_INSERT_LIBRARIES` when it is run as `curl ... | sudo bash`. Every `curl` it or `phygital-key update` runs starts with `-q`, so no `.curlrc` is read.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success: approval valid, receipt valid, keyring checks out, audit log intact, file written, update done or not needed. |
| `1` | Refused (`verify` and `receipt-check` print `{"ok": false, "error": "<CODE>"}`, `keys --check` prints `keyring NOT ok: <reason>`, all on stdout), or could not run (`phygital-key: <message>` on stderr, nothing on stdout). |
| `2` | Usage error: a missing or unknown argument. |

## The admin helper

The secure install adds `phygital-key-admin` at `<prefix>/bin/phygital-key-admin`, linked from `/usr/local/bin` only when that folder is root-only. Each command runs through `sudo`, so it needs your administrator password.

| Command | What it does |
|---|---|
| `phygital-key-admin enroll` | Prints a one-time link to add a key (runs `enroll` as the gate user). |
| `phygital-key-admin token` | Prints the MCP access token and the line to paste (runs `mcp-token`). |
| `phygital-key-admin keys` | Prints the keyring (runs `keys`). |
| `phygital-key-admin audit-check` | Checks the audit log (runs `audit-check`). |
| `phygital-key-admin status` | Prints the program version, whether auto-update is on and the last update result, then checks that only the gate user listens on the port and prints `/api/status`. Exits 1 with a warning not to approve anything if another program listens or the gate is down. |
| `phygital-key-admin logs` | The last 50 lines of the gate log (macOS `gate.log`, Linux `journalctl -u phygital-key`). |
| `phygital-key-admin update [--now]` | Runs the update job now: installs the latest signed release if it is newer. It waits while standing approvals are active (waiting requests do not count); `--now` installs anyway. |

## MCP server and tools

The server names itself `phygital-key` (title `Phygital Key`, version = the program version) and offers six tools. None of them can approve. Schemas, every result and every error text: [MCP tools reference](tools.md).

| Tool | Arguments | Result |
|---|---|---|
| `request_approval` | `request` (`{"type", "params"}`), `aud`, `level` (`medium` or `high`) | `id`, `approve_url`, `status: pending`, `next` |
| `get_approval` | `id`, `wait_seconds` (0 to 120, default 60) | `status` (`pending`, `approved`, `denied`, `cancelled`, `expired`), `next`, and when approved `approval`, `approved_by`, `exp` |
| `cancel_approval` | `id` | Withdraws a waiting request (`status: cancelled`), or revokes an approved standing approval and returns `sign_url` for the person's signed revocation |
| `record_receipt` | `id`, `receipt` | `id`, `service_reported` (`success` or `error`); only for a receipt signed by the service's pinned key |
| `request_standing_approval` | `for_type`, `fixed`, `vary` (`{"min", "max"}` integers, both required, or `{"one_of": [...]}` with 1 to 20 strings), `max_uses` (1 to 1000), `days` (1 to 30), `aud` | `id`, `approve_url`, `status: pending`, `next` |
| `use_standing_approval` | `id`, `request` | `approval`, `use_token` (send both), `uses_left`, `next` |

`next` after approval, exactly:

| Approved | `next` |
|---|---|
| One-time approval | ``Approved. Send `approval` with the action to the service, in the Phygital-Approval header (or the field the service names). It works once, before `exp`.`` |
| Standing approval | ``Approved standing approval. For each action, call use_standing_approval with the exact request, then send both `approval` (header Phygital-Approval) and `use_token` (header Phygital-Use). It lasts until `exp` or until its uses run out.`` |

Reserved: the request types `standing` and `revoke`, and the audience `phygital-key:keyring:<origin>` (key changes). The gate makes these itself and refuses an agent that asks for them.

## HTTP endpoints of the gate

| Method and path | What |
|---|---|
| `GET /` | Home page: trusted keys, their fingerprints, remove buttons. |
| `GET /r/<id>` | Approval page for one request. |
| `GET /enroll#token=...` | Add a key. The token sits in the fragment, which browsers do not send to the server. |
| `GET /assets/...` | The page's script, styles, fonts and logo. |
| `GET /.well-known/phygital-key.json` | Public key cards and signed key history. |
| `GET /.well-known/phygital-key-revoked.json` | `{"revocations": [...]}`: signed revocations of standing approvals. |
| `GET /api/status` | `keys` (name, credential id, created, fingerprint), `rp_id`, `origin`, `root_fingerprint`, `history_ok`. |
| `GET /api/r/<id>` | One request as the page shows it. |
| `POST /api/r/<id>/start`, `/finish`, `/deny` | The page's approve and deny steps. |
| `POST /api/r/<id>/revoke`, `/sign-revocation` | Revoke a standing approval, and get the link to sign its revocation. |
| `POST /api/enroll/begin`, `/api/enroll/finish` | Add a key. |
| `POST /api/keys/remove` | Ask to remove a key (needs an approval by a trusted key). |
| `/mcp` | MCP over HTTP in `serve` and `demo`: needs `Authorization: Bearer <token>` (any case of `Bearer`) unless `--open-mcp`, and refuses any request that carries a browser `Origin`. |

Every response carries `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `X-Frame-Options: DENY` and same-origin `Cross-Origin-Opener-Policy` and `Cross-Origin-Resource-Policy`; everything outside `/mcp` also gets a strict Content-Security-Policy (`default-src 'none'`, `frame-ancestors 'none'`) and `Cache-Control: no-store` (the built-in fonts and logo are cached). The gate answers 421 to a `Host` it does not expect (DNS rebinding), 403 to a `POST` outside `/mcp` whose `Origin` is not the gate's own, and 401 to `/mcp` without the right token. Request bodies are limited to 64 KB. The two `/.well-known` files carry `Access-Control-Allow-Origin: *` on a public origin and no CORS header on `localhost`.

## HTTP endpoints of the demo service

On port + 1 (`http://localhost:7469` by default; that is also its audience).

| Method and path | What |
|---|---|
| `GET /` | Its audience, its actions and how to call them. |
| `GET /.well-known/phygital-key-service.json` | `{"iss", "receipt_public_key"}`: its receipt key. |
| `POST /deploy` | Level `high`. Body: the params. |
| `POST /payment` | Level `high`. |
| `POST /send_message` | Level `medium`. |

It reads the approval from the `Phygital-Approval` header and, for a standing approval, the use token from `Phygital-Use`. A refusal is HTTP 403 with `{"ok": false, "error": "<CODE>"}`; an action it does not offer is 404 with `UNKNOWN_ACTION`. Nothing real happens.

## Files in the data folder

`<home>` is `~/.phygital-key` by default; for the secure install it is `/Library/Application Support/PhygitalKey` (macOS) or `/var/lib/phygital-key` (Linux), mode 0700, owned by the gate user. Every file below is mode 0600.

| File | Written by | What |
|---|---|---|
| `keyring.json` | the gate, when keys change | Key cards, origin and signed key history |
| `audit.log` | the gate (append only) | Hash-chained log of requests, approvals, denials, cancels, key changes, receipts, standing uses and revocations; never the request itself |
| `revocations.json` | the gate | Signed revocations it publishes (a damaged file is refused, never replaced) |
| `services.json` | `trust-service` | Pinned service receipt keys, one per audience |
| `mcp-token` | `mcp-token`, `serve`, `demo` (first run) | The HTTP MCP access token (32 random bytes, base64url) |
| `enroll-token.json` | `enroll` | A one-time enrollment token and its expiry, deleted at the first step of enrolling |
| `seen-ids.jsonl` (+ `.lock`, `.tmp`) | `verify` | Approval ids already used by `phygital-key verify` |

Files are written to a `.tmp` file first, flushed and renamed into place. Waiting requests and the use keys of standing approvals live only in memory: a restart of the gate (an update included) drops requests that are still waiting and ends every standing approval it holds. The daily job waits while standing approvals are active (not for waiting requests), for up to 7 days from the first wait on the installed version, then installs anyway.

## Files of the secure install

`<prefix>` is `/Library/PhygitalKey` (macOS) or `/usr/lib/phygital-key` (Linux), owned by root, mode 0755.

| Path | What |
|---|---|
| `<prefix>/bin/phygital-key` | The program, owned by root (mode 0755). |
| `<prefix>/bin/phygital-key-admin` | The admin helper, owned by root. |
| `<prefix>/auto-update` | `on` or `off`: your auto-update choice, reused by later installs. |
| `<prefix>/update-status` | The daily job's last result, for example `2026-10-18T03:12:00Z up to date (v0.3.0; latest signed release v0.3.0)`. Written by root. |
| `<prefix>/update-postponed` | `<unix time> <installed version>`: when the daily job first waited for standing approvals on the installed version. Removed when an update goes ahead. Written by root. |
| `/usr/local/bin/phygital-key-admin` | Link to the helper, made only if `/usr/local/bin` is root-only. |

| | macOS | Linux |
|---|---|---|
| Gate user | `_phygitalkey` (hidden, id 400 to 499) | `phygitalkey` (system user, no login) |
| Gate service | `/Library/LaunchDaemons/io.phygitalkey.gate.plist` | `/etc/systemd/system/phygital-key.service` (hardened unit) |
| Gate log | `/Library/Logs/PhygitalKey/gate.log` (folder 0700, gate user) | systemd journal: `journalctl -u phygital-key` |
| Daily update job | `/Library/LaunchDaemons/io.phygitalkey.update.plist` (root, mode 0600, a random time of day chosen at install) | `/etc/systemd/system/phygital-key-update.timer` (`OnCalendar=daily`, `RandomizedDelaySec=6h`, `Persistent=true`) and `phygital-key-update.service` (oneshot, `PrivateTmp`, `ProtectHome`) |
| Update log | `/Library/Logs/PhygitalKeyUpdate/update.log` (root-only folder) | `journalctl -u phygital-key-update` |

The daily job fetches `https://github.com/s17sh70x2jfl031ag/phygital-key-mcp/releases/latest/download/release.txt` and its `.sig`, so the secure install contacts github.com once a day (and downloads the release file when it updates). The gate itself makes no network calls. Details: [secure install](secure-install.md).

## Limits

| Limit | Value |
|---|---|
| Approval life | 60 s by default (`--lifetime`), 300 s at most |
| Time for the person to decide | 10 minutes |
| `get_approval` wait | 0 to 120 s, default 60 |
| Enrollment link | 10 minutes, works once |
| Clock drift allowed (`iat` in the future) | 30 s |
| Standing approval | 1 to 30 days, 1 to 1000 uses, always level `high` |
| `one_of` list | 1 to 20 strings |
| Use token life | 60 s, never past the standing approval's end |
| Encoded approval | 16384 characters |
| Canonical request | 8192 bytes |
| Client data | 4096 bytes |
| Receipt | 4096 characters |
| JSON nesting | 16 levels |
| Integers | within ±(2^53 - 1); no floats |
| Request `type` | 1 to 64 of `a-z 0-9 _ . -` |
| Audience (`aud`) at the gate | 1 to 256 visible ASCII characters |
| New requests at the gate | 30 per minute, 50 waiting, 5000 held; revocations are always let through, at most one revocation request per standing approval |
| Request body to the gate | 64 KB |
| Revocation list | first 1000 entries read |
| Release list (`release.txt`) | 8192 bytes |
| Seen and use stores | keep each id until `exp + 30` (use stores: the standing approval's end + 30) |

## Cryptography

| What | Algorithm |
|---|---|
| The person's approval | WebAuthn assertion, ES256 (ECDSA over P-256 with SHA-256), from a passkey or hardware key; the private key never leaves the authenticator |
| Challenge | $\mathrm{SHA256}(\texttt{"phygital-key/approval/v1"} \Vert \texttt{0x00} \Vert \mathrm{JCS}(\text{claims}))$ |
| `request_hash` | base64url SHA-256 of the RFC 8785 canonical request |
| Fingerprint | base64url SHA-256 of the key's SPKI DER |
| Use tokens | ES256 JWS signed by a per-standing-approval P-256 key the gate makes and holds only in memory |
| Receipts | ES256 JWS signed by the service's own P-256 key |
| Audit log | SHA-256 hash chain |
| Release list | ECDSA P-256 / SHA-256 signature (DER) by a release key kept off GitHub |

Approvals are signed and verified, not encrypted: anyone who holds one can read the request in it. Use HTTPS for any origin other than localhost. Formats and checks in detail: [spec](spec.md).

## Next

- [Command line](cli.md)
- [MCP tools reference](tools.md)
- [Secure install](secure-install.md)
- [Spec](spec.md)
