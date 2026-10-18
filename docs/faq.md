# Questions and answers

Short answers to the questions people ask most, for people who approve, developers who check approvals, and anyone comparing options. Each answer links to the page with the details.

**On this page**

- [Basics](#basics)
- [Safety and the AI](#safety-and-the-ai)
- [Verification and authority](#verification-and-authority)
- [Encryption and privacy](#encryption-and-privacy)
- [Time limits](#time-limits)
- [Standing approvals](#standing-approvals)
- [Devices, offline use and outages](#devices-offline-use-and-outages)
- [People and companies](#people-and-companies)
- [Comparisons](#comparisons)
- [Audits, licence and cost](#audits-licence-and-cost)

## Basics

**What is it, in one sentence?**
A person's signed yes, for one exact request, that anyone can check. Before an AI agent deploys, pays, deletes or sends, you approve that exact action with a passkey, fingerprint or hardware key, and the service checks your signature itself, offline.

**Do I need special hardware?**
No. A phone with a passkey, Touch ID, Windows Hello or any FIDO2 security key works. A hardware key is the strongest choice.

**Does it work on Windows?**
The gate, the checker and the demo run on Windows, macOS and Linux. The secure install (separate system user) is macOS and Linux for now.

**How does it stay up to date?**
The secure install updates itself: a daily job (a root LaunchDaemon on macOS, a systemd timer on Linux) fetches the release list, checks its signature against the release keys pinned in the installer, and installs only a strictly newer signed version. It keeps the old program and switches back to it if the new gate does not start. It is on by default; `--no-auto-update` turns it off, and `phygital-key-admin update` updates now. Other installs update when you run them: `phygital-key update` (install script or `cargo install`), `brew upgrade phygital-key`, or `scoop update phygital-key` on Windows. An update restarts the gate, which drops requests still waiting and ends every standing approval it holds. The daily job waits while standing approvals are active (7 days at most, counted from the first wait on the installed version); waiting requests do not hold it back, and the agent asks again after the restart. Approvals already signed still check. The full update path runs for real only once a signed release newer than v0.3.0 exists. See [updates](updates.md).

**Who can sign a release?**
Only a release run on GitHub that the repository owner approved. The [release key](glossary.md#release-key) is a secret of the GitHub environment `release`, and GitHub hands it only to a run from a `v*` tag after the owner presses "Review deployments" and approves. A swapped file, a mirror or a stolen workflow token cannot sign. Someone who takes over the owner's GitHub account could, and the secure install would install that release within about a day. If you would rather check each release first, install with `--no-auto-update` and update by hand. See [the update channel](threat-model.md#the-update-channel).

**Does it work with any AI app?**
With any app that can use an MCP server: the gate is one, over stdio (what most AI apps launch) or over HTTP with an access token. Other ways in (an HTTP API, email links, QR codes) are not built. See [connect your AI](connect-your-ai.md).

**Does my service need MCP?**
No. MCP is only how the agent reaches the gate. Your service receives a plain string in a `Phygital-Approval` header and checks it with the Rust library, the JavaScript checker or the `phygital-key verify` command. See [integrate](integrate.md).

## Safety and the AI

**Can the AI approve on its own?**
No. Only your key can sign, and your key never leaves your device. The AI can ask, wait, cancel and report, nothing else. None of the six MCP tools can approve, and the gate has no signing key for approvals.

**What if the AI changes the request after I approve?**
The service checks the request it is about to run against the one you signed, to the last character. Any change is refused with `REQUEST_MISMATCH`.

**What if the AI writes "approved" into the request?**
Nothing changes. Text inside a request is data: it is shown to you on the page like any other field, and only your key's signature counts. The end-to-end tests check exactly this ("prompt injection in params changes nothing").

**What if the AI runs on my computer and edits the gate's files?**
Use the [secure install](secure-install.md). The gate then runs as its own locked-down user: the AI, even acting as you, can't change its program, read its data or write its keys. Only the administrator password can. It needs two things from you: install from a terminal you trust, and keep `sudo` behind your password.

**What if someone copies the approval page?**
Passkeys only sign for the real address of your gate. A copy on another address gets nothing usable. On your own computer, the gate also holds both localhost addresses (IPv4 and IPv6), so nothing else can pretend to be it there.

**Is there an AI inside the gate or the page?**
No. The gate and the approval page are plain code with no language model. The page shows the request as the gate built it from the canonical form, never text the agent wrote about it.

## Verification and authority

**How does a service know an approval is real?**
It runs these checks, in order, and refuses at the first failure:

| Step | Check | Code on failure |
|---|---|---|
| 1 | Present and well formed: size, strict JSON, no unknown fields, version `pka.1`, one signature | `APPROVAL_REQUIRED`, `MALFORMED` |
| 2 | Made for this service (`aud`) | `WRONG_AUDIENCE` |
| 3 | In time: issued at most 30 s ahead, not past its end, life at most 300 s | `EXPIRED` |
| 4 | Exactly the request the service is about to run | `REQUEST_MISMATCH` |
| 5 | At least the level this service requires | `ASSURANCE_TOO_LOW` |
| 6 | Signed by a key in the service's keyring, for an allowed relying party | `UNTRUSTED_KEY` |
| 7 | The WebAuthn signature: type, challenge, origin, not cross-origin, relying party hash, ES256 over the exact data | `BAD_SIGNATURE` |
| 8 | User presence, and user verification for `high` | `ASSURANCE_TOO_LOW` |
| 9 | First use | `REPLAYED` |

Two checkers that share no code (Rust and JavaScript) pass the same shared test vectors. See [how it works](how-it-works.md) and [error codes](error-codes.md).

**Who decides what?**

| Who | Decides | Can never |
|---|---|---|
| You (the approver) | Yes or no, on the exact request shown | Be skipped: no key, no approval |
| The agent (requester) | What to ask for, and which level to ask | Approve, sign, or change a request after you sign |
| The gate | Shows the request, collects your signature, checks it before keeping it | Sign an approval on its own |
| The service (verifier) | Its own audience, the exact request it will run, the level for each action | Trust what the agent says about the approval |
| The key history | Which keys count: each change is signed by a key already trusted, starting from one pinned fingerprint | Add a key without that signature, or bring back a removed key |

The agent picks a level when it asks, but the service decides what it accepts, so an agent cannot ask for `medium` on a production deploy and get through ([decision 7](decisions.md)).

**How does a service get my keys, and how does it know they are mine?**
From `phygital-key keys`, or from your gate's `/.well-known/phygital-key.json`. The keyring carries a signed history of every key change. The service checks it against your first key's fingerprint, learned once (in person, on a call), and keeps the copy it accepted so an older copy cannot be passed off later. See [key history](key-history.md).

**Can a service accept only hardware keys (like YubiKeys)?**
Yes, in a simple way: a service trusts only the keys in its keyring. If your company hands out hardware keys and adds only those to the keyring, nothing else can approve. Automatic proof that a key is certified hardware (checking its maker's certificate) is not built ([decision 35](decisions.md)): phone and laptop passkeys never provide it, and the extra code would add risk for few users.

**Can I see who approved?**
Yes. A successful check in the Rust or JavaScript checker returns the approval id, the request hash, the key's credential id and the name on its key card (`approver`); `phygital-key verify` prints the id, the level, the end time and the name (`approved_by`). Services should log them. The gate keeps its own hash-chained [audit log](audit-log.md).

**Is there a record of what happened?**
The gate writes every request, approval, denial, key change, receipt and revocation to an append-only, hash-chained audit log, and `phygital-key audit-check` checks the chain. It stores the request hash, never the request itself. It detects edits to past lines, but not a full rewrite or a cut-off end, so keep a copy of the last hash somewhere else. Services can also return signed [receipts](receipts.md), so you see "the service reported: done".

## Encryption and privacy

**Is my request encrypted?**
No. Approvals are signed and verified, not secret. The gate and the service see the request, and anyone holding the approval can read it. Keep secrets (passwords, card numbers, connection strings) out of requests.

**Then what does the cryptography protect?**
That the yes is real and unchanged. Your key signs with ES256 (ECDSA over P-256 with SHA-256); the request is hashed with SHA-256 over its RFC 8785 canonical form; the signature covers the exact request, the audience, the level, the times and a one-time id. Anyone can check it, and nobody can change a character without breaking it. Your private key stays in your phone, laptop chip or security key.

**Why not encrypt requests end to end?**
The approval page is served by the gate, so encrypting a request against the gate only helps if the page comes from somewhere the gate cannot change, such as a separate, pinned page or an app. On your own computer, the AI is already on that machine. If a hosted gate is ever built, encryption comes with it ([decision 33](decisions.md)).

**What is protected in transit and at rest?**
In transit: HTTPS for any address other than localhost (passkeys refuse to work otherwise). At rest: the gate's data folder is readable only by the gate (mode 0700; only the gate's own user with the secure install); pending requests live only in memory; the MCP access token is a file with mode 0600. The audit log never stores the request itself. See [privacy](privacy.md).

**Does it send anything to the internet?**
The gate itself makes no network calls: no analytics, no cloud, no account, no external fonts or scripts. The secure install contacts github.com once a day to fetch the signed release list (and the release file when it updates); `--no-auto-update` stops that. The install script and `phygital-key update` contact GitHub only when you run them. Every download is checked against the signed release list and its SHA-256. If your passkeys sync (for example through your phone's password manager), that syncing is done by your platform, not by Phygital Key.

## Time limits

**Why 60 seconds?**
An approval is meant for right now. A short life means a stolen one is almost useless, and it still works only once, for one service. The 60 seconds start when you press Approve, not when the agent asks: you have 10 minutes to read and decide. A gate can set up to 300 seconds with `--lifetime`.

**What if the clocks are not in sync?**
A service accepts an approval issued up to 30 seconds in its future, so a small drift is fine. If the service's clock runs more than about a minute ahead of the gate's, every approval looks expired (`EXPIRED`). Used ids are kept 30 seconds past their end, so a clock that steps back cannot replay one. Keep clocks set by network time.

**I approved, but the service says `EXPIRED`. Why?**
The agent waited too long to use it, or the clocks differ. Ask again. See [troubleshooting](troubleshooting.md).

## Standing approvals

**I approve the same small thing every day. Do I have to touch my key each time?**
No. Give a standing approval with limits: which action, exact fixed values (to whom, which currency), integer ranges with both a minimum and a maximum, or a short list of allowed strings, a number of uses (1 to 1000) and an end date (at most 30 days). It is always level `high`. See [standing approvals](standing-approvals.md).

**How does a service check a standing approval?**
Each use arrives with your standing approval and a use token the gate signs for that one request (valid at most 60 seconds). The service checks your signature on the limits, the token, that the request is exactly the one in the token and inside the limits (`NOT_ALLOWED`), and counts uses itself (`USED_UP`).

**How do I stop one?**
Press **Revoke now** on its page, or ask the AI to cancel it. The gate stops new uses at once. Then you sign a revocation (one more touch), published at `/.well-known/phygital-key-revoked.json`, and services that fetch it refuse the standing approval (`REVOKED`). Until a service fetches the list, use tokens already handed out work for up to 60 seconds. See [revocations](revocations.md).

## Devices, offline use and outages

**Can I approve on my phone?**
Yes. If the gate runs on your computer, the browser offers a QR code to approve with your phone (this is built into passkeys). If the gate runs on your own server behind HTTPS, you can open the approval page on the phone directly and approve with a passkey enrolled on the phone. See [own server](own-server.md) and [keys and devices](keys-and-devices.md).

**What if I lose my phone?**
Add two keys from the start (phone and laptop). Remove the lost one with the other. If every key is lost, start a new keyring; services then need your new fingerprint.

**Does checking work offline?**
Yes. The check is pure: no network, no call home. A service needs your keyring file, its own config and a store of used ids. A door controller or a machine with no internet can check approvals, as long as its clock is right. Approving needs your device to reach the gate.

**What if the gate is down?**
Nothing can be approved: there are no approvals without the gate and your key, so the system fails closed. Approvals already handed out still check at the service until they expire, because the service needs nothing from the gate. Pending requests live only in memory, so a restart (or an update) drops them, and it ends every standing approval the gate holds.

## People and companies

**Can several people approve for one service?**
Yes, if their keys are in the keyring the service trusts. Any one trusted key can approve, and the result names which key signed, so a service can require a specific person for a specific action. An approval that needs 2 of 3 people is not built: the format has a list of signatures, but version 0.3.0 requires exactly one.

**Does it work between companies?**
Yes. A company publishes its keyring at its gate's `/.well-known/phygital-key.json`; the other side checks it against a fingerprint learned once and keeps the copy it accepted. Today requests reach the gate through MCP only, so an outside company's agent needs an access token, or your own agent relays the request. See [use cases](use-cases.md#7-company-to-company-a-suppliers-ai-asks-a-buyer-to-approve-an-order).

**Does it work with AP2 (agent payments)?**
It follows the same ideas: closed and open mandates (one-time and standing approvals), a Trusted Surface (the approval page, with no AI), and receipts that point at the approval by hash. The difference: your own passkey signs, where AP2 uses a wallet or provider key. There is no AP2 export yet ([decision 28](decisions.md)); the mapping and what an export would cost are in [ap2.md](ap2.md).

**Can it be used with A2A (Agent2Agent)?**
The approval is a signed string, so it can travel in any message, and the checkers work in any agent that receives one. An A2A adapter that carries approvals inside A2A messages is planned for later and is not built ([roadmap](roadmap.md)).

## Comparisons

**Many AI apps already ask "Allow?" before a tool runs. Why add this?**
An in-app prompt is not signed. The service on the other end receives the action, not the click, so it cannot check that a person said yes, or to what. A Phygital Key approval is a signature the service checks itself: for this exact request, this service, this level, once. The two work together: the app's prompt decides whether the tool runs; the approval decides whether the service acts.

**How is it different from an API key with limits?**
An API key says which program may call. It says nothing about which person agreed to which action. With Phygital Key, the agent can hold the service's ordinary credentials and still cannot perform a checked action without your signed yes for that exact request.

**How is it different from signing in with a passkey?**
Signing in proves who you are for a session. An approval signs one exact request, so the proof travels with the action and the service can check it without any session.

## Audits, licence and cost

**Has it been audited?**
Not by an outside firm yet. Treat it as pre-audit. It has had seven independent internal security reviews (AI security auditor agents), with every finding fixed or documented as a limit, and it is tested on every change: 123 shared test vectors (55 approvals, 25 key histories, 43 standing approvals) in two independent checkers, property tests, fuzzing, mutation testing, 108 end-to-end checks with a real browser and a virtual passkey, and a real secure install on macOS and Linux. See [security reviews](security-reviews.md) and [testing](testing.md).

**What does it cost?**
Nothing. It is free and open source, with no account, no subscription and no paid tier. If you run the gate on your own server, you pay for that server.

**Can I use it commercially?**
Yes. Everything (spec, test vectors, both checkers, the gate) is Apache-2.0. You may use it, change it, sell it and build on it.

**What do I have to keep?**
The `NOTICE` file, with any copy or changed version. It carries the line "Created by Phygital-Key", and Apache-2.0 section 4(d) makes it travel with every copy. See [CONTRIBUTING.md](../CONTRIBUTING.md) to help.

## Next

- [Use cases](use-cases.md)
- [Threat model](threat-model.md)
- [Glossary](glossary.md)
- [Roadmap](roadmap.md)
