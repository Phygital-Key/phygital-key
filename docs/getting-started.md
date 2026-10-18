# Getting started

This page gets you from nothing to a first signed [approval](glossary.md). Pick the path that fits you: try the demo, protect your own AI app, or check approvals in a service you run.

- [Try the demo (about 2 minutes)](#try-the-demo)
- [Use it with your AI app](#use-it-with-your-ai-app)
- [Check approvals in a service](#check-approvals-in-a-service)

## Before you start

You need two things:

- **A key.** Any passkey works: Touch ID or Face ID on a Mac or iPhone, Windows Hello, an Android phone, a password manager that stores passkeys, or a FIDO2 security key. For your first key, the device must be able to check a fingerprint, face or PIN (see [keys and devices](keys-and-devices.md#levels-and-what-each-key-can-approve)).
- **A recent browser** (Chrome, Edge, Safari or Firefox). Passkeys only work on `https` pages or on `localhost`.

Install the program (pick one):

| Where | Command |
|---|---|
| macOS, Linux (Homebrew) | `brew install phygital-key/tap/phygital-key` |
| macOS, Linux (script) | `curl -fsSL https://raw.githubusercontent.com/Phygital-Key/phygital-key-mcp/main/install.sh \| sh` |
| Windows (Scoop) | `scoop bucket add phygital-key https://github.com/Phygital-Key/phygital-key-mcp` then `scoop install phygital-key` |
| Anywhere with Rust | `cargo install --locked --git https://github.com/Phygital-Key/phygital-key-mcp phygital-key` |
| By hand | Download from [Releases](https://github.com/Phygital-Key/phygital-key-mcp/releases). Each release has a signed list of SHA-256 sums, `release.txt` ([how to check it](updates.md#check-a-download-yourself)). |

The script downloads the latest release list (`release.txt`), checks that it is signed by a release key pinned in the script, downloads the archive for your machine, compares its SHA-256 with the signed list and writes one file, `~/.local/bin/phygital-key`. Nothing else is touched. It needs `curl`, `tar` and `openssl`. Later, `phygital-key update` installs newer signed releases ([updates](updates.md)).

Check that it works:

```bash
phygital-key --version
```

```text
phygital-key 0.3.0
```

## Try the demo

The demo runs the gate and a fake service on your computer. The fake service deploys, pays and sends messages only in words: nothing real ever happens, and it refuses every action that does not carry a valid approval.

1. Start it:

   ```bash
   phygital-key demo
   ```

   It prints something like this (your tokens differ):

   ```text
   phygital-key: no key enrolled yet. Open this link to add your passkey (valid 10 minutes, works once):
     http://localhost:7468/enroll#token=G7ZNXW4m...
   phygital-key demo
     approval page   http://localhost:7468
     MCP endpoint    http://localhost:7468/mcp  (needs header  Authorization: Bearer o63AlJdL...)
     Claude Code:    claude mcp add --transport http phygital-key http://localhost:7468/mcp --header "Authorization: Bearer o63AlJdL..."
     demo service    http://localhost:7469  (fake deploy, payment, send_message)
   Add the MCP endpoint to your AI app, then ask it to deploy `api` 2.4.1 to production through the demo service.
   ```

2. **Add your key.** Open the enrollment link, type a name (for example `My laptop`) and press **Add key**. Your browser asks for Touch ID, Windows Hello, your phone or a security key. The page then shows **Confirm your key**: press **Approve with my key** once more. This first signature proves the key works and starts your [signed key history](keys-and-devices.md#the-signed-key-history).

3. **Connect an AI app.** Paste the `Claude Code:` line the demo printed into a terminal, or put the endpoint and header into any other app ([connect your AI app](connect-your-ai.md) shows each shape).

4. **Ask for something.** In the AI app, ask:

   > Deploy api 2.4.1 to production through the demo service at http://localhost:7469.

   The AI calls `request_approval` and gives you a link (some apps open it for you). The page shows `deploy`, `env production`, `service api`, `version 2.4.1`, who carries it out (`http://localhost:7469`) and the proof needed (fingerprint, face or PIN). Read it, then press **Approve with my key** or **Deny**. [Approving](approving.md) explains every part of that page.

5. **Watch the service check it.** If you approved, the AI sends the approval to the demo service, which checks it itself and answers `Deployed (demo: nothing real happened)`. Within the same minute, ask the AI to send the same approval again: the service answers `REPLAYED`. Ask it to deploy `2.4.2` with the approval for `2.4.1`: the service answers `REQUEST_MISMATCH`. After the minute, any reuse answers `EXPIRED`.

Stop the demo with Ctrl-C. Your key stays in the data folder (`~/.phygital-key`), so the next `phygital-key demo` or `phygital-key mcp` already knows it.

> [!NOTE]
> The demo prints the enrollment link because it runs in your own terminal. `phygital-key mcp`, which AI apps start, never prints one: its output goes to the AI app's logs, where the AI could read it.

## Use it with your AI app

There are two ways to run the gate for real use. Both give the AI the same six tools, and in both the AI can ask, wait, cancel and report, but never approve.

| | Quick install | Secure install |
|---|---|---|
| Runs as | You | Its own system user (`_phygitalkey` on macOS, `phygitalkey` on Linux) |
| Can an AI acting as you change the gate's program, data or keys? | Yes, if it has shell access as you | No, not without your administrator password |
| Systems | macOS, Linux, Windows | macOS, Linux |
| Started by | Your AI app (`phygital-key mcp`) | The system at boot (launchd or systemd) |
| AI app connects over | stdio (the app starts it) | HTTP, `http://localhost:7468/mcp` with an access token |
| Updates | You run `phygital-key update`, `brew upgrade` or `scoop update` | By itself, daily, signed releases only (can be turned off) |
| Good for | Trying it, low-stakes work | Anything that matters |

### Quick install

1. Install the program (table above).
2. Add the gate to your AI app. For Claude Code:

   ```bash
   claude mcp add phygital-key -- phygital-key mcp
   ```

   Other apps take the same command in their MCP settings: command `phygital-key`, argument `mcp`. See [connect your AI app](connect-your-ai.md).
3. Start the AI app once, so it starts the gate. The gate's approval page then runs at `http://localhost:7468` for as long as the app keeps it running.
4. In your own terminal, add your key:

   ```bash
   phygital-key enroll
   ```

   Open the link it prints (it works once, for 10 minutes), press **Add key**, then confirm with the same key.
5. Check it works: ask the AI to call `request_approval` for a harmless `send_message` ([the exact test](connect-your-ai.md#check-that-it-works)).

> [!WARNING]
> With the quick install, the gate's files live in your own home folder. An AI that can run shell commands as you could edit them. Use the secure install for anything real.

### Secure install

From a fresh terminal that no AI controls (macOS or Linux):

```bash
curl -fsSL https://raw.githubusercontent.com/Phygital-Key/phygital-key-mcp/v0.3.0/crates/phygital-key/assets/install-service.sh | sudo bash
```

It downloads the official v0.3.0 release, checks that its file list is signed by a pinned release key and that the archive matches the signed SHA-256, installs it where only an administrator can change it, and starts it as its own user. From then on it updates itself once a day to newer signed releases ([updates](updates.md#automatic-updates-secure-install); add `-s -- --no-auto-update` after `sudo bash` to turn that off). Then:

```bash
phygital-key-admin enroll      # add your key (prints a one-time link)
phygital-key-admin token       # the line to paste into your AI app
phygital-key-admin status      # check that only the gate listens on its port
```

Each command asks for your administrator password. Paste the line from `token` into your AI app ([HTTP setup](connect-your-ai.md#http-the-app-connects-to-a-url)). Everything the install creates, checks and refuses is on [secure install](secure-install.md).

### Habits that keep you safe

- [ ] Add a second key (phone and laptop), so losing one does not lock you out ([how](keys-and-devices.md#add-more-keys)).
- [ ] Read every row before you approve. Approve only what you expected to see.
- [ ] Share your key fingerprint (on the home page, `http://localhost:7468`) with services that check your approvals.
- [ ] Keep secrets (passwords, card numbers) out of requests: approvals are signed, not encrypted ([privacy](privacy.md)).

## Check approvals in a service

If you run the service that acts (a deploy system, a payment API, a mail sender), you are the **verifier**. Your service receives the action plus an `approval` string, usually in a `Phygital-Approval` header. Before acting, it checks the approval itself, offline: right request to the last character, meant for your service, in time, signed by a trusted key at the level you require, never used before.

The fastest check from any language is the command line:

```bash
phygital-key verify --approval "$APPROVAL" \
  --request '{"type":"deploy","params":{"service":"api","env":"production","version":"2.4.1"}}' \
  --aud https://deploy.example.com --level high
```

It prints `{"approved_by":"My laptop","exp":...,"id":...,"level":"high","ok":true}` and exits 0, or prints `{"error":"REQUEST_MISMATCH","ok":false}` (or another [error code](error-codes.md)) and exits 1. A second check of the same approval gives `REPLAYED`.

[Integrate](integrate.md) covers the Rust and JavaScript libraries, standing approvals, revocations, receipts and how to trust a keyring.

## Next

- [Connect your AI app](connect-your-ai.md): exact settings for stdio and HTTP
- [Approving](approving.md): read the approval page line by line
- [Secure install](secure-install.md): put the gate out of the AI's reach
- [Troubleshooting](troubleshooting.md): exact messages and what to do
