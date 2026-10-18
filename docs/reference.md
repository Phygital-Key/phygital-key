# Reference

Every shared command, option, HTTP endpoint, file and limit of the Phygital Key products on one page, for looking things up. Commands are shown for Phygital Key MCP (`phygital-key-mcp`, 0.4.0); Phygital Key A2A (`phygital-key-a2a`) takes the same commands and options, and the names that differ are in [names per product](#names-per-product). Outputs and examples for each command are in [command line](cli.md). Terms are in the [glossary](glossary.md).

On this page:

- [Names per product](#names-per-product)
- [Commands](#commands)
- [Global options](#global-options)
- [Options per command](#options-per-command)
- [Environment variables](#environment-variables)
- [Exit codes](#exit-codes)
- [The admin helper](#the-admin-helper)
- [Agent endpoints](#agent-endpoints)
- [HTTP endpoints of the gate](#http-endpoints-of-the-gate)
- [HTTP endpoints of the demo service](#http-endpoints-of-the-demo-service)
- [Files in the data folder](#files-in-the-data-folder)
- [Files of the secure install](#files-of-the-secure-install)
- [Install methods](#install-methods)
- [The container image](#the-container-image)
- [Limits](#limits)
- [Cryptography](#cryptography)

## Names per product

Every name comes from the product's program name (`bin`) and short id (`id`) in its `Product` ([architecture](architecture.md#product)).

| | Phygital Key MCP | Phygital Key A2A |
|---|---|---|
| Repository | [Phygital-Key/phygital-key-mcp](https://github.com/Phygital-Key/phygital-key-mcp) | [Phygital-Key/phygital-key-a2a](https://github.com/Phygital-Key/phygital-key-a2a) (new) |
| Program | `phygital-key-mcp` | `phygital-key-a2a` |
| Version | 0.5.0 | 0.2.0 |
| Default port | `7468` (demo service `7469`) | `7478` (demo service `7479`) |
| Agent endpoint | `/mcp` (MCP Streamable HTTP), plus `stdio` | `/a2a` (A2A 1.0 JSON-RPC), agent card at `/.well-known/agent-card.json`, no stdio mode |
| Data folder | `~/.phygital-key-mcp` | `~/.phygital-key-a2a` |
| Environment | `PHYGITAL_KEY_MCP_HOME`, `_PORT`, `_BIND`, `_ORIGIN` | `PHYGITAL_KEY_A2A_HOME`, `_PORT`, `_BIND`, `_ORIGIN` |
| Admin helper | `phygital-key-mcp-admin` | `phygital-key-a2a-admin` |
| Secure install user | `_phygitalkeymcp` (macOS), `phygitalkey-mcp` (Linux) | `_phygitalkeya2a` (macOS), `phygitalkey-a2a` (Linux) |
| Program folder | `/Library/PhygitalKey/mcp`, `/usr/lib/phygital-key-mcp` | `/Library/PhygitalKey/a2a`, `/usr/lib/phygital-key-a2a` |
| Secure install data | `/Library/Application Support/PhygitalKey/mcp`, `/var/lib/phygital-key-mcp` | `/Library/Application Support/PhygitalKey/a2a`, `/var/lib/phygital-key-a2a` |
| Logs (macOS) | `/Library/Logs/PhygitalKey/mcp`, `/Library/Logs/PhygitalKey/mcp-update` | `/Library/Logs/PhygitalKey/a2a`, `/Library/Logs/PhygitalKey/a2a-update` |
| launchd labels | `io.phygitalkey.mcp.gate`, `io.phygitalkey.mcp.update` | `io.phygitalkey.a2a.gate`, `io.phygitalkey.a2a.update` |
| systemd units | `phygital-key-mcp.service`, `phygital-key-mcp-update.service`, `phygital-key-mcp-update.timer` | `phygital-key-a2a.service`, `phygital-key-a2a-update.service`, `phygital-key-a2a-update.timer` |
| Container image | `ghcr.io/phygital-key/phygital-key-mcp` | `ghcr.io/phygital-key/phygital-key-a2a` |
| Release archives | `phygital-key-mcp-vX.Y.Z-<target>.tar.gz` / `.zip` | `phygital-key-a2a-vX.Y.Z-<target>.tar.gz` / `.zip` |

## Commands

| Command | What it does | Detail |
|---|---|---|
| `phygital-key-mcp stdio` | The gate for one AI app over stdio (most MCP apps start it this way). The approval page runs alongside on `--port`; if the page cannot start, the gate stops. Never prints an enrollment link. Only in products with a stdio mode: elsewhere it is hidden and refuses with `<name> has no stdio mode: run <program> serve`. | [cli](cli.md#stdio-serve-and-demo) |
| `phygital-key-mcp serve` | The gate over HTTP: approval page at `/`, the agent endpoint at the product's path (needs the access token unless `--no-token`). Never prints the token or a link, so its output is safe to log. | [cli](cli.md#stdio-serve-and-demo) |
| `phygital-key-mcp demo` | The gate as `serve`, plus a fake service on port + 1 that only acts with a valid approval and pins its own receipt key. Prints how to connect, with the token, and an enrollment link if no key is enrolled. | [cli](cli.md#stdio-serve-and-demo) |
| `phygital-key-mcp enroll` | Writes a one-time enrollment token and prints the link to add a key (works once, for 10 minutes). The gate must be running. | [cli](cli.md#enroll-and-token) |
| `phygital-key-mcp token` | Prints the access token for the agent endpoint (made once) and how to connect an AI app or agent. | [cli](cli.md#enroll-and-token) |
| `phygital-key-mcp install-service` | Secure install: the gate as its own system user (macOS, Linux; needs sudo), program checked against the signed release list, daily signed updates unless turned off. | [cli](cli.md#install-service), [secure install](secure-install.md) |
| `phygital-key-mcp update` | Replaces the program file you run with the latest signed release, if it is newer. For installs in your own folders (install script, `cargo install`). Refuses for the secure install, Homebrew and Windows, and says what to run instead. | [cli](cli.md#update), [updates](updates.md) |
| `phygital-key-mcp keys` | Prints the keyring (key cards, origin, signed history) on stdout and the first key's fingerprint on stderr; with `--check`, checks a keyring's signed history. | [cli](cli.md#keys), [key history](key-history.md) |
| `phygital-key-mcp verify` | Checks an approval exactly as a service would and spends its id in a seen file. Exit 0 means act. | [cli](cli.md#verify) |
| `phygital-key-mcp receipt-check` | Checks a service's signed receipt for a one-time approval. | [cli](cli.md#receipt-check), [receipts](receipts.md) |
| `phygital-key-mcp trust-service` | Pins a service's receipt key in `services.json`, so the gate records only receipts that key signed. | [cli](cli.md#trust-service) |
| `phygital-key-mcp audit-check` | Checks the audit log's hash chain and prints the last hash. | [cli](cli.md#audit-check), [audit log](audit-log.md) |
| `phygital-key-mcp help` | Prints the help, or the help of one command. | |

Five hidden commands exist and are not meant to be typed: `release-check --file <list> --sig <list.sig> [--pq-sig <list.mldsa>]` (checks a signed release list with the pinned keys; for `release.txt` it prints the version, for `release2.txt` it also checks the ML-DSA signature when a key is pinned and the dates, and prints `<version> <signed> <expires>`; the secure installer uses it), `release-sign-pq --file <release2.txt> --out <release2.txt.mldsa>` (the release run's ML-DSA signature, key from `RELEASE_MLDSA_KEY`), `release-keygen-pq --public-out <file>` (makes the ML-DSA release key for `scripts/release-key.sh --pq`), `install-service --auto-update-run` (what the daily update job runs) and `product-files --out <folder>` (writes the product's files made from the base templates; used by the product's CI and `sync-base`).

## Global options

Every command accepts these, before or after the command name. Commands that do not use one ignore it.

| Option | Default | Rules |
|---|---|---|
| `--home <HOME>` | `PHYGITAL_KEY_MCP_HOME`, else `$HOME/.phygital-key-mcp` (`%USERPROFILE%` on Windows) | The data folder. Created with mode 0700 when a command needs it. |
| `--port <PORT>` | The product's port, `7468` for MCP (env `PHYGITAL_KEY_MCP_PORT`) | Port of the approval page, and of the agent endpoint in `serve` and `demo`. The demo service uses port + 1. |
| `--bind <BIND>` | `127.0.0.1` (env `PHYGITAL_KEY_MCP_BIND`) | Address to listen on. On loopback (`127.0.0.1`, `localhost`, `::1`) the gate holds both `127.0.0.1` and `[::1]` and refuses to start if either is taken. Keep it on loopback unless a TLS proxy sits in front. |
| `--origin <ORIGIN>` | `http://localhost:<port>` (env `PHYGITAL_KEY_MCP_ORIGIN`) | Public origin of the approval page: `scheme://host[:port]`, no path or query. Must be `https` unless the host is `localhost`. Its host is the WebAuthn relying party id (rp_id). For `verify`, it also sets which approval page counts. |
| `--no-token` | off | Serve the agent endpoint without an access token: anyone on this machine could then create requests. |
| `--lifetime <LIFETIME>` | `60` | Approval life in seconds, 1 to 300. |
| `-h`, `--help` | | Print help. |
| `-V`, `--version` | | Print the version (`phygital-key-mcp 0.4.0`). |

## Options per command

| Command | Options |
|---|---|
| `stdio`, `serve`, `demo`, `enroll`, `token`, `audit-check` | Global options only. |
| `install-service` | `--release <VERSION>` (release to install, `vX.Y.Z`, default this program's version; use it to update), `--binary <BINARY>` (install this root-owned file instead of downloading; it and every folder above it must be owned by root only), `--uninstall` (keeps keys and log), `--purge` (with `--uninstall`: also deletes the gate user, keys and log), `--auto-update` (daily signed updates, the default), `--no-auto-update` (off, kept on later installs until `--auto-update`). `--auto-update` and `--no-auto-update` cannot be combined. Of the global options only `--port` reaches the service: it always serves `http://localhost:<port>` from its own data folder, so `--home`, `--origin`, `--bind`, `--lifetime` and `--no-token` have no effect. |
| `update` | `--check` (only say whether a newer signed release exists). |
| `keys` | `--check <FINGERPRINT>` (check the signed history against the first key's fingerprint), `--file <FILE>` (keyring file, default `<home>/keyring.json`), `--signed-on <ORIGIN>` (origin the history must be signed on; required with `--file`, otherwise `--origin` is used), `--previous <PREVIOUS>` (the keyring accepted last time; the new one must extend it). |
| `verify` | `--approval <APPROVAL>` (or `@file`), `--request <REQUEST>` (the exact request JSON, or `@file`), `--aud <AUD>`, `--level <LEVEL>` (`medium` or `high`, default `high`), `--keyring <KEYRING>` (default `<home>/keyring.json`), `--seen-file <SEEN_FILE>` (default `<home>/seen-ids.jsonl`). The first three are required. |
| `receipt-check` | `--receipt <RECEIPT>` (or `@file`), `--key <KEY>` (service receipt key, SPKI DER base64url, or `@file`), `--iss <ISS>` (the service's audience id), `--approval <APPROVAL>` (or `@file`). All required. |
| `trust-service` | `--aud <AUD>`, `--key <KEY>` (SPKI DER base64url, or `@file`). Both required. |

`@file` reads the value from a file and trims surrounding whitespace.

## Environment variables

The program reads four variables named after it: the program name in capitals, `-` turned into `_`, then the setting.

| Variable (Phygital Key MCP) | Phygital Key A2A | Used by | Meaning |
|---|---|---|---|
| `PHYGITAL_KEY_MCP_HOME` | `PHYGITAL_KEY_A2A_HOME` | the program | Data folder when `--home` is not given. The container image sets it to `/data`. |
| `PHYGITAL_KEY_MCP_PORT` | `PHYGITAL_KEY_A2A_PORT` | the program | Default for `--port`. |
| `PHYGITAL_KEY_MCP_BIND` | `PHYGITAL_KEY_A2A_BIND` | the program | Default for `--bind`. |
| `PHYGITAL_KEY_MCP_ORIGIN` | `PHYGITAL_KEY_A2A_ORIGIN` | the program | Default for `--origin`. |
| `PHYGITAL_KEY_BIN_DIR` | the same | `install.sh` | Folder to install into, default `~/.local/bin`. |
| `PHYGITAL_KEY_VERSION` | the same | `install.sh` | A release to install instead of the latest, for example `v0.4.0`. |

`install-service` reads none of your environment. It runs as root, so it starts its script with an empty environment: only `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, root's `HOME` (`/var/root` on macOS, `/root` on Linux), `LANG=C` and its own `PKA_VERSION`, `PKA_BINARY` and `PKA_PORT`. The script also resets `PATH` and `HOME` and clears variables such as `CURL_HOME`, `SSL_CERT_FILE`, `OPENSSL_CONF`, `BASH_ENV`, `LD_PRELOAD` and `DYLD_INSERT_LIBRARIES` when it is run as `curl ... | sudo bash`. Every `curl` it or `update` runs starts with `-q`, so no `.curlrc` is read.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success: approval valid, receipt valid, keyring checks out, audit log intact, file written, update done or not needed. |
| `1` | Refused (`verify` and `receipt-check` print `{"ok": false, "error": "<CODE>"}`, `keys --check` prints `keyring NOT ok: <reason>`, all on stdout), or could not run (`phygital-key-mcp: <message>` on stderr, nothing on stdout). |
| `2` | Usage error: a missing or unknown argument. |

## The admin helper

The secure install adds `<program>-admin` (`phygital-key-mcp-admin`, `phygital-key-a2a-admin`) at `<prefix>/bin/`, linked from `/usr/local/bin` only when that folder is root-only. Each command runs through `sudo`, so it needs your administrator password.

| Command | What it does |
|---|---|
| `phygital-key-mcp-admin enroll` | Prints a one-time link to add a key (runs `enroll` as the gate user). |
| `phygital-key-mcp-admin token` | Prints the access token and how to connect (runs `token`). |
| `phygital-key-mcp-admin keys` | Prints the keyring (runs `keys`). |
| `phygital-key-mcp-admin audit-check` | Checks the audit log (runs `audit-check`). |
| `phygital-key-mcp-admin status` | Prints the program version, whether auto-update is on and the last update result, then checks that only the gate user listens on the port and prints `/api/status`. Exits 1 with a warning not to approve anything if another program listens or the gate is down. |
| `phygital-key-mcp-admin logs` | The last 50 lines of the gate log (macOS `gate.log`, Linux `journalctl -u <program>`). |
| `phygital-key-mcp-admin update [--now]` | Runs the update job now: installs the latest signed release if it is newer. It waits while standing approvals are active (waiting requests do not count); `--now` installs anyway. |

## Agent endpoints

What an agent can do depends on the product's protocol. In every product it can ask, wait, cancel, record a receipt and use a standing approval, and never approve.

| Product | Endpoint | Reference |
|---|---|---|
| Phygital Key MCP | stdio (`phygital-key-mcp stdio`) or Streamable HTTP at `/mcp`. Server name `phygital-key-mcp`, title `Phygital Key MCP`. Six tools: `request_approval`, `get_approval`, `cancel_approval`, `record_receipt`, `request_standing_approval`, `use_standing_approval` | [MCP tools reference](https://github.com/Phygital-Key/phygital-key-mcp/blob/main/docs/tools.md) |
| Phygital Key A2A | A2A 1.0 JSON-RPC at `/a2a`; agent card at `/.well-known/agent-card.json` (readable without the token) | [its repository](https://github.com/Phygital-Key/phygital-key-a2a) (new; its docs come later) |

Reserved in every product: the request types `standing` and `revoke`, and the audience `phygital-key:keyring:<origin>` (key changes). The gate makes these itself and refuses an agent that asks for them.

## HTTP endpoints of the gate

| Method and path | What |
|---|---|
| `GET /` | Home page: trusted keys, their fingerprints, remove buttons. |
| `GET /r/<id>` | Approval page for one request. |
| `GET /enroll#token=...` | Add a key. The token sits in the fragment, which browsers do not send to the server. |
| `GET /assets/...` | The page's script, styles, fonts and logo. |
| `GET /.well-known/phygital-key.json` | Public key cards and signed key history. |
| `GET /.well-known/phygital-key-revoked.json` | `{"revocations": [...]}`: signed revocations of standing approvals. |
| `GET /api/status` | `keys` (name, credential id, created, fingerprint), `rp_id`, `origin`, `root_fingerprint`, `history_ok`, `waiting`, `standing_active`. |
| `GET /api/r/<id>` | One request as the page shows it. |
| `POST /api/r/<id>/start`, `/finish`, `/deny` | The page's approve and deny steps. |
| `POST /api/r/<id>/revoke`, `/sign-revocation` | Revoke a standing approval, and get the link to sign its revocation. |
| `POST /api/enroll/begin`, `/api/enroll/finish` | Add a key. |
| `POST /api/keys/remove` | Ask to remove a key (needs an approval by a trusted key). |
| The agent path (`/mcp`, `/a2a`) | The product's agent endpoint in `serve` and `demo`: needs `Authorization: Bearer <token>` (any case of `Bearer`) unless `--no-token`, and refuses any request that carries a browser `Origin`. |

Every response carries `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `X-Frame-Options: DENY` and same-origin `Cross-Origin-Opener-Policy` and `Cross-Origin-Resource-Policy`; everything outside the agent path also gets a strict Content-Security-Policy (`default-src 'none'`, `frame-ancestors 'none'`) and `Cache-Control: no-store` (the built-in fonts and logo are cached). The gate answers 421 to a `Host` it does not expect (DNS rebinding), 403 to a `POST` outside the agent path whose `Origin` is not the gate's own, and 401 to the agent path without the right token. Page request bodies are limited to 64 KB. The two `/.well-known/phygital-key` files carry `Access-Control-Allow-Origin: *` on a public origin and no CORS header on `localhost`.

## HTTP endpoints of the demo service

On port + 1 (`http://localhost:7469` for Phygital Key MCP; that is also its audience).

| Method and path | What |
|---|---|
| `GET /` | Its audience, its actions and how to call them. |
| `GET /.well-known/phygital-key-service.json` | `{"iss", "receipt_public_key"}`: its receipt key. |
| `POST /deploy` | Level `high`. Body: the params. |
| `POST /payment` | Level `high`. |
| `POST /send_message` | Level `medium`. |

It reads the approval from the `Phygital-Approval` header and, for a standing approval, the use token from `Phygital-Use`. A refusal is HTTP 403 with `{"ok": false, "error": "<CODE>"}`; an action it does not offer is 404 with `UNKNOWN_ACTION`. Nothing real happens.

## Files in the data folder

`<home>` is `~/.<program>` by default (`~/.phygital-key-mcp`); for the secure install it is the data folder in [names per product](#names-per-product), mode 0700, owned by the gate user. Every file below is mode 0600.

| File | Written by | What |
|---|---|---|
| `keyring.json` | the gate, when keys change | Key cards, origin and signed key history |
| `audit.log` | the gate (append only) | Hash-chained log of requests, approvals, denials, cancels, key changes, receipts, standing uses and revocations; never the request itself |
| `revocations.json` | the gate | Signed revocations it publishes (a damaged file is refused, never replaced) |
| `services.json` | `trust-service` | Pinned service receipt keys, one per audience |
| `access-token` | `token`, `serve`, `demo` (first run) | The access token for the agent endpoint (32 random bytes, base64url) |
| `enroll-token.json` | `enroll` | A one-time enrollment token and its expiry, deleted at the first step of enrolling |
| `seen-ids.jsonl` (+ `.lock`, `.tmp`) | `verify` | Approval ids already used by `verify` |

Files are written to a `.tmp` file first, flushed and renamed into place. Waiting requests and the use keys of standing approvals live only in memory: a restart of the gate (an update included) drops requests that are still waiting and ends every standing approval it holds. The daily job waits while standing approvals are active (not for waiting requests), for up to 7 days from the first wait on the installed version, then installs anyway.

## Files of the secure install

`<prefix>` is `/Library/PhygitalKey/<id>` (macOS) or `/usr/lib/<program>` (Linux), owned by root, mode 0755.

| Path | What |
|---|---|
| `<prefix>/bin/<program>` | The program, owned by root (mode 0755). |
| `<prefix>/bin/<program>-admin` | The admin helper, owned by root. |
| `<prefix>/auto-update` | `on` or `off`: your auto-update choice, reused by later installs. |
| `<prefix>/update-status` | The daily job's last result, for example `2026-10-18T03:12:00Z up to date (v0.4.0; latest signed release v0.4.0)`. Written by root. |
| `<prefix>/update-postponed` | `<unix time> <installed version>`: when the daily job first waited for standing approvals on the installed version. Removed when an update goes ahead. Written by root. |
| `/usr/local/bin/<program>-admin` | Link to the helper, made only if `/usr/local/bin` is root-only. |

| | macOS | Linux |
|---|---|---|
| Gate user | `_phygitalkey<id>` (hidden, id 400 to 499) | `phygitalkey-<id>` (system user, no login) |
| Gate service | `/Library/LaunchDaemons/io.phygitalkey.<id>.gate.plist` | `/etc/systemd/system/<program>.service` (hardened unit) |
| Gate log | `/Library/Logs/PhygitalKey/<id>/gate.log` (folder 0700, gate user) | systemd journal: `journalctl -u <program>` |
| Daily update job | `/Library/LaunchDaemons/io.phygitalkey.<id>.update.plist` (root, mode 0600, a random time of day chosen at install) | `/etc/systemd/system/<program>-update.timer` (`OnCalendar=daily`, `RandomizedDelaySec=6h`, `Persistent=true`) and `<program>-update.service` (oneshot, `PrivateTmp`, `ProtectHome`) |
| Update log | `/Library/Logs/PhygitalKey/<id>-update/update.log` (root-only folder) | `journalctl -u <program>-update` |

The daily job fetches `https://github.com/<product repo>/releases/latest/download/release.txt` and its `.sig`, so the secure install contacts github.com once a day (and downloads the release file when it updates). The gate itself makes no network calls. Details: [secure install](secure-install.md).

## Install methods

For Phygital Key MCP. Each product publishes the same set from its own repository.

| Method | Command | Runs as | Updates |
|---|---|---|---|
| Secure install (macOS, Linux) | `curl -q -fsSL https://raw.githubusercontent.com/Phygital-Key/phygital-key-mcp/v0.5.0/install-service.sh \| sudo bash` | Its own system user | Daily, signed releases only (`--no-auto-update` turns it off) |
| Install script (macOS, Linux) | `curl -fsSL https://github.com/Phygital-Key/phygital-key-mcp/releases/latest/download/install.sh \| sh` | You | `phygital-key-mcp update` |
| Homebrew (macOS, Linux) | `brew install phygital-key/tap/phygital-key-mcp` | You | `brew upgrade phygital-key-mcp` |
| Scoop (Windows) | `scoop bucket add phygital-key-mcp https://github.com/Phygital-Key/phygital-key-mcp` then `scoop install phygital-key-mcp` | You | `scoop update phygital-key-mcp` |
| cargo | `cargo install --locked --git https://github.com/Phygital-Key/phygital-key-mcp phygital-key-mcp` | You | `phygital-key-mcp update`, or build again |
| Container (Linux servers) | `docker run -d --name phygital-key-mcp -p 127.0.0.1:7468:7468 -v phygital-key-mcp:/data ghcr.io/phygital-key/phygital-key-mcp:latest` | uid 65532 inside the container | Pull a new image, start a new container on the same volume |
| By hand | Download from [Releases](https://github.com/Phygital-Key/phygital-key-mcp/releases), [check it](updates.md#check-a-download-yourself) | You | Download again |

Each method in detail: [updates](updates.md#update-by-install-method).

## The container image

Setup with a TLS proxy, enrollment and updates: [run it in a container](own-server.md#run-it-in-a-container).

| Fact | Phygital Key MCP |
|---|---|
| Image | `ghcr.io/phygital-key/phygital-key-mcp` (Phygital Key A2A: `ghcr.io/phygital-key/phygital-key-a2a`) |
| Tags | `vX.Y.Z` (one release) and `latest` |
| Platforms | `linux/amd64`, `linux/arm64` |
| Base | `gcr.io/distroless/cc-debian12:nonroot`, pinned by digest in the product's `packaging/container/Dockerfile` |
| User | `65532:65532` (nonroot) |
| Program | `/usr/local/bin/phygital-key-mcp` (the entrypoint) |
| Default command | `--bind 0.0.0.0 serve` (listens on all addresses inside the container, so Docker can forward to it) |
| Data folder | `/data`, a volume; `PHYGITAL_KEY_MCP_HOME=/data` |
| Port | `7468` (publish it as `-p 127.0.0.1:7468:7468`) |
| Built by | The `container` job of the product's `.github/workflows/release.yml`, after the `publish` job signs the release, from the same Linux binaries whose archives are listed in the signed `release.txt` |
| Provenance | A build provenance attestation pushed with the image: `gh attestation verify oci://ghcr.io/phygital-key/phygital-key-mcp:v0.4.0 --repo Phygital-Key/phygital-key-mcp` |
| Not inside | Automatic updates and `update` |

Commands in a running container go through `docker exec`, for example `docker exec -it phygital-key-mcp phygital-key-mcp enroll` and `docker exec -it phygital-key-mcp phygital-key-mcp token`. On a desktop where your user can run `docker`, an agent acting as you can control the container, so it does not give the [secure install](secure-install.md)'s guarantee.

## Limits

| Limit | Value |
|---|---|
| Approval life | 60 s by default (`--lifetime`), 300 s at most |
| Time for the person to decide | 10 minutes |
| Waiting for an answer (MCP `get_approval`) | 0 to 120 s, default 60 |
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
| Request body to the page API | 64 KB |
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
| Release list | ECDSA P-256 / SHA-256 signature (DER) by the product's release key, a GitHub environment secret used only in release runs from `v*` tags, which only repository admins can push |

Approvals are signed and verified, not encrypted: anyone who holds one can read the request in it. Use HTTPS for any origin other than localhost. Formats and checks in detail: [spec](spec.md).

## Next

- [Command line](cli.md)
- [Secure install](secure-install.md)
- [Architecture](architecture.md)
- [Spec](spec.md)
