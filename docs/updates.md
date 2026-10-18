# Updates

How Phygital Key gets new versions for each way of installing it: the secure install's automatic daily updates, `phygital-key update` for installs in your own folders, package managers, and how to check a release yourself. For anyone who runs the gate.

**On this page:** [how often](#how-often) · [by install method](#update-by-install-method) · [automatic updates](#automatic-updates-secure-install) · [phygital-key update](#phygital-key-update-install-script-and-cargo) · [package managers](#package-managers) · [the signed release list](#the-signed-release-list) · [after updating](#after-updating) · [what carries over](#what-carries-over) · [check a download yourself](#check-a-download-yourself) · [limits](#limits)

## How often

There is no fixed release schedule. New versions appear on the [Releases](https://github.com/Phygital-Key/phygital-key-mcp/releases) page, and what changed is in [CHANGELOG.md](../CHANGELOG.md). While the project is at 0.x, only the latest release gets fixes ([SECURITY.md](../SECURITY.md)).

See what you have:

```bash
phygital-key --version
```

```text
phygital-key 0.3.0
```

## Update, by install method

| You installed with | How it updates | Update now |
|---|---|---|
| The [secure install](secure-install.md) | By itself, once a day, signed releases only ([below](#automatic-updates-secure-install)) | `phygital-key-admin update` |
| The install script (`install.sh`) | When you run the command | `phygital-key update` (or `phygital-key update --check` to only look) |
| cargo | When you run the command | `phygital-key update`, or build again: `cargo install --locked --force --git https://github.com/Phygital-Key/phygital-key-mcp phygital-key` |
| Homebrew | With Homebrew | `brew update` then `brew upgrade phygital-key` |
| Scoop (Windows) | With Scoop | `scoop update` then `scoop update phygital-key` |
| Container (`ghcr.io/phygital-key/phygital-key`) | When you pull a new image | `docker pull ghcr.io/phygital-key/phygital-key:latest`, then start a new container on the same volume ([how](own-server.md#run-it-in-a-container)) |
| By hand | When you download it | Download the new archive, [check it](#check-a-download-yourself), replace the program file |

Each install is updated on its own. Updating your own `phygital-key` (Homebrew, the script, cargo) does not update the secure install, which runs its own locked copy, and the other way round.

> [!NOTE]
> Coming from v0.2.0 or older: those versions have no `phygital-key update` and no automatic updates. Install v0.3.0 once the way you installed before (the secure install: run the pinned `curl` command from [secure install](secure-install.md#install) again; the script: run it again). From then on, the paths on this page work.

## Automatic updates (secure install)

The secure install sets up a daily job, owned by root, that installs a newer release only when its file list is signed by a pinned release key. It is on by default.

| | macOS | Linux |
|---|---|---|
| Job | launchd LaunchDaemon `io.phygitalkey.update` (`/Library/LaunchDaemons/io.phygitalkey.update.plist`), runs as root | systemd `phygital-key-update.timer` and `phygital-key-update.service` (`/etc/systemd/system/`), a oneshot run as root with `PrivateTmp` and `ProtectHome` |
| When | Once a day at a time of day picked at random when you install (printed at the end of the install). If the Mac was asleep at that time, launchd runs it when the Mac wakes | `OnCalendar=daily` with `RandomizedDelaySec=6h`: once a day, between midnight and 06:00. `Persistent=true` runs a missed check after the machine was off |
| Output | `/Library/Logs/PhygitalKeyUpdate/update.log` (folder 0700, root only) | The journal: `journalctl -u phygital-key-update` |
| Last result | `/Library/PhygitalKey/update-status` | `/usr/lib/phygital-key/update-status` |

Both jobs run `<prefix>/bin/phygital-key --port <port> install-service --auto-update-run` (a hidden option). Each run does this:

```mermaid
flowchart TD
    A["Daily job, as root"] --> B["Download release.txt and release.txt.sig of the latest release from github.com"]
    B -->|"download fails"| R1["Result: update check failed. The gate keeps running"]
    B --> C{"Signed by a pinned release key?"}
    C -->|"no"| R2["Result: update refused. The gate keeps running"]
    C -->|"yes"| D{"Newer than the installed version?"}
    D -->|"no"| R3["Result: up to date. Nothing changes"]
    D -->|"yes"| W{"Standing approvals active, for less than 7 days?"}
    W -->|"yes"| R6["Result: waiting to install. Try again the next day"]
    W -->|"no"| E["Download the archive for this machine and compare its SHA-256 with the signed list"]
    E -->|"no match"| R4["Stop. Nothing was installed"]
    E -->|"match"| F["Keep the old program, swap in the new one, restart the gate"]
    F --> G{"Gate answers on /api/status and only the gate user listens?"}
    G -->|"yes"| R5["Result: updated"]
    G -->|"another program took the port"| K["Stop that program and start the gate again, up to 5 times"]
    K --> G
    G -->|"the new gate does not start"| H["Put the previous program back and start it again"]
```

In words:

1. It downloads `release.txt` and `release.txt.sig` from `https://github.com/Phygital-Key/phygital-key-mcp/releases/latest/download/` over https (TLS 1.2 or newer). Every `curl` starts with `-q`, so it never reads a `.curlrc` file.
2. It checks the signature against the release keys pinned in the installed program's own installer script ([the signed release list](#the-signed-release-list)). Any doubt is a refusal.
3. It compares the signed version with the installed one, number by number (`v0.10.0` is newer than `v0.9.9`). Only a strictly newer version goes on. An automatic update never installs an older version.
4. If any standing approval is active, it waits ([waiting while you are busy](#waiting-while-you-are-busy)).
5. It downloads the archive for this machine and compares its SHA-256 with the value in the signed list. The `.sha256` file next to the archive is not used.
6. It keeps a copy of the current program, swaps the new one in with one rename, and restarts the gate.
7. It runs the same start checks as a fresh install: the gate must answer within about 30 seconds, and no program other than the gate user may listen on the port. If a program running as another user takes the port while the gate restarts, the job stops that program (`kill -9`) and starts the gate again, up to 5 times, and names it in the result line. If the port is still taken after that, the gate is stopped and the result says so. If the new gate does not start, the job puts the previous program back and starts that one.

Steps 1 to 5 do not touch the running gate. Only an update that passes them restarts it.

**Root starts from a clean environment.** On macOS, `sudo` keeps your `HOME` (and your `PATH` when no `secure_path` is set). So `install-service`, which the daily job and `phygital-key-admin update` both run, starts bash with an empty environment: only `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, root's own `HOME` (`/var/root` on macOS, `/root` on Linux), `LANG=C` and the installer's `PKA_*` values. The script itself sets that `PATH` and `HOME` again as its first step and clears variables such as `CURL_HOME`, `SSL_CERT_FILE`, `OPENSSL_CONF`, `BASH_ENV`, `LD_PRELOAD` and `DYLD_INSERT_LIBRARIES`, which also covers `curl ... | sudo bash`. The admin helper sets `PATH` to system folders before it calls `sudo`. A `~/.curlrc` or a fake `curl`, `awk` or `openssl` early in your `PATH` therefore never runs as root (secure-install checks "root ignores the user's ~/.curlrc" and "root ignores programs early in the user's PATH").

### See the last result

```bash
phygital-key-admin status
```

```text
program:      phygital-key 0.3.0
auto-update:  on  (last: 2026-10-18T03:12:00Z up to date (v0.3.0; latest signed release v0.3.0))
ok: only the gate (_phygitalkey) listens on port 7468
{"history_ok":true,"keys":[...],"origin":"http://localhost:7468","root_fingerprint":"...","rp_id":"localhost","standing_active":0,"waiting":0}
```

Before the first daily run, `last:` says `not run yet`. The result line is one of these (time in UTC first):

| Result | Meaning |
|---|---|
| `up to date (v0.3.0; latest signed release v0.3.0)` | Nothing newer is signed. Nothing changed |
| `updated from v0.3.0 to v0.4.0 (signed release, SHA-256 checked)` | The new version runs and passed the start checks |
| `updated from v0.3.0 to v0.4.0 (signed release, SHA-256 checked); stopped programs that took the port of the gate during the restart: [uid 501, pid 1234]` | Updated, but while the gate restarted a program running as another user took its port, posing as the gate. The job stopped it and started the gate again. Find out what that program was |
| `update check failed: could not download the latest signed release list from GitHub (still on v0.3.0)` | `release.txt` or its signature could not be downloaded |
| `waiting to install v0.4.0: 1 standing approval(s) active. It installs once they end, after 7 days at the latest, or now with: phygital-key-admin update --now` | A newer signed release exists, but installing it now would end active standing approvals. See [waiting while you are busy](#waiting-while-you-are-busy) |
| `update refused: the latest release list is not signed by a pinned release key (still on v0.3.0)` | The list is missing a valid signature. Nothing was installed |
| `update to v0.4.0 failed: the new gate did not start, so v0.3.0 is back` | The new version failed its start checks. The previous program runs again |
| `update stopped: ...` | The run stopped before installing (for example `update stopped: the download does not match its signed SHA-256. Nothing was installed.`), or, with `the gate is stopped:` in it, the gate is stopped and needs you. See [troubleshooting](troubleshooting.md#automatic-updates) |

Every stopped run also leaves its message in the update log or the journal.

### Update now

```bash
phygital-key-admin update
```

This runs the daily job at once (it asks for your administrator password): it installs the latest signed release if it is newer, and otherwise records `up to date`. Like the daily job, it waits while standing approvals are active. `phygital-key-admin update --now` installs anyway (and so ends them).

### Waiting while you are busy

Installing restarts the gate, and a restart ends every standing approval (their use keys live only in memory). So before it installs, the daily job asks the gate how many standing approvals are active (`standing_active` in `/api/status`). If one or more are, it records the `waiting to install` line and tries again the next day.

Requests still waiting for you do not hold an update back. An agent can make a request at any time, so counting them would let an agent keep updates away for good. `/api/status` still reports them as `waiting`; they are lost on the restart, and the agent asks again.

The 7 days are counted per installed version. The first wait on the version you run writes `<unix time> <installed version>` to `<prefix>/update-postponed`, and a newer release that appears meanwhile does not restart the clock. After 7 days the job installs anyway, so a security fix arrives within a week. An agent cannot start this wait on its own, because a standing approval needs your passkey. Your own active standing approval can still hold a security update back for up to 7 days.

To install one chosen version, newer or older, use the locked program with `--release`:

```bash
sudo /Library/PhygitalKey/bin/phygital-key install-service --release v0.4.0      # macOS
sudo /usr/lib/phygital-key/bin/phygital-key install-service --release v0.4.0     # Linux
```

(`v0.4.0` is an example: use a version from the [Releases](https://github.com/Phygital-Key/phygital-key-mcp/releases) page.) The same signed-list check applies. With automatic updates on, the next daily run moves an older version up to the latest signed release again.

### Turn automatic updates off or on

```bash
sudo /Library/PhygitalKey/bin/phygital-key install-service --no-auto-update   # off (macOS)
sudo /Library/PhygitalKey/bin/phygital-key install-service --auto-update      # on again
```

(On Linux: `/usr/lib/phygital-key/bin/phygital-key`.) To install with updates off from the start:

```bash
curl -fsSL https://raw.githubusercontent.com/Phygital-Key/phygital-key-mcp/v0.3.0/crates/phygital-key/assets/install-service.sh | sudo bash -s -- --no-auto-update
```

The choice is saved as `on` or `off` in `<prefix>/auto-update` and kept on later installs until you pass the other option. `--no-auto-update` removes the job; `phygital-key-admin update` still works when you want it. Both options run the whole install again for the installed version, so the gate restarts.

### What automatic updates need

- **openssl**, to check the signature. macOS has an `openssl` command; most Linux systems do too. If it is missing, the job uses the installed program's own hidden `release-check` command instead, which checks the same pinned keys (only when that program is root-owned in root-only folders). A fresh install without openssl stops with `needs openssl to check the release signature. Install openssl, then run this again.`
- **curl** and https access to `github.com`.
- **A gate restart.** An update restarts the gate: requests still waiting for a decision are lost, and every [standing approval](standing-approvals.md) ends, because both live only in memory. Approvals you already signed and the AI already fetched keep working at the service until they expire. Days without a newer release cause no restart.

## `phygital-key update` (install script and cargo)

For a `phygital-key` in your own folders, such as `~/.local/bin` from the install script, or `~/.cargo/bin` from cargo:

```bash
phygital-key update --check    # only say whether a newer signed release exists
phygital-key update            # install it
```

```text
phygital-key v0.3.0 is up to date (latest signed release: v0.3.0).
```

```text
phygital-key v0.4.0 is available (you have v0.3.0). Run: phygital-key update
```

```text
Updated /Users/you/.local/bin/phygital-key from v0.3.0 to v0.4.0 (signed release, SHA-256 checked).
Restart your AI app, or any running phygital-key serve, to use it.
```

It downloads the latest `release.txt` and its signature from GitHub with `curl`, checks the signature itself (ECDSA P-256 with SHA-256, against the same pinned release keys as the installers, so it needs no openssl), refuses anything that is not strictly newer, downloads the archive for this machine, compares its SHA-256 with the signed list, and renames the new program over the old one. The folder that holds the program must be writable by you. A cargo install is replaced by the official release build.

It refuses where another updater is in charge:

| Install | It answers |
|---|---|
| Secure install | `phygital-key: this is the secure install: it updates itself every day. To update now: phygital-key-admin update` |
| Homebrew | `phygital-key: installed with Homebrew: run  brew upgrade phygital-key` |
| Windows | `phygital-key: on Windows, update with Scoop (scoop update phygital-key), or download the new .zip from the Releases page` |

The install script itself also checks the signed release list (it needs `openssl` for that) and ends with `Update later with: phygital-key update`. To pin a version with the script, put `PHYGITAL_KEY_VERSION=v0.3.0` before `sh`.

## Package managers

- **Homebrew:** `brew update`, then `brew upgrade phygital-key`. Homebrew checks the archive against the SHA-256 in the formula of the tap `Phygital-Key/homebrew-tap`.
- **Scoop:** `scoop update`, then `scoop update phygital-key`. Scoop installs the version and SHA-256 listed in the project's bucket file ([`bucket/phygital-key.json`](../bucket/phygital-key.json)), which is updated separately from each release.
- **winget:** manifests are generated in [`packaging/winget`](../packaging/winget) but are not submitted to Microsoft, so `winget upgrade` does not find Phygital Key yet.

Package managers do not read the signed release list. They rely on their own checksums.

## The signed release list

Every release carries two extra files:

- `release.txt`: the version and the SHA-256 of each archive, exactly in this form (LF line ends, final LF):

  ```text
  phygital-key release manifest v1
  version v0.4.0
  <64 lowercase hex>  phygital-key-v0.4.0-aarch64-apple-darwin.tar.gz
  <64 lowercase hex>  phygital-key-v0.4.0-x86_64-pc-windows-msvc.zip
  ```

- `release.txt.sig`: an ECDSA P-256 signature over SHA-256 of `release.txt` (DER):

  $$\text{release.txt.sig} = \mathrm{ECDSA\text{-}P256}_{\text{release key}}\big(\mathrm{SHA256}(\texttt{release.txt})\big)$$

The private release key is a secret of the GitHub environment `release` (named `RELEASE_SIGNING_KEY`). GitHub hands that secret only to a release run started by a `v*` tag, and only after the repository owner approved that run on its page ("Review deployments", then Approve). No copy is kept anywhere else: `scripts/release-key.sh` makes the key in a temporary folder, stores it in GitHub and deletes the local copy. The public key is pinned in a `# BEGIN RELEASE KEYS` block in the secure installer (`crates/phygital-key/assets/install-service.sh`) and in `install.sh`, and a test checks that the two blocks match. `phygital-key update` reads the same block, built into the program.

```mermaid
flowchart LR
    Tag["Owner pushes tag vX.Y.Z"] --> Build["build job: tests, 5 archives, a build record for each"]
    Build --> Wait["publish job waits for the owner's approval"]
    Wait --> Sign["After approval: computes every SHA-256, writes and signs release.txt, checks it against the pinned key"]
    Sign --> Pub["Published release with release.txt and release.txt.sig"]
    Pub --> Check["Installer, update job or phygital-key update: checks the signature with the pinned keys, then the SHA-256"]
```

1. A pushed tag `vX.Y.Z` starts `.github/workflows/release.yml`. Its build job runs `cargo test --workspace --locked` and builds the archives (and their `.sha256` files) for the 5 targets, and publishes a build provenance attestation for each archive (`actions/attest-build-provenance`: a record signed through Sigstore of which workflow built which file from which commit).
2. The `publish` job uses the environment `release`, so it waits until the owner presses "Review deployments" and approves. Only then does it get the key. It computes the SHA-256 of every archive itself, writes `release.txt`, signs it, checks the signature against the keys pinned in that commit's `install-service.sh` (it stops if none matches), and publishes the release with every file plus `release.txt` and `release.txt.sig`.
3. Every installer and updater checks the signature first and takes the SHA-256 only from the signed list.

The parser in `crates/phygital-key/src/update.rs` refuses anything that is not exactly this form: another header, a version line without `v` or with leading zeros, uppercase hex, one space instead of two, CRLF line ends, a file name for another version, a path, a duplicate file, or a list with no files.

What this protects: someone who swaps a release file on GitHub, runs a download mirror or sits between you and GitHub cannot make an update that installs, because they cannot sign. A stolen workflow token or a pull request cannot read the key either: GitHub gives the environment secret only to approved runs from `v*` tags.

What it does not protect: whoever controls the owner's GitHub account (or is a repository admin) can push a tag, approve the run and get a signed release, and the secure install with automatic updates installs it within about a day. What still limits that: the owner's GitHub sign-in (two-factor, ideally a passkey), the build record that shows afterwards which workflow built each file from which commit, the version check (no older version installs), and your choice to turn automatic updates off. Your first install also trusts the installer script you download (pinned to the `v0.3.0` tag over https), and the keys in it are the root of trust from then on. See [the update channel](threat-model.md#the-update-channel).

## After updating

- **stdio (`phygital-key mcp`)**: the AI app keeps running the old program until it starts the gate again. Restart the app.
- **`phygital-key serve` or `demo`**: stop it and start it again.
- **Secure install**: the update restarts the service for you.

> [!WARNING]
> A restart drops requests that are still waiting and ends every standing approval (their signing keys live only in memory). The daily job waits while standing approvals are active, but for 7 days at most: after that it installs anyway, and you ask for standing approvals again. Turn automatic updates off if even that would hurt you.

## What carries over

Updates never touch the data folder. Your keyring, audit log, MCP token, revocations and pinned service keys stay, so AI apps keep working with the same token and services keep trusting the same keys.

Keyrings made by v0.1.0, before the signed key history existed, keep working and stay unsigned; the home page then says the history does not check out. Start a new keyring if you want a signed one ([keys and devices](keys-and-devices.md#if-you-lose-a-device) lists the steps).

Services that check approvals use their own copy of the verifier (the Rust crate, the JavaScript checker or `phygital-key verify`). Update those separately. The approval format is versioned (`pka.1`): the [spec](spec.md) and its test vectors say what every verifier must accept and refuse.

## Check a download yourself

| Machine | Archive |
|---|---|
| macOS, Apple silicon | `phygital-key-vX.Y.Z-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `phygital-key-vX.Y.Z-x86_64-apple-darwin.tar.gz` |
| Linux, x86_64 | `phygital-key-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
| Linux, arm64 | `phygital-key-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` |
| Windows, x86_64 | `phygital-key-vX.Y.Z-x86_64-pc-windows-msvc.zip` |

Each archive holds one folder with the program, `README.md`, `LICENSE` and `NOTICE`.

**Check the signed list (who published it).** Download `release.txt` and `release.txt.sig` from the release, take the pinned public keys from the installer of the same tag, and check the signature:

```bash
curl -fsSL https://raw.githubusercontent.com/Phygital-Key/phygital-key-mcp/v0.3.0/crates/phygital-key/assets/install-service.sh \
  | awk '/-----BEGIN PUBLIC KEY-----/{n++; f="release-key-" n ".pem"} f{print > f} /-----END PUBLIC KEY-----/{close(f); f=""}'
openssl dgst -sha256 -verify release-key-1.pem -signature release.txt.sig release.txt
```

It prints `Verified OK` for a key that signed the list (try `release-key-2.pem` if there are two keys). Then compare the archive with the signed list:

```bash
tail -n +3 release.txt | shasum -a 256 -c --ignore-missing     # macOS
tail -n +3 release.txt | sha256sum -c --ignore-missing         # Linux
```

Both print the archive's name and `OK` when it matches.

**Check only the checksum file.** Every archive also has `<file>.sha256` next to it:

```bash
shasum -a 256 -c phygital-key-v0.3.0-aarch64-apple-darwin.tar.gz.sha256      # macOS
sha256sum -c phygital-key-v0.3.0-x86_64-unknown-linux-gnu.tar.gz.sha256      # Linux
```

On Windows (PowerShell), compare the hash it prints with the first word of the `.sha256` file:

```powershell
(Get-FileHash .\phygital-key-v0.3.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash.ToLower()
```

A matching `.sha256` shows only that the file is the one published next to it on GitHub. The signed `release.txt` also shows that a release run the owner approved signed it.

**Check the build record (who built it).** With the GitHub CLI (`gh`), check that the release workflow built the archive, and from which commit:

```bash
gh attestation verify phygital-key-v0.3.0-aarch64-apple-darwin.tar.gz --repo Phygital-Key/phygital-key-mcp
```

## Limits

- **Not tested against GitHub yet.** Signed release lists start with v0.3.0, so the first real automatic update (download, check, swap and restart against a newer signed release on GitHub) happens with the release after v0.3.0. CI tests the parts on their own, on macOS and Linux, on every push to `main` and every pull request (`tests/secure-install.sh`): the update job is installed and root-owned, it is on by default, a release list signed by an unknown key is refused, the job leaves the gate running unless a newer signed release exists and records its result, a new program that does not start is switched back to the previous one, `--no-auto-update` removes the job, the off choice is kept on later installs, `--auto-update` turns it back on, uninstall removes the job, and root ignores a planted `~/.curlrc` and a fake `curl` early in the user's `PATH`. The unit tests in `update.rs` cover the signature check, the strict format, version order, reading the pinned keys, and that both installers pin the same keys.
- **The owner's GitHub account can ship an update.** The release key lives in GitHub, so whoever controls the owner's account can approve a release run and get a signed update ([what this protects](#the-signed-release-list)). Turn automatic updates off if you want to install each release yourself.
- **One trust root.** Everything rests on the pinned release keys, the owner's GitHub account, and the first installer you ran.
- **A moment with a free port.** During an update restart the gate's port is free for a moment. A program that takes it is stopped and the gate started again, but a fast program could win that moment again (and is stopped again). Handing the open port from launchd or systemd to the gate, so it is never free, is not built: the gate forbids unsafe code.
- **No expiry on the signed list.** Someone who controls the GitHub account or a download mirror could keep serving an old, validly signed `release.txt`. The job then reports `up to date` and never sees the newer release (a freeze). Compare the version in `status` with the [Releases](https://github.com/Phygital-Key/phygital-key-mcp/releases) page now and then.
- **Daily network use.** With automatic updates on, the secure install contacts `github.com` once a day ([privacy](privacy.md#what-leaves-your-machine)). The gate itself still makes no network calls.
- **Restarts.** An installed update restarts the gate at the job's time, with the effects [above](#what-automatic-updates-need).

## Next

- [Secure install](secure-install.md#update)
- [Troubleshooting](troubleshooting.md#automatic-updates)
- [Privacy](privacy.md)
- [Changelog](../CHANGELOG.md)
