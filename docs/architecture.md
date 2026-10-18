# Architecture

This page is for contributors. It describes how the base and the products fit together, the parts of the gate, where the trust boundaries are, how a request moves from "asked" to "approved", what is kept in memory and on disk, and the limits the gate enforces. Every rule here is read from the code in `crates/phygital-key-gate/src` and `crates/phygital-key-verify/src`. Commands are shown with `phygital-key-mcp`; `phygital-key-a2a` takes the same commands and options.

## On this page

- [The base and the products](#the-base-and-the-products)
- [Components](#components)
- [Trust boundaries](#trust-boundaries)
- [Request lifecycle](#request-lifecycle)
- [Memory and disk](#memory-and-disk)
- [The two clocks](#the-two-clocks)
- [Limits](#limits)
- [The HTTP guard](#the-http-guard)
- [Listening addresses](#listening-addresses)
- [Modes](#modes)
- [Shutdown](#shutdown)
- [Enrollment](#enrollment)
- [How finish checks before it stores](#how-finish-checks-before-it-stores)
- [Saving the keyring](#saving-the-keyring)
- [Signed releases and updates](#signed-releases-and-updates)

## The base and the products

The base (this repository) holds everything the products share. A product is a small program that names itself and adds one agent protocol.

```mermaid
flowchart TD
    subgraph base["Base: Phygital-Key/phygital-key"]
        V["phygital-key-verify (verifier)"]
        G["phygital-key-gate (gate engine)"]
        T["assets/*.sh templates"]
        G --> V
        G --> T
    end
    subgraph mcp["Product: Phygital-Key/phygital-key-mcp"]
        MP["Product: phygital-key-mcp, id mcp, port 7468"]
        MA["Adapter: MCP at /mcp, stdio"]
    end
    subgraph a2a["Product: Phygital-Key/phygital-key-a2a"]
        AP["Product: phygital-key-a2a, id a2a, port 7478"]
        AA["Adapter: A2A at /a2a, agent card"]
    end
    MP --> G
    MA --> G
    AP --> G
    AA --> G
```

### Product

`Product` (in `src/product.rs`) is fixed at build time:

| Field | Meaning | Phygital Key MCP |
|---|---|---|
| `name` | Display name | `Phygital Key MCP` |
| `bin` | Program name. Also the release file prefix, the data folder `~/.<bin>`, the environment prefix and the admin helper `<bin>-admin` | `phygital-key-mcp` |
| `id` | Short id (lowercase letters and digits) for the secure install's user, folders and service labels | `mcp` |
| `version` | The product crate's version | `0.4.0` |
| `repo` | GitHub repository with the product's signed releases | `Phygital-Key/phygital-key-mcp` |
| `port` | Default port of the approval page | `7468` |
| `about` | One line for `--help` | |
| `release_keys` | The product's pinned release keys (P-256 `PUBLIC KEY` PEM blocks), from its `release-keys.pem` | |

`Product::env("PORT")` gives `PHYGITAL_KEY_MCP_PORT`: the program name in capitals with `-` turned into `_`. The shared commands read `<PREFIX>_HOME`, `_PORT`, `_BIND` and `_ORIGIN`.

### Adapter

`Adapter` is the trait a product implements for its agent protocol:

| Method | What it gives the gate |
|---|---|
| `path()` | The URL path the protocol is served on (`/mcp`, `/a2a`). Every request under it needs the access token and is refused when it carries a browser `Origin`. |
| `router(gate)` | The protocol's routes, all under `path()` |
| `public_router(gate)` | Optional GET-only routes anyone may read without the token, for example an A2A agent card |
| `connect_hint(settings)` | How to connect an AI app or agent, printed by `serve`, `demo` and `token` |
| `demo_hint()` | What to try first in the demo |
| `has_stdio()`, `stdio(gate)` | An optional stdio mode. Without one, the `stdio` command is hidden and refuses with "has no stdio mode". |

A product's `main` is one call: `phygital_key_gate::run(&PRODUCT, Adapter)`. The adapter talks to the gate through the same `Gate` API the page uses to create requests, wait, cancel, record receipts and hand out use tokens. Nothing in that API approves.

### Templates and product-files

Five shell files are templates in `crates/phygital-key-gate/assets/`. Each has a `# BEGIN PRODUCT` block (`P_NAME`, `P_BIN`, `P_ID`, `P_REPO`, `P_PORT`, `P_VERSION`) that `product_files()` fills for one product; the two installers also get a `# BEGIN RELEASE KEYS` block filled from `release_keys`.

| Template | Written in the product as |
|---|---|
| `install-service.sh` | `install-service.sh` (also compiled into the program and run by `install-service`) |
| `install.sh` | `install.sh` |
| `secure-install-test.sh` | `tests/secure-install.sh` (also gets `P_PATH`, the agent endpoint) |
| `release-key.sh` | `scripts/release-key.sh` |
| `package-manifests.sh` | `scripts/package-manifests.sh` |

The hidden command `phygital-key-mcp product-files --out .` writes them. The product's CI job "files made from the base are up to date" runs it and fails on `git diff --exit-code`, so a product never carries a hand-edited copy.

### sync-base

Each product depends on the base's `main` branch, and its `Cargo.lock` pins the exact commit. Its workflow `sync-base.yml` runs every day (and by hand):

1. `cargo update -p phygital-key-gate -p phygital-key-verify`, a release build, then `product-files --out .`.
2. If nothing changed, it stops.
3. Otherwise it runs `cargo test --locked` and the product's end-to-end checks (108 for Phygital Key MCP).
4. Only if they pass: it commits "Update the base to <commit>", pushes to the product's `main`, and starts the product's full CI (`gh workflow run ci.yml`).

A base change that breaks a product is never pushed to it; the product stays on the last good commit. Releases stay in the product: a base change reaches users only with the product's next signed release.

## Components

The base is one Rust workspace with three crates, plus a JavaScript verifier. Paths under `src/` and `assets/` are in `crates/phygital-key-gate`.

| Part | Where | What it is |
|---|---|---|
| Gate engine | `crates/phygital-key-gate` | A library. Holds the web server, the shared state and the shared commands. The product's adapter plugs in its protocol. |
| Commands | `src/cli.rs` | clap commands and settings. `run` names the commands after the product, sets its default port and environment names, and hides `stdio` if the adapter has none. |
| Product and adapter | `src/product.rs` | `Product`, `Adapter` and the templates ([above](#the-base-and-the-products)). |
| Web server | `src/web.rs` | axum router: the approval page, its JSON API, `/.well-known` files, and in `serve` and `demo` mode the adapter's routes. Every request passes the [HTTP guard](#the-http-guard). |
| State | `src/state.rs` | The `Gate` struct: pending requests, the keyring, the enrollment session, the audit log, the use keys of standing approvals, the access token. One mutex guards it (receipt keys pinned for this run sit behind their own). |
| Audit log | `src/audit.rs` | Append-only, hash-chained JSON lines. See [audit log](audit-log.md). |
| Updates | `src/update.rs`, `assets/install-service.sh` | The signed release list, `update`, the hidden `release-check` command, and the secure installer with its daily update job. See [Signed releases and updates](#signed-releases-and-updates). |
| Seen store | `src/seen.rs` | `FileSeenStore`, used by `verify` so each approval works once across processes. |
| Approval page | `assets/approve.html`, `assets/app.js`, `assets/style.css`, `assets/fonts.css` | Plain HTML and JavaScript, compiled into the program with `include_str!`. The same page in every product. See [approval page](approval-page.md). |
| Verifier library | `crates/phygital-key-verify` | Pure checks: [canonical form](canonical-form.md), strict JSON, [WebAuthn](webauthn.md) assertion, `verify`, key history, standing approvals, receipts. No network, no clock reads. The gate uses the same library to check signatures before it stores them. |
| Test vector generator | `crates/phygital-key-testgen` | Writes the shared [test vectors](../test-vectors/README.md). |
| JavaScript verifier | `verifier-js` | A second verifier written from the spec, sharing no code with the Rust one. See [JavaScript verifier](verifier-js.md). |

In a product repository: the adapter (for MCP, `src/mcp.rs` with six tools on the official `rmcp` SDK, server name `phygital-key-mcp`, title `Phygital Key MCP`; none of the tools can approve), `release-keys.pem`, the container image (`packaging/container/Dockerfile`) and the release workflow.

There is no language model anywhere in the gate or the page, and the gate makes no outgoing network calls. Only `update` and the secure install's daily update job contact GitHub.

## Trust boundaries

```mermaid
flowchart LR
    subgraph agentside["Untrusted: the agent"]
        A["AI app or agent"]
    end
    subgraph gatehost["Trusted: the gate host"]
        M["Adapter (MCP or A2A)"]
        W["Web server and HTTP guard"]
        S["State: requests, keyring, use keys"]
        D[("Data folder: keyring.json, audit.log")]
        M --> S
        W --> S
        S --> D
    end
    subgraph person["Trusted: the person"]
        B["Browser showing the approval page"]
        K["Authenticator: passkey or hardware key"]
        B --> K
    end
    subgraph service["Trusted by itself: the service"]
        V["Verifier library or CLI"]
        R[("Keyring copy and seen store")]
        V --> R
    end
    A -- "ask, wait, cancel" --> M
    B -- "same-origin page API" --> W
    A -- "action + approval" --> V
```

| Boundary | What crosses it | What protects it |
|---|---|---|
| Agent to gate | Protocol calls (MCP tools, A2A messages) | The adapter can only ask, wait, cancel, record a receipt and use a standing approval. Over HTTP, the agent path needs the access token and refuses any browser `Origin`. stdio is private to the app that started the gate. |
| Browser to gate | Page API calls | Host allowlist, exact `Origin` on every POST, strict CSP, no framing. |
| Page to authenticator | A WebAuthn `get` with a challenge the page computed | The page compares the claims with what it shows before it asks the key. The authenticator binds the signature to the gate's origin. |
| Agent to service | The action and the approval string | The service runs `verify` with its own audience, the request it is about to run, its own level and its own keyring. |

The gate host is trusted: it serves the page, so whoever controls it controls what the page shows. The [threat model](threat-model.md) covers this.

## Request lifecycle

Each request has one `Status`: `pending`, `approved`, `denied`, `cancelled` or `expired`. `expired` is never stored. `current_status(t)` derives it: a `pending` request at or past its `deadline`, or an `approved` one at or past its `exp`, reads as `expired`.

```mermaid
stateDiagram-v2
    [*] --> pending: create
    pending --> pending: start fixes the claims
    pending --> approved: finish, signature checked
    pending --> denied: deny on the page
    pending --> cancelled: cancel by the agent
    pending --> expired: deadline passes
    approved --> expired: exp passes
    approved --> cancelled: revoke, standing only
    denied --> [*]
    cancelled --> [*]
    expired --> [*]
    note right of cancelled
        Revoking a standing approval drops its use key
        and creates one revocation request.
        revocation_published turns true
        once the person signs that revocation.
    end note
```

- **Create** (`create_full`) checks the request, stores it as `pending` with `deadline = created + 600`, and writes a `requested` audit line.
- **Start** (`POST /api/r/{id}/start`, the person pressed Approve) fixes the claims: `v`, `id`, `aud`, `request`, `request_hash`, `level`, `iat` (now) and `exp`. While those claims are still valid (`exp` in the future), a second start returns the same claims, so another program cannot swap them while the person is signing. The status stays `pending`.
- **Finish** (`POST /api/r/{id}/finish`) takes the signature, builds the approval and checks it before anything is stored. See [How finish checks before it stores](#how-finish-checks-before-it-stores).
- **Deny** and **cancel** close a `pending` request and write `denied` or `cancelled`. A closed request cannot be approved later: start and finish refuse anything that is not `pending`.
- **Revoke** applies to an approved standing approval. It sets the status to `cancelled`, sets `usable` to false, removes the use key from `use_keys`, writes `revoked`, and creates the revocation request (`{"type": "revoke", "params": {"standing_id": ...}}`, level `medium`, same audience). There is at most one revocation request per standing approval: after a deny or a timeout, `sign_revocation` sets the same request back to `pending` with a new deadline and no claims, and never makes a second one. When the person signs it, the gate appends it to `revocations.json`, sets `revocation_published` on the standing approval and writes `revocation_published`.

## Memory and disk

### In memory

`Inner`, behind one `Mutex`:

| Field | Holds |
|---|---|
| `requests` | Every request, by id (see the fields below) |
| `recent` | Times of new requests in the last 60 seconds, for the per-minute limit |
| `enroll` | The one enrollment session, if any: SHA-256 of the token, a 32-byte challenge, a 16-byte user id, the key name, the expiry |
| `keyring` | The current keyring (public key cards, origin, signed history) |
| `audit` | The open audit log: path, last `seq`, last hash |
| `use_keys` | The private P-256 key of each standing approval, by request id. Memory only. |

The `Gate` struct also holds the `Settings` (including the adapter's `agent_path` and the access token), a `Notify` that wakes waiters, and `pinned`, receipt keys pinned for this run only (the demo pins its own service).

Fields of one `Request`:

| Field | Meaning |
|---|---|
| `id` | 128-bit random id, base64url (22 characters). Also the approval's single-use id. |
| `request` | The request value |
| `request_jcs` | Its canonical text. The page shows this. |
| `request_hash` | base64url SHA-256 of `request_jcs` |
| `aud` | Audience |
| `level` | `medium` or `high` |
| `kind` | `Action`, `AddKey(card)`, `RemoveKey(card)`, `Standing(params)` or `Revoke(standing_id)` |
| `first_key` | True when the keyring was empty at creation |
| `created`, `deadline` | Creation time and `created + 600` |
| `status` | Stored status (see above) |
| `claims` | The fixed claims, once Approve was pressed |
| `approval` | The checked approval string, once approved |
| `approved_by` | Name on the key card that signed |
| `exp` | Approval expiry, once approved |
| `receipt` | The receipt, once one signed by the pinned service key was recorded |
| `until` | Standing only: when the permission ends |
| `uses` | Standing only: use tokens signed so far |
| `usable` | Standing only: true from approval until revoke |
| `issued` | Standing only: use tokens handed out, by use id, waiting for their receipt |
| `revocation_published` | Standing only: the person's revocation is signed and published |

A request is forgotten at `keep_until()`: one hour after its `deadline`, or for an approved standing approval, one hour after `until`, whichever is later. `prune()` runs every 60 seconds and also drops expired enrollment sessions; `create` prunes too. A use key goes when its request goes.

### On disk

All in the data folder (`--home`, default `~/.<program>`: `~/.phygital-key-mcp` or `~/.phygital-key-a2a`), which the gate creates with mode `0700`. Files the gate writes are mode `0600`.

| File | Written by | Holds |
|---|---|---|
| `keyring.json` | `finish` on a key change | Key cards, `origin`, signed `history` |
| `audit.log` | Every event | Hash-chained JSON lines, never the request itself |
| `access-token` | First `serve`, `demo` or `token` | 32 random bytes, base64url: the token for the agent endpoint |
| `enroll-token.json` | `enroll` | `{"token", "expires"}`. Deleted when enrollment starts. |
| `services.json` | `trust-service` | Pinned receipt keys: `{"services": [{"aud", "receipt_public_key"}]}` |
| `revocations.json` | `finish` on a revocation | `{"revocations": ["<approval>", ...]}`, served at `/.well-known/phygital-key-revoked.json` |
| `seen-ids.jsonl` | `verify` only | Used approval ids with their `exp` |

Pending requests, decided requests, the enrollment session and use keys never touch the disk. A restart drops them: pending requests end (the answer is no) and every standing approval stops working, because its use key is gone.

## The two clocks

| Clock | Constant | Starts | Length |
|---|---|---|---|
| Time to decide | `PENDING_SECS` | When the request is created | 600 s |
| Approval life | `--lifetime` (`DEFAULT_LIFETIME_SECS`) | When the person presses Approve (`start`) | 60 s by default, 1 to 300 |

Fixing `iat` at Approve, not at creation, means a person who takes 9 minutes to read still gets a full minute to use the approval. The verifier allows `iat` up to 30 s in the future (`MAX_CLOCK_SKEW_SECS`) and refuses `exp - iat > 300`.

A standing approval is different: `until = created + days × 86400` is set when the request is made, and its claims use `exp = until`. `start` refuses if `until` is 60 s away or less, or more than 30 days away. The page refuses to sign if the claims' `exp` is not the `until` it shows.

## Limits

| Limit | Value | Why |
|---|---|---|
| `MAX_PER_MINUTE` | 30 new requests in any 60 s | Approval fatigue: a flood of requests trains people to click yes |
| `MAX_PENDING` | 50 waiting at once | Same, and a bound on what the person must look through |
| `MAX_STORED` | 5000 held in memory | A memory bound: decided requests stay up to an hour |
| Canonical request size | 8192 bytes (`MAX_REQUEST_BYTES`) | The page shows every field, and verifiers refuse more |
| Request nesting | 13 levels in the request | The approval object adds levels above `params`, and verifiers refuse nesting over 16 |
| Page API body | 64 KiB | No large bodies on the page routes |

A revocation request skips all three request limits and is not counted in `recent`. An agent that fills the limits must never be able to stop the person from revoking. Since there is at most one revocation request per standing approval, this cannot be used to flood the gate.

`create_full` also refuses, before the limits: a request not shaped `{"type", "params"}`; text not in NFC; floats or unsafe integers; an audience that is not 1 to 256 visible ASCII characters (`!` to `~`); the reserved types `standing` and `revoke` outside their own paths; the gate's own audience `phygital-key:keyring:<origin>` for actions; and any action while no key is enrolled.

## The HTTP guard

`guard` in `web.rs` runs on every HTTP request, in this order. "The agent path" is the adapter's `path()` and everything under it (`/mcp` for Phygital Key MCP, `/a2a` for Phygital Key A2A).

1. **Host allowlist.** The `Host` header, lowercased, must be one of: the host of `--origin` (with its port, if it has one), `localhost:<port>`, `127.0.0.1:<port>`, `[::1]:<port>`. Anything else gets `421 unknown host`. This stops DNS rebinding, where another site points its own name at `127.0.0.1`.
2. **No browsers on the agent path.** A request to the agent path that carries an `Origin` header gets `403`. Agent clients are not browsers.
3. **Access token.** On the agent path, when a token is set: `Authorization: <scheme> <token>` with the scheme `bearer` in any case, compared in constant time. Otherwise `401` ("missing or wrong access token (see `phygital-key-mcp token`)").
4. **Origin on writes.** Every method other than GET, outside the agent path, must carry `Origin` equal to the gate's origin exactly. Otherwise `403 wrong origin`. This stops other web pages from posting to the page API.
5. **Headers on the response.**

| Header | Value | Where |
|---|---|---|
| `Content-Security-Policy` | `default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'` | Everything except the agent path |
| `Cache-Control` | `no-store` (built-in fonts and logos keep `public, max-age=86400, immutable`) | Everything except the agent path |
| `X-Content-Type-Options` | `nosniff` | Everything |
| `Referrer-Policy` | `no-referrer` | Everything |
| `X-Frame-Options` | `DENY` | Everything |
| `Cross-Origin-Opener-Policy` | `same-origin` | Everything |
| `Cross-Origin-Resource-Policy` | `same-origin` | Everything |

In Phygital Key MCP, `/mcp` is `rmcp`'s Streamable HTTP service with allowed hosts `localhost`, `127.0.0.1`, `::1` and the RP ID, and its own origin validation on as well.

The `/.well-known/phygital-key.json` and `/.well-known/phygital-key-revoked.json` files carry `Access-Control-Allow-Origin: *` only when the RP ID is not `localhost`. On localhost there is no CORS header, so other web pages cannot read local key names.

## Listening addresses

When `--bind` is `127.0.0.1`, `localhost`, `::1` or `[::1]`, the gate listens on both `127.0.0.1:<port>` and `[::1]:<port>`. Browsers may try either for `localhost`. If the gate held only one, another program could open the other and serve a fake page with the same passkey origin.

- If the IPv6 address does not exist on the machine (`AddrNotAvailable`), the gate goes on with IPv4: nobody else can listen there either.
- Any other bind error stops the gate with "Another program is using this address ... Refusing to start."
- Each listener runs as its own task. When one stops, all stop, so one address is never left free while the other serves.

Any other `--bind` value gives one listener on that address. Keep the default unless a TLS proxy sits in front.

## Modes

| Mode | Agent transport | Page | Access token | Enrollment link printed |
|---|---|---|---|---|
| `stdio` (only products with a stdio mode, such as Phygital Key MCP) | stdio | Yes | None (stdio is private) | Never: stderr goes to the AI app's logs |
| `serve` | HTTP at the agent path | Yes | Required unless `--no-token` | Never: output often goes to a log file |
| `demo` | HTTP at the agent path | Yes, plus a fake service on port + 1 | Required unless `--no-token` | Yes, on stderr, when no key is enrolled |

In `stdio` mode the page server and the adapter's stdio server run under one `tokio::select!`. If either ends, the process ends. If the page cannot listen (its address is taken, perhaps by something pretending to be the gate), the agent side stops too and never hands out links to that address.

In `demo` mode the fake service checks approvals with the same library, pins its receipt key in the gate's memory, and offers `deploy` (level `high`), `payment` (level `high`) and `send_message` (level `medium`).

## Shutdown

- Ctrl-C or SIGTERM starts a graceful shutdown of every listener (`shutdown_signal` in `web.rs`).
- In `stdio` mode, closing stdin ends the agent side, which ends the process.
- `run` calls `std::process::exit` as soon as the command returns, because a stdio reader runs on a blocking thread that would keep the process alive.
- Nothing in memory survives. Pending requests are gone and standing approvals cannot be used again. The MCP product's end-to-end suite checks that stdio mode stops cleanly on SIGTERM.

## Enrollment

```mermaid
sequenceDiagram
    participant T as Your terminal
    participant B as Browser
    participant G as Gate
    participant K as Authenticator
    T->>G: phygital-key-mcp enroll writes enroll-token.json
    T-->>B: link with token in the fragment
    B->>B: read token, remove it from the address bar
    B->>G: enroll/begin with token and name
    G->>G: check token, delete the file, open a 10 minute session
    G-->>B: challenge, rp_id, user_id, exclude list
    B->>K: credentials.create, ES256, attestation none
    K-->>B: credential id, public key, clientDataJSON
    B->>G: enroll/finish
    G->>G: check clientDataJSON, key, id, make add_key request
    G-->>B: approve_url
    B->>G: approval page, the key signs its own add_key
```

1. `phygital-key-mcp enroll` writes 32 random bytes as `enroll-token.json` (`{"token", "expires": now + 600}`) and prints `<origin>/enroll#token=<token>`. The fragment never leaves the browser, so it is in no server log.
2. The enroll page reads the token from the fragment and removes it from the address bar and history (`history.replaceState`).
3. `enroll/begin`: if a session with this token's hash exists, it is reused (the person can retry after closing the key prompt). Otherwise the token must match the file, in constant time and before its expiry. The file is deleted at once, so the link works only once, and a session opens for 10 minutes. The key name is cleaned: control characters removed, at most 64 characters, `Key` if empty.
4. The browser creates the credential: `alg -7` (ES256), `attestation: "none"`, `residentKey: "preferred"`, `userVerification: "preferred"`, and the existing keys in `excludeCredentials`. The page refuses an authenticator that does not give a P-256 public key through `getPublicKey()`.
5. `enroll/finish` checks the session, then the client data (`type` `webauthn.create`, the session challenge, the gate's origin, not cross-origin), that the public key is a P-256 SPKI, that the credential id is 1 to 1023 bytes and not already enrolled. It builds the key card with the gate's RP ID and today's date, closes the session, and creates an `add_key` request at level `high` for the audience `phygital-key:keyring:<origin>`.
6. The key is not trusted yet. For the first key, the approval page asks it to sign its own `add_key`; only that credential is allowed to sign. Any later key must be approved by a key already in the keyring. A new key can never approve itself.

Attestation is `none`, so the gate cannot prove the key lives in hardware. The one-time link, printed only in your own terminal, is what protects the first key. See [WebAuthn](webauthn.md#registration).

## How finish checks before it stores

`finish` holds the state lock from start to end, so no other call can change the request or the keyring in between.

1. The request must be `pending` and have claims (Approve was pressed).
2. Pick who may sign:
   - first key: only the new card, and the keyring must still be empty;
   - adding a key that is already trusted, or removing one already gone: refused;
   - removing a key when only one is left: refused;
   - anything else: the current keyring.
3. Build the approval: canonical JSON of `{"claims", "signatures": [assertion]}`, base64url.
4. Run `check` (or `check_standing` for a standing approval) with the request's audience, request and level, the signers from step 2 and the gate's own origin and RP ID. That is every verifier check except replay. Any failure returns "The signature was not accepted (CODE). Nothing was approved." and changes nothing.
5. For a revocation: append the approval to `revocations.json` (temp file, then rename).
6. For a key change: build the next keyring and save it (next section).
7. Only now: status `approved`, the approval, the signer's name and `exp`; `usable` for a standing approval; audit lines `approved`, then `revocation_published` or `key_added` / `key_removed`; wake every waiter.

The MCP product's end-to-end suite sends a forged signature to the gate and checks that the request is still not approved afterwards.

## Saving the keyring

For a key change, `finish` copies the current keyring to `next`, adds or removes the card, and, when the keyring is signed (the first key, or a keyring that already has history), sets `origin` and appends the approval to `history`. Then:

1. `save_keyring` writes `next` to `keyring.tmp` (mode `0600`), syncs it to disk and renames it over `keyring.json`.
2. Only after the save succeeds does the gate switch its in-memory keyring to `next`.

A crash leaves either the old file or the new one, never half of one. If the save fails, the request stays `pending`, the in-memory keyring is unchanged and nothing is approved. Keyrings made before key history existed (v0.1.0) stay unsigned.

## Signed releases and updates

The secure install updates itself once a day, on by default ([decision 36](decisions.md)). An update runs new code as root, so a SHA-256 published next to the file is not enough: whoever can change the release files could replace both. Each product release therefore carries a file list signed with that product's release key, and every installer and updater checks that signature against public keys pinned in the product. The key is a secret of the GitHub environment `release` in the product's repository, which GitHub hands only to a release run from a `v*` tag, and only repository admins can push `v*` tags. For now no approval click is needed: the run signs and publishes as soon as the tag is pushed (decision 36). So a swapped file, a mirror or a stolen workflow token cannot make an update that installs, but whoever controls the owner's GitHub account can ([the update channel](threat-model.md#the-update-channel)). The base code is `src/update.rs` and the templates `assets/install-service.sh`, `assets/install.sh` and `assets/release-key.sh`; the release workflow (`.github/workflows/release.yml`) and the container (`packaging/container/Dockerfile`) are in each product.

```mermaid
flowchart TD
    T["Tag vX.Y.Z pushed by a repository admin, in the product repository"] --> B["CI: tests, builds 5 targets, writes .sha256 files and build records"]
    B --> S["publish job: hashes every archive, writes and signs release.txt, checks it against the pinned key"]
    S --> P["Release published with release.txt and release.txt.sig"]
    P --> IMG["container job: image built from the Linux archives, pushed to ghcr.io with a build record"]
    P --> F["Daily job fetches the latest release.txt and its signature"]
    F --> K{"Signed by a pinned release key?"}
    K -- no --> R1["Refused: nothing changes"]
    K -- yes --> N{"Newer than the installed version?"}
    N -- no --> U["Up to date: nothing changes"]
    N -- yes --> SA{"Standing approvals active, under 7 days?"}
    SA -- yes --> WT["Waiting to install: try the next day"]
    SA -- no --> H["Download, compare SHA-256 with the signed list"]
    H -- "differs" --> R2["Refused: nothing is installed"]
    H -- "matches" --> W["Swap the program, keep the old one, restart the gate"]
    W --> C{"Gate answers and only the gate listens?"}
    C -- yes --> OK["Updated"]
    C -- "no answer" --> BACK["Previous program put back and restarted"]
    C -- "another program on the port" --> KILL["Stop that program, start the gate again (up to 5 times)"]
    KILL --> C
    KILL -- "still taken" --> STOP["Gate stopped"]
```

### release.txt

Every product release has `release.txt` and `release.txt.sig`. The list is exact, line by line, LF line ends and a final LF. For Phygital Key MCP:

```text
phygital-key release manifest v1
version v0.4.0
<64 lowercase hex>  phygital-key-mcp-v0.4.0-aarch64-apple-darwin.tar.gz
<64 lowercase hex>  phygital-key-mcp-v0.4.0-x86_64-pc-windows-msvc.zip
```

The Rust parser (`verify_manifest` and `parse_manifest` in `update.rs`) refuses anything else:

| Rule | Exact behaviour |
|---|---|
| Size | At most 8192 bytes, valid UTF-8, ends with LF (CRLF is refused) |
| Line 1 | Exactly `phygital-key release manifest v1` (the same in every product) |
| Line 2 | `version vX.Y.Z`: decimal numbers, no leading zeros, compared by number (`v0.10.0` is newer than `v0.9.9`) |
| File lines | 64 lowercase hex digits, two spaces, a file name that starts with `<program>-vX.Y.Z-` (this product's program, the same version), ends in `.tar.gz` or `.zip`, is at most 128 characters and uses only letters, digits, `.`, `-` and `_` |
| Files | At least one, none listed twice |
| Signature | `release.txt.sig` is an ECDSA P-256 signature over SHA-256 of the exact bytes, DER-encoded (what `openssl dgst -sha256 -sign` writes), by one of the pinned keys |

Because the file names start with the program name, a release list of one product is refused by another. The shell installers check the signature with `openssl dgst -sha256 -verify`. The secure installer (`install-service.sh`, which runs as root) then applies the same strict format as the program: LF only, a final LF, at least one file line, every file line exactly `<64 hex>  <program>-<version>-<name>.tar.gz` or `.zip`, no name twice. `install.sh` (which runs as you) checks the first line, the version line and the line for the one file it downloads. Signatures with a high S value are accepted, which has no security effect. Five unit tests in `update.rs` cover the parser, the version order and reading PEM blocks.

### Pinned release keys

Each product keeps its public release keys in `release-keys.pem`, as PEM `PUBLIC KEY` blocks. The program compiles that file in (`Product::release_keys`), so `update` and the hidden `release-check` command trust those keys, and `product-files` writes the same keys into the `# BEGIN RELEASE KEYS` block of both installers. The product's CI fails if the installers in its repository differ from what `product-files` writes.

The product's `scripts/release-key.sh` is run once, by the repository owner, in a terminal (it refuses without one), with `gh` signed in as the owner and `openssl` installed. It makes one P-256 key in a temporary folder, creates the GitHub environment `release` in that repository (deployments only from tags `v*`), stores the private key as that environment's secret `RELEASE_SIGNING_KEY`, deletes the local copy, and pins the public key in `release-keys.pem` and the installers (the owner commits that change). It refuses if a key is already pinned. The private key is then only in GitHub: there is no copy on any computer, no passphrase and no backup key. If the key has to change, a release signed with the old key pins both the old and the new public key, and a later release removes the old one.

> [!IMPORTANT]
> A product with an empty `release-keys.pem` refuses every install and update: the secure installer says "no release key is pinned in this installer", and `install.sh` and `update` say the release list is not signed by a pinned release key. The release key must be made with the product's `scripts/release-key.sh` before its first release.

### Signed in the release run

These steps run in each product's `release.yml`; the names below are those of Phygital Key MCP.

1. Pushing a `v*` tag runs `release.yml`. Its build job runs the tests and a release build for 5 targets (Linux x86_64 and arm64, macOS Apple silicon and Intel, Windows x86_64), each packed as `.tar.gz` (`.zip` on Windows) with a `.sha256` file.
2. Each build job publishes a build provenance attestation for its archive (`actions/attest-build-provenance`, signed through Sigstore): GitHub's record of which workflow built which file from which commit. Anyone can check it with `gh attestation verify <file> --repo Phygital-Key/phygital-key-mcp`.
3. The `publish` job runs in the environment `release`, so it gets `RELEASE_SIGNING_KEY` only in a run from a `v*` tag. The environment has no required reviewer for now, so the job does not wait for an approval click. The owner can turn it back on under Settings, Environments, release, Required reviewers.
4. The job computes the SHA-256 of every archive itself, writes `release.txt`, signs it with `openssl dgst -sha256 -sign`, checks the signature against the keys pinned in that commit, and publishes the release (not a draft) with every archive, its `.sha256`, `release.txt` and `release.txt.sig`.
5. The `container` job (`needs: publish`) builds the image from the two Linux archives of the release for `linux/amd64` and `linux/arm64` (distroless `cc-debian12:nonroot` base pinned by digest, user `65532:65532`, `PHYGITAL_KEY_MCP_HOME=/data`, default command `--bind 0.0.0.0 serve`), pushes it to `ghcr.io/phygital-key/phygital-key-mcp` as `vX.Y.Z` and `latest`, and publishes a build provenance attestation for the image digest. The image is not in `release.txt`. A container updates only when its image is pulled again: the daily job and `update` do not run in it ([run it in a container](own-server.md#run-it-in-a-container)).

### The daily job

| | macOS | Linux |
|---|---|---|
| Job | LaunchDaemon `io.phygitalkey.<id>.update` (root, plist mode 0600), one random time of day chosen at install | `<program>-update.timer` (`OnCalendar=daily`, `RandomizedDelaySec=6h`, `Persistent=true`) starting `<program>-update.service` (`Type=oneshot`, `PrivateTmp`, `ProtectHome`) |
| Runs | `<prefix>/bin/<program> --port <port> install-service --auto-update-run` | the same |
| Output | `/Library/Logs/PhygitalKey/<id>-update/update.log`, in a folder only root can open | the journal: `journalctl -u <program>-update` |

| Product | `<id>` | `<program>` | `<prefix>` on macOS | `<prefix>` on Linux |
|---|---|---|---|---|
| Phygital Key MCP | `mcp` | `phygital-key-mcp` | `/Library/PhygitalKey/mcp` | `/usr/lib/phygital-key-mcp` |
| Phygital Key A2A | `a2a` | `phygital-key-a2a` | `/Library/PhygitalKey/a2a` | `/usr/lib/phygital-key-a2a` |

The installed program runs the installer script compiled into it, with bash started from an empty environment (only `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, root's `HOME`, `LANG=C` and the `PKA_*` values). The script sets that `PATH` and `HOME` again, clears variables such as `CURL_HOME`, `SSL_CERT_FILE`, `OPENSSL_CONF`, `BASH_ENV`, `LD_PRELOAD` and `DYLD_INSERT_LIBRARIES`, sets `umask 022`, and then:

1. fetches `https://github.com/<product repo>/releases/latest/download/release.txt` and `release.txt.sig` (`curl -q --proto '=https' --tlsv1.2`; `-q` means no `.curlrc` is read);
2. checks the signature with `openssl` and the pinned keys, or, if `openssl` is missing, with the installed root-owned program's `release-check`;
3. stops if the signed version is not strictly newer than the installed one;
4. asks the gate's `/api/status` for `standing_active`; if it is above zero, records `waiting to install ...` and stops, unless run with `--now`. The first wait on the installed version writes `<unix time> <installed version>` to `<prefix>/update-postponed`; a newer release does not restart that clock, and after 7 days the job installs anyway. Waiting requests (`waiting`) do not count, because an agent can make one at any time;
5. downloads the release file for this machine, compares its SHA-256 with the signed list, and unpacks it;
6. keeps a copy of the current program, moves the new one into place in one step, and restarts the gate;
7. waits up to about 30 seconds for the gate to answer at `/api/status`, and checks that only the gate user listens on the port. A program running as another user that took the port during the restart is stopped with `kill -9` and the gate started again, up to 5 times.

The result goes to `<prefix>/update-status`, one line, written by root and readable by everyone, for example `2026-10-18T03:12:00Z up to date (v0.4.0; latest signed release v0.4.0)`. Other lines: `update check failed: could not download the latest signed release list from GitHub (still on ...)`, `waiting to install ...: N standing approval(s) active. ...`, `update refused: the latest release list is not signed by a pinned release key (still on ...)`, `updated from ... to ... (signed release, SHA-256 checked)`, which ends with `; stopped programs that took the port of the gate during the restart: [uid U, pid P] ...` when the job had to stop such a program. Any error that stops the run (a download that does not match its signed SHA-256, for example) is recorded as `update stopped: <message>`.

`phygital-key-mcp-admin update` runs the same job now. `phygital-key-mcp-admin status` prints the program version, whether automatic updates are on, and the last result. `--no-auto-update` removes the job and writes `off` to `<prefix>/auto-update`, so later installs keep it off until `--auto-update`. With the install command from GitHub: `curl ... | sudo bash -s -- --no-auto-update`.

### Rollback

| What happens after the swap | Result |
|---|---|
| The new gate does not answer within about 30 seconds | The previous program is put back and started. Status: `update to vX.Y.Z failed: the new gate did not start, so vA.B.C is back` |
| The previous program does not answer either | The gate stays stopped. Status: `update stopped: the gate is stopped: the gate did not start. See: <prefix>/bin/<program>-admin logs. To install again: sudo <prefix>/bin/<program> install-service` |
| Another program takes the gate's port during the restart | That program is stopped (`kill -9`) and the gate started again, up to 5 times. The status line names it |
| The port is still taken after 5 tries, or was taken before the update started | The gate is stopped, never left running next to it. Status: `update stopped: the gate is stopped: another program (...) is listening on port ...` |

The product's CI secure-install test installs a program that exits at once and checks that the previous program is back and answering.

### What an update changes

An update restarts the gate. Requests still waiting for a decision are lost (they live in memory), and every standing approval stops working, because its use key lived only in memory. One-time approvals already signed keep working at services until their `exp`: services check them offline. The keyring, the audit log and the rest of the data folder are not touched.

Installs in your own folders do not update themselves. `install.sh` checks the same signed list (it needs `openssl`) and writes only `~/.local/bin/<program>`. `phygital-key-mcp update` (or `update --check`) fetches the latest signed list, refuses unless it is newer, checks the signed SHA-256 and swaps the program in place. It refuses on the secure install ("it updates itself every day"), on a Homebrew install (`brew upgrade phygital-key-mcp`) and on Windows (`scoop update phygital-key-mcp`, or download the new `.zip`).

The gate itself still makes no network calls. The secure install's daily job contacts github.com once a day (plus the release file when it updates); `install.sh` and `update` contact GitHub only when you run them.

> [!WARNING]
> The full download-and-update path has not been tested against GitHub yet: no product has published a release with this signed update channel. The product's CI test covers the parts that run without a signed release: the job is installed and root-owned, a list signed by an unknown key is refused, the job leaves the gate running when no newer signed release exists and records its result, root ignores a planted `~/.curlrc` and a fake `curl` early in `PATH`, a program that does not start is switched back, and the on/off choice is kept. Killing a program that takes the port during an update restart has no CI test.

## Next

- [How it works](how-it-works.md): the same flow in plain words
- [Approval page](approval-page.md): what the page checks before it signs
- [Testing](testing.md): what runs in the base and what runs in each product
- [Updates](updates.md): turning automatic updates on and off
