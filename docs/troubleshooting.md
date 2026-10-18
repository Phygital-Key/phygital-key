# Troubleshooting

Real problems, with the exact message you see and what to do. Find your message with your browser's search (Ctrl-F or Cmd-F).

**On this page:** [starting the gate](#starting-the-gate) · [adding a key](#adding-a-key) · [on the approval page](#on-the-approval-page) · [the AI app and MCP](#the-ai-app-and-mcp) · [standing approvals](#standing-approvals) · [at the service](#at-the-service) · [command line](#command-line) · [the demo](#the-demo) · [secure install](#secure-install) · [automatic updates](#automatic-updates) · [own server](#own-server)

## Starting the gate

**`phygital-key: cannot listen on 127.0.0.1:7468: Address already in use (os error 48). Another program is using this address (another phygital-key, or something pretending to be one). Refusing to start.`**

Something already holds the gate's address. (The error number differs between systems.) Usually it is another gate: your AI app running `phygital-key mcp`, a second AI app doing the same, a `phygital-key demo`, or the secure install's service. Find it:

```bash
lsof -nP -iTCP:7468 -sTCP:LISTEN       # macOS, Linux
```

Stop the extra one. If several AI apps need the gate, run one gate over HTTP and connect each app to it ([connect your AI app](connect-your-ai.md#two-ways-to-connect)). If the program holding the port is not one you know, do not approve anything until you know what it is: it could serve a fake approval page on the same address.

The gate holds both `127.0.0.1` and `[::1]` and refuses to start if either is taken. In `mcp` mode, the MCP side stops too, so the AI is never handed links to a page another program serves.

**`phygital-key: --origin must be https (passkeys only work on https, or on localhost)`**

Use an `https://` address, or leave `--origin` out for `http://localhost:<port>`. See [own server](own-server.md#why-https).

**`phygital-key: --origin is just scheme://host[:port], no path`**

Remove the path: `https://approve.example.com`, not `https://approve.example.com/gate`.

**`phygital-key: --lifetime must be 1 to 300 seconds`**

Approvals live at most 5 minutes. The default is 60.

**`phygital-key: audit log line 12: hash does not match (line changed)`** (or `chain broken (prev does not match)`, `sequence broken`, `no hash`, `not a JSON object`)

The gate checks the whole audit log when it starts and refuses to run on a changed one. Someone or something edited `audit.log`. Keep the file as evidence, find out what changed it, then restore it from a backup. `phygital-key audit-check` runs the same check and prints `audit log ok: <n> entries, last hash <hash>` when it is intact.

**`revocations.json is damaged; fix or restore it`**

The gate never quietly replaces this file, because earlier revocations would be lost. Restore it from a backup.

## Adding a key

**`phygital-key: no key enrolled yet. In your own terminal, run: phygital-key enroll (or phygital-key-admin enroll for the secure install)`**

Not an error. The gate started with an empty keyring. Run the command it names.

**``This enrollment link is not valid or has expired. Run `phygital-key enroll` for a new one.``**

One of these:

- The link is older than 10 minutes.
- It was already used, or a newer `enroll` replaced it (only the newest link works).
- The gate that is running uses a different data folder from the `enroll` you ran. With the secure install, use `phygital-key-admin enroll`. With `phygital-key mcp` started by an AI app, give `enroll` the same `--home` (or `PHYGITAL_KEY_HOME`) and `--port` as the app's command.

Run `enroll` again and open the new link right away.

**``This page needs the link printed by `phygital-key enroll`.``**

You opened `/enroll` without the `#token=...` part. Copy the whole link.

**`This key is already added.`** / **`This key is already enrolled.`**

This device already holds a key for this gate. Use a different device or security key, or keep the one you have.

**`This key type is not supported yet. Use a passkey or a FIDO2 key with ES256.`** / **`Only P-256 (ES256) keys are supported.`**

The authenticator made a key the gate cannot use. Passkeys on current phones, laptops and password managers, and FIDO2 security keys, make ES256 keys.

**`Almost done: confirm with the key you just made.`**

Not an error: the key is made but not trusted yet. Approve the **Confirm your key** page that follows.

**`Almost done: approve this new key with a key you already trust.`**

Not an error either. A new key is trusted only once a key you already have approves it ([add more keys](keys-and-devices.md#add-more-keys)).

**`The browser's response did not match this page. Start again from the link.`**

The browser signed for a different page address than the gate expects. Open the link exactly as printed (same host and port), not through another name for the same machine.

## On the approval page

**`The key prompt was closed or timed out. Nothing was signed.`**

You closed the passkey prompt, it timed out, or the key could not do what the request needs. Press **Approve with my key** again. If it keeps failing on a request that needs **Fingerprint, face or PIN**, your key cannot verify you: a security key without a PIN, for example, can only approve "A touch on your key" requests ([levels](keys-and-devices.md#levels-and-what-each-key-can-approve)). Another trusted key, or a PIN set on the security key, solves it.

**`This browser cannot use passkeys here. Open the link in a recent browser, over https or localhost.`**

The page is not in a secure context (it was opened over plain `http` on an address that is not `localhost`), or the browser has no passkey support. Use the exact link from the gate in a recent browser. For other devices, put the gate behind https ([own server](own-server.md)).

**`This page address cannot use passkeys. Open it through the exact link you were given.`**

The browser refused because the page's address does not match the key's host, for example `127.0.0.1` instead of `localhost`. Open the link exactly as given.

**`What would be signed does not match what is shown. Nothing was signed.`**

The page compared the claims the gate wants signed with what it shows, and they differ. This should never happen with an honest gate. Press **Deny**, and check what serves the page (`phygital-key-admin status` with the secure install).

**`The signature was not accepted (BAD_SIGNATURE). Nothing was approved.`** (or another code)

The gate checked the signature as a service would and refused it. Nothing was approved. Try again; if it repeats, see the [error codes](error-codes.md).

**`This request is no longer waiting.`**

It was decided, withdrawn or expired while the page was open. Reload to see its state.

**`Time is up. The answer is no.`**

The 10 minutes to decide ran out. Ask the AI to request it again if you still want it.

**`Request not found`** with "This link is wrong or the request was removed."

The id is unknown: the link is wrong, the gate restarted (waiting requests live only in memory), or the request ended over an hour ago.

**The home page says `✕ Check this: the key history does not check out.`**

The keyring's signed history does not match its keys or its address. This happens with a keyring from v0.1.0 (made before the signed history), after changing the gate's `--origin` or port, or after the file was edited. Do not share it with services. See [the signed key history](keys-and-devices.md#the-signed-key-history).

**`You cannot remove your only key. Add another key first.`** / **`A removal of this key is already waiting for approval.`**

Add a second key before removing the first. Approve or let expire the removal that is already waiting.

## The AI app and MCP

**The AI app shows a `401` or ``missing or wrong MCP access token (see `phygital-key mcp-token`)``**

The app connects over HTTP without the right `Authorization: Bearer <token>` header. Run `phygital-key mcp-token` (secure install: `phygital-key-admin token`) and paste the line again. Use the same `--home` as the running gate: each data folder has its own token.

**`421` and `{"error":"unknown host"}`**

The request used a host name the gate does not know. Connect to `localhost`, `127.0.0.1` or `[::1]` with the gate's port, or the exact host of `--origin`.

**`403` and `Forbidden: Origin header is not allowed` from `/mcp`**

The request carried a browser `Origin` header. `/mcp` refuses any call from a web page. Use an MCP client, not a browser page.

**The app cannot start `phygital-key`** (stdio)

Desktop apps may not search the same folders as your terminal. Use the full path from `which phygital-key` (macOS, Linux) or `where phygital-key` (Windows) as the command.

**`phygital-key: mcp: connection closed: initialize request`**

You ran `phygital-key mcp` by hand. It waits for an MCP client on its input; started without one, it stops. Let the AI app start it.

Messages the AI gets from the tools:

| Message | Meaning |
|---|---|
| ``No key is enrolled yet. Ask the person to run `phygital-key enroll` and add their passkey first.`` | Add your key first |
| `request must be {"type": "<lowercase name>", "params": {...}}` | The request has the wrong shape |
| `request has no canonical form: use strings or integers, never floats` | A number with a decimal point: send amounts as text (`"120.00"`) or whole minor units (`12000`) |
| `request text must be Unicode NFC (it is never rewritten, so look-alikes cannot sneak in)` | The text uses a decomposed or look-alike form (for example KELVIN SIGN) |
| `request is over 8192 bytes` / `request is nested too deeply` | Make the request smaller or flatter |
| `aud must be 1 to 256 visible ASCII characters naming who will act on the yes (a URL, domain or id)` | The audience has spaces, invisible or non-ASCII characters |
| `"standing" is reserved: use request_standing_approval` / `"revoke" is reserved: revoke a standing approval on its approval page` | Reserved request types |
| `this audience is reserved for the gate itself` | The gate's own key-change audience cannot be used |
| `Too many requests right now. Try again shortly.` | More than 30 new requests in a minute, or 5000 held |
| `Too many requests are waiting. Wait for the person to decide.` | 50 requests are waiting |
| `unknown request` | Wrong id, or the gate restarted |
| ``No receipt key is pinned for this service, so the receipt cannot be checked. Pin it with `phygital-key trust-service`.`` | See [receipts](receipts.md) |

## Standing approvals

| Message | What to do |
|---|---|
| `This standing approval cannot be used: unknown, revoked, or the gate restarted.` | It was revoked, or the gate restarted (which ends all standing approvals). Ask again if you still want it |
| `This standing approval is not active.` | It is not approved yet, or it ended |
| `This request is outside the limits the person approved. Ask for a one-time approval instead.` | The request breaks a limit: another value, an extra or missing field |
| `No uses left on this standing approval.` | All uses are spent |
| `days must be 1 to 30` | Standing approvals last 1 to 30 days |
| `vary.<field>: use {"min": n, "max": n} (both) or {"one_of": ["a", "b"]}` | Both ends of a range are required |
| `This standing approval would end too soon or too late. Ask again.` | Its end date (counted from when the AI asked) is less than a minute away |
| `Revoke the standing approval first.` / `The revocation is already signed and published.` | Revocation steps out of order, or already done |

## At the service

A service that checks approvals answers with an error code. The demo service answers `{"error":"<CODE>","ok":false}` with status 403.

| Code | Usual cause | What to do |
|---|---|---|
| `APPROVAL_REQUIRED` | No approval sent | Send it in the `Phygital-Approval` header |
| `MALFORMED` | Damaged or cut approval string, wrong kind of approval | Send the exact string `get_approval` returned |
| `WRONG_AUDIENCE` | Approved for another service | Ask again with this service's audience |
| `EXPIRED` | More than 60 seconds since Approve | Ask again, and send it right away |
| `REQUEST_MISMATCH` | The request differs from the approved one (even by one character) | Send exactly what was approved, or ask again |
| `ASSURANCE_TOO_LOW` | Approved with a touch; the service needs fingerprint, face or PIN | Ask again at level `high` |
| `UNTRUSTED_KEY` | The service's keyring does not have the signing key | Give the service your current keyring and fingerprint |
| `BAD_SIGNATURE` | Signature, origin or page address does not check out | Check the service's `origins` and `rp_ids` match your gate's address |
| `REPLAYED` | Already used | Ask again: each approval works once |
| `NOT_ALLOWED`, `USED_UP`, `REVOKED` | Standing approval limits, uses or revocation | See [standing approvals](standing-approvals.md) |

Every code, with the exact check behind it: [error codes](error-codes.md).

## Command line

**`keyring NOT ok: the history does not start with the pinned key`**

The fingerprint you gave is not this keyring's first key, or the file is another keyring. Copy the fingerprint again from the home page or `phygital-key keys`.

**`keyring NOT ok: the history was signed on another approval page`**

`--signed-on` must be the gate's exact origin, for example `http://localhost:7468` or `https://approve.example.com`.

**`phygital-key: with --file, say where it must be signed: --signed-on https://approve.example.com`**

Add `--signed-on <origin>` when you check a keyring file.

**`{"error":"REPLAYED","ok":false}` from `phygital-key verify`**

The command-line verifier remembers used approvals in `seen-ids.jsonl` in its data folder. The approval was checked once already on this machine.

**`phygital-key install-service: needs administrator rights (sudo).`**

See [secure install](#secure-install) below.

## The demo

**`phygital-key: demo service port 7469: Address already in use (os error 48)`**

The demo's fake service uses the gate's port plus one. Free that port, or move both: `phygital-key demo --port 7480` puts the gate on 7480 and the service on 7481. A keyring's signed history is tied to its address, so give a different port its own data folder too (`--home`).

**The demo's `Claude Code:` line stops working**

The demo prints the token from its data folder. If you started it with another `--home`, the token differs. Run `phygital-key mcp-token` with the same options.

## Secure install

| Message | What to do |
|---|---|
| `needs administrator rights (sudo).` | Run it with `sudo` |
| `only macOS and Linux for now.` | Windows is not supported yet |
| `another program (uid <u>, pid <p>) is listening on port 7468. Stop it, then run this again.` | Find it with `sudo lsof -nP -iTCP:7468 -sTCP:LISTEN`, stop it (often a quick-install gate an AI app started), run the install again |
| `needs openssl to check the release signature. Install openssl, then run this again.` | Install openssl with your system's package manager, then run the install again |
| `could not download the signed release list for <version>.` | Check the network, and that the version exists on the [Releases](https://github.com/Phygital-Key/phygital-key-mcp/releases) page |
| `the release list for <version> is not signed by a pinned release key. Nothing was installed.` | Do not work around it. Check that you used the pinned `curl` command, then report it ([SECURITY.md](../SECURITY.md)) |
| `the signed release list is for <got>, not <version>. Nothing was installed.` | The release files do not belong together. Report it ([SECURITY.md](../SECURITY.md)) |
| `the download does not match its signed SHA-256. Nothing was installed.` | Try again later; if it repeats, report it ([SECURITY.md](../SECURITY.md)) |
| `the gate did not start. See: <prefix>/bin/phygital-key-admin logs` | Read the logs |
| `<path> is not safe to run as root: ...` | `--binary` must be owned by root, with every folder above it, and not be a link |
| `WARNING: something other than the gate is listening on port 7468, or the gate is not running. Do not approve anything until this is fixed.` (from `status`) | Do not approve. Find the listener, then run the locked `install-service` again |
| `phygital-key-admin: command not found` | Use the full path: `/Library/PhygitalKey/bin/phygital-key-admin` (macOS) or `/usr/lib/phygital-key/bin/phygital-key-admin` (Linux) |

Every refusal is listed in [secure install](secure-install.md#what-it-checks-and-refuses).

## Automatic updates

The secure install's daily job writes one result line, shown by `phygital-key-admin status` after `last:`. Its full output is in `/Library/Logs/PhygitalKeyUpdate/update.log` (macOS, read it with `sudo`) or `journalctl -u phygital-key-update` (Linux). How the job works: [updates](updates.md#automatic-updates-secure-install).

| Result line | What to do |
|---|---|
| `up to date (v0.3.0; latest signed release v0.3.0)` | Nothing. No newer signed release exists |
| `update check failed: could not download the latest signed release list from GitHub (still on v0.3.0)` | The gate keeps running. The job could not download `release.txt` or its signature: no network, GitHub down, or a latest release without a signed list. It tries again the next day; `phygital-key-admin update` tries now |
| `update refused: the latest release list is not signed by a pinned release key (still on v0.3.0)` | The gate keeps running the old version. Do not install that release by hand. Report it ([SECURITY.md](../SECURITY.md)) |
| `update to v0.4.0 failed: the new gate did not start, so v0.3.0 is back` | The old version runs and the gate works. Read `phygital-key-admin logs` and the update log for why the new one failed, and report it. The next daily run tries again |
| `waiting to install v0.4.0: 1 standing approval(s) active. It installs once they end, after 7 days at the latest, or now with: phygital-key-admin update --now` | Nothing is wrong. The gate keeps running the old version until your standing approvals end, 7 days at most (counted from the first wait on the version you run). Waiting requests do not hold it back. To install now (ending them): `phygital-key-admin update --now` |
| `updated from v0.3.0 to v0.4.0 (signed release, SHA-256 checked); stopped programs that took the port of the gate during the restart: [uid <u>, pid <p>]` | The update worked. While the gate restarted, a program running as user `<u>` took its port, posing as the gate; the job stopped it (`kill -9`) and started the gate again. Find out what that program was (`ps`, your AI apps, recent installs): something on this machine tried to take the approval page's address |
| `update stopped: the gate is stopped: another program (uid <u>, pid <p>) is listening on port 7468. Stop it, then run this again. Do not approve anything until then.` | The port was taken before the update started, or was taken again after 5 tries to stop the program. The gate is stopped on purpose. Do not approve anything. Find the program (`sudo lsof -nP -iTCP:7468 -sTCP:LISTEN`), stop it, then run `sudo <prefix>/bin/phygital-key install-service` |
| `update stopped: the gate is stopped: the gate did not start. See: <prefix>/bin/phygital-key-admin logs. To install again: sudo <prefix>/bin/phygital-key install-service` | Neither the new nor the previous program started. Read the logs, then run `sudo <prefix>/bin/phygital-key install-service` |

Other messages in the update log. Those that start with `phygital-key install-service:` also become the result line, after `update stopped:`:

| Message | Meaning |
|---|---|
| `phygital-key install-service: the download does not match its signed SHA-256. Nothing was installed.` | The archive is not the one the signed list names. The gate was not touched |
| `phygital-key install-service: the signed release list has no <name>.tar.gz. Nothing was installed.` | The release has no archive for this machine |
| `phygital-key install-service: download failed: <url>` | The archive could not be downloaded. The gate was not touched |
| `The new gate did not start. Switching back to the previous program (v0.3.0).` | Printed before the switch back; the result line follows |

**`phygital-key update` (install script, cargo):**

| Message | What to do |
|---|---|
| `phygital-key: download of https://github.com/Phygital-Key/phygital-key-mcp/releases/latest/download/release.txt failed` | No network, or the latest release has no signed list. Nothing was changed |
| `phygital-key: release list is not signed by a pinned release key` | Nothing was changed. Do not install that release. Report it ([SECURITY.md](../SECURITY.md)) |
| `phygital-key: release list refused: <reason>` | The list is not in the exact format. Nothing was changed |
| `phygital-key: the download does not match the signed SHA-256. Nothing was changed.` | Try again later; if it repeats, report it |
| `phygital-key: this is the secure install: it updates itself every day. To update now: phygital-key-admin update` | Use `phygital-key-admin update` |
| `phygital-key: installed with Homebrew: run  brew upgrade phygital-key` | Use Homebrew |
| `phygital-key: on Windows, update with your package manager: scoop update phygital-key, or winget upgrade` | Use `scoop update phygital-key` |
| `phygital-key: <folder>/.phygital-key.update: Permission denied (os error 13)` | You cannot write the folder that holds the program. Update it the way you installed it |

## Own server

| Symptom | Cause |
|---|---|
| `421` `{"error":"unknown host"}` through the proxy | The proxy changes the `Host` header, or adds `:443`. Pass it through unchanged |
| `403` `{"error":"wrong origin"}` when you press a button | The proxy changes or drops the `Origin` header, or `--origin` differs from the address in the browser |
| Enrollment link or MCP line shows `http://localhost:7468` | `enroll` or `mcp-token` ran without `--origin`. Use the same options as `serve` |
| MCP answers stall | The proxy buffers streamed answers; turn buffering off for `/mcp` |

More in [own server](own-server.md).

## Next

- [Error codes](error-codes.md)
- [Getting started](getting-started.md)
- [FAQ](faq.md)
- [Report a security problem](../SECURITY.md)
