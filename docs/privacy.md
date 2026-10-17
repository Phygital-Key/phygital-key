# Privacy

What Phygital Key stores, where, who can read a request, what leaves your machine, and how to delete everything. In short: everything stays on the machine that runs the gate, the gate itself sends nothing anywhere, the secure install checks GitHub once a day for signed updates, and approvals are signed so anyone can check them, not encrypted.

## What is stored, and where

The gate keeps a few files in its data folder:

| Install | Data folder | Who can open it |
|---|---|---|
| Quick install, demo | `~/.phygital-key` (`%USERPROFILE%\.phygital-key` on Windows), or `--home` / `PHYGITAL_KEY_HOME` | You (mode 0700 on macOS and Linux) |
| Secure install, macOS | `/Library/Application Support/PhygitalKey` | Only the gate user `_phygitalkey` (0700) |
| Secure install, Linux | `/var/lib/phygital-key` | Only the gate user `phygitalkey` (0700) |

Every file is private to its owner (mode 0600 on macOS and Linux). Files that are rewritten go through a temporary file and a rename, so a crash never leaves half a file; the audit log is only ever appended to.

| File | What is in it | Secret? |
|---|---|---|
| `keyring.json` | Your public key cards (credential id, public key, host, the name you gave each key, the date) and the signed history of key changes | No: it is meant to be shared with services |
| `audit.log` | One line per event (below). Never the request itself | No secrets, but it shows your activity |
| `mcp-token` | The access token for HTTP MCP (32 random bytes) | Yes |
| `enroll-token.json` | A one-time enrollment token (32 random bytes) and its expiry. Deleted when used | Yes, for 10 minutes |
| `revocations.json` | Your signed revocations of standing approvals | No: published for services |
| `services.json` | Receipt keys of services you pinned with `trust-service` | No |
| `seen-ids.jsonl` (and `.lock`) | Ids and expiry times of approvals already checked by `phygital-key verify` | No |

The secure install's macOS log (`/Library/Logs/PhygitalKey/gate.log`, readable only by the gate user) and the Linux journal hold what `serve` prints: its address and warnings. `serve` never prints the enrollment link or the MCP token.

The secure install's update job keeps its own output (`/Library/Logs/PhygitalKeyUpdate/update.log` on macOS, root only; the journal under `phygital-key-update` on Linux) and one line with the last result in `<prefix>/update-status`, next to the `on` or `off` choice in `<prefix>/auto-update`. They hold versions, times and download messages, nothing about your requests or keys.

### Kept only in memory

- **Waiting and decided requests**, including the request text, until about an hour after they end. A restart forgets them.
- **Standing approvals** and their signing keys (the `use_key`), until their end date. A restart ends them.
- **Enrollment sessions**, for up to 10 minutes.

### What the audit log records

Each line is a hash-chained JSON object with `seq`, `ts` (Unix time), `event`, `prev` and `hash`, plus:

| Event | Extra fields |
|---|---|
| `requested` | request `id`, `aud`, `level`, `request_hash` |
| `approved` | `id`, `aud`, `level`, `request_hash`, `credential_id` of the key that signed |
| `denied`, `cancelled`, `revoked` | `id`, `aud`, `request_hash` |
| `standing_used` | standing `id`, `use_id`, `aud`, `request_hash` |
| `receipt` | `id`, `use_id`, `aud`, `status`, `reference`, `verified` |
| `key_added`, `key_removed` | `credential_id`, the key's `name`, `approved_with` |
| `revocation_published` | `standing_id`, `approved_with` |

The request itself (who you paid, what you deployed, the message text) is never written. Only its SHA-256 (`request_hash`) is: enough to match a line to an approval you hold, not enough to read it. The [audit log](audit-log.md) page explains the chain and its limits.

## What leaves your machine

The gate (`mcp`, `serve`, `demo`) makes no outgoing network connections. It has no analytics, no telemetry, no account and no cloud. The approval page loads nothing from other sites: its fonts and logo are built into the program, and its Content Security Policy (`default-src 'none'`, with only `'self'` allowed for scripts, styles, fonts and connections) stops the browser from fetching anything else.

Network use happens only where you start it:

| What | Connects to | When |
|---|---|---|
| The install script (`install.sh`) | `github.com` (`release.txt`, its signature and the archive) and `raw.githubusercontent.com` (the script itself) | When you run it |
| `phygital-key update` | `github.com` (`release.txt`, its signature, and the archive when there is a newer release) | When you run it |
| The secure install | `github.com` (`release.txt`, its signature and the archive) | When you install or update by hand |
| The secure install's daily update job | `github.com`: `release.txt` and `release.txt.sig` of the latest release, plus the archive when a newer signed release exists | Once a day, while automatic updates are on |
| Homebrew, Scoop, cargo | Their own sources | When you install or update |
| The AI agent | The service it acts on, with your approval | When you approve |

The daily job sends no data about you or your gate: it is two plain https downloads with `curl` (and a third when it updates). GitHub sees what any download shows: your machine's IP address, the time, and curl's standard `User-Agent`. To stop it, turn automatic updates off: `sudo <prefix>/bin/phygital-key install-service --no-auto-update` ([updates](updates.md#turn-automatic-updates-off-or-on)).

Your passkeys belong to your device, browser or password manager. If you use a synced passkey, that provider syncs it under its own terms; Phygital Key never sees the private key.

## Signed, not encrypted

An approval proves who said yes to what. It does not hide what. It is a base64url-encoded JSON object holding the full request in plain text, plus a signature (ES256 over P-256) that anyone with your public key can check.

Who can read a request:

- **The AI agent** that made it.
- **The gate**, which shows it on the approval page.
- **The service** that acts on it, which needs the request to run it.
- **Anyone who holds the approval string**: the AI app's logs, a proxy that sees the call to the service, a copy kept by the service.

So keep secrets out of requests. Put a reference in the request (`"invoice": "INV-118"`), not a password, card number or private message. The `request_approval` tool tells the AI the same: "No secrets: the person sees it, and anyone holding the approval can read it."

Why not encrypt: the approval page is served by the gate, so encrypting a request against the gate only helps if the page comes from somewhere the gate cannot change. On your own computer the AI is already on that machine. That choice is [decision 33](decisions.md); it would be revisited if a hosted gate were ever built.

What does protect requests on the way:

- **localhost:** traffic between your browser and the gate never leaves your computer.
- **Own server:** passkeys require https for any address other than `localhost`, so the page and `/mcp` are reached over TLS ([own server](own-server.md)).
- **The page** sends `no-store` (no caching), `no-referrer`, `nosniff`, and refuses to be framed by other sites.

> [!NOTE]
> On a public address, the gate publishes your key cards (including the names you gave your keys) and your signed revocations at `/.well-known/`, readable by any website, so services can fetch them. On `localhost` other websites cannot read them. Requests, approvals and the audit log are never published.

## Delete everything

**Quick install or demo:**

1. Stop the gate (quit the AI app that runs `phygital-key mcp`, or stop `serve` / `demo`).
2. Delete the data folder: `rm -rf ~/.phygital-key` (Windows: delete `%USERPROFILE%\.phygital-key`). Also delete any folder you passed as `--home`.
3. Remove the program: `brew uninstall phygital-key`, `rm ~/.local/bin/phygital-key`, `scoop uninstall phygital-key`, or `cargo uninstall phygital-key`.
4. Remove the `phygital-key` entry from your AI apps' MCP settings (for Claude Code: `claude mcp remove phygital-key`).

**Secure install:**

```bash
sudo /Library/PhygitalKey/bin/phygital-key install-service --uninstall --purge    # macOS
sudo /usr/lib/phygital-key/bin/phygital-key install-service --uninstall --purge   # Linux
```

This removes the service, the daily update job, the program, the helper, the data folder, the gate and update logs and the gate user, and prints `Removed Phygital Key, its user, keys and log.`

**Your passkeys** are not in the data folder. They stay on your phone, laptop, password manager or security key, listed under the name you gave them. Delete them there if you no longer want them.

**Copies elsewhere:** services you gave your keyring to keep their copy, and services keep their own logs of approvals they acted on. Ask them if you want those removed.

## Next

- [Audit log](audit-log.md)
- [Threat model](threat-model.md)
- [Secure install](secure-install.md)
- [Decisions](decisions.md)
