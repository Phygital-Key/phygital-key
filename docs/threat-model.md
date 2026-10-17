# Threat model (v0.2, pre-audit)

Plain words first: Phygital Key makes sure **a real person said yes to this exact request**, and that **the yes cannot be faked, changed or reused**. It does not make a bad request safe, and it cannot protect a service that does not check the approval.

## What we protect

1. **The yes itself.** Only the person's key can make it. It covers one exact request, for one audience, for up to 5 minutes, once.
2. **The approver's keys.** They never leave the person's device. The gate only ever sees public keys.
3. **The record.** Every request, approval, denial and new key is in a hash-chained audit log.

## Who we defend against

| Attacker | Example | Defense |
|---|---|---|
| A misled or hostile AI agent | Prompt injection makes it want to deploy, pay or delete | It can only ask. It never holds a key. Text inside a request is shown as data and never counts as a yes. |
| An agent that changes the request after approval | Approved 2.4.1, sends 2.4.2, adds `force: true`, swaps the action type | The verifier hashes the request it is about to run. Any change is `REQUEST_MISMATCH`. |
| An agent that reuses or stretches an approval | Replays it, uses it later, uses it at another service | Single-use `id` (`REPLAYED`), short `exp` (`EXPIRED`), `aud` (`WRONG_AUDIENCE`). |
| An agent that asks for too little | Requests `medium` for a production deploy | The **verifier** sets the level it needs (`ASSURANCE_TOO_LOW`). |
| An agent that forges | Sends a made-up signature to the gate or the service | Signatures are checked against trusted public keys (`BAD_SIGNATURE`, `UNTRUSTED_KEY`). The gate checks before it stores anything. |
| An agent that adds its own key | Calls the enrollment API with a software key | Enrollment needs a one-time token from `phygital-key enroll` in the person's own terminal; `mcp` mode never prints one (its output reaches the AI app's logs). After the first key, every new key must be approved (`high`) by an existing key. Attestation is `none`, so the gate cannot tell hardware from software keys: whoever holds a valid link can enroll the first key. |
| A phishing page | A copy of the approval page on another domain | WebAuthn binds keys to the real domain (RP ID). Wrong origin or RP ID is `BAD_SIGNATURE`. |
| A web page in the person's browser | CSRF, DNS rebinding, framing the page, reading local key names | Origin check on every page POST, `/mcp` refuses any browser Origin, Host check on every request, `frame-ancestors 'none'`, cross-origin assertions refused, no CORS on localhost. |
| Another program on the same machine | Calls the page API directly with a fake Origin | It can deny or cancel a waiting request (a nuisance, the answer becomes no). It cannot approve, and cannot swap the claims while the person signs. |
| Look-alike text | `аpi` with a Cyrillic a, KELVIN SIGN instead of K, invisible characters | The exact bytes are signed and compared, never rewritten; text that is not NFC is refused; audiences are visible ASCII only; the page quotes odd keys and highlights common invisible and mixed-script characters (a help, not a guarantee). |
| Approval fatigue | Hundreds of requests so the person clicks yes | 30 requests per minute, 50 waiting at most, every field always shown, fingerprint or PIN for `high`. |
| A tampered key list | A website or file lists an extra key, relabels one, serves an old copy, or a removed key signs a new branch | Every key change is signed by a key trusted at that point, back to one pinned fingerprint and a pinned origin; cards must match what was signed; whoever refetches keeps the last accepted copy and takes only extensions (`--previous`). A first fetch with no stored copy cannot tell an old copy from the latest. |
| Another program on this machine using the gate's MCP | Creating requests to flood the person | Over HTTP, `/mcp` needs the gate's access token; stdio is private to the app that started it. Requests are also rate-limited. |
| A revoked standing approval still used at a service | Uses collected before the revoke | The person signs a revocation; services that pick up `/.well-known/phygital-key-revoked.json` refuse it with `REVOKED`. Until they do, tokens already handed out work for up to 60 s. |
| A made-up receipt | The agent reports "done" with a fake receipt | The gate records a receipt only if it is signed by the service's pinned receipt key. |
| An agent stretching a standing approval | More uses, bigger amounts, another payee, extra fields, after revoke | Exact `fixed` values, `vary` limits, nothing extra, use counting by the service, 60-second use tokens, use key kept only in the gate's memory and thrown away on revoke. Tokens already handed out work up to 60 s after a revoke, and that can be every remaining use if the agent collected them first. Services must count uses in a durable store shared by all their instances. |
| Parser tricks | Duplicate keys, floats, huge or deeply nested JSON, lone surrogates, base64 padding | Strict parsing refuses all of these (`MALFORMED`), before any signature work. |

## What we trust

- **The person's device and authenticator.** If the phone or laptop is fully compromised, an attacker can trigger signatures.
- **The machine that runs the gate.** The approval page is served by the gate. Whoever controls that machine controls what the page shows, and could show one request while asking the key to sign another. Authenticators do not display what they sign. This is the same role as AP2's Trusted Surface.
- **The verifier's keyring.** Whoever can write the keyring decides which keys are trusted.

## Where to run it (important)

The safety holds only if **the agent cannot change the gate's code, the gate's data folder or the verifier's keyring**.

- **Good:** `sudo phygital-key install-service` (macOS, Linux). The gate runs as its own system user; its program is owned by root and its data folder is readable only by that user; enrolling a key or reading the MCP token needs your administrator password (`phygital-key-admin`). An agent acting as you can then use the gate only through MCP. This holds as long as the agent cannot use `sudo` without your password (check that your account has no password-less sudo).
- **Also good:** the gate on another machine, the verifier with its own read-only copy of the keyring.
- **Demo and quick tries:** everything under your own user (`phygital-key demo`, `phygital-key mcp`). This shows the flow, but an agent with full shell access as you could edit the files.

## Known limits in v0.1

- **Signed, not encrypted.** Whoever holds an approval can read the request. Keep secrets out of requests. Encrypted requests are planned (spec section 9).
- **Bearer for up to 60 s.** A stolen approval works once, only for that request and audience, until it expires. Binding to the requester's key is planned (v0.2).
- **The audit log catches edits, not a full rewrite.** The hash chain has no secret key: someone who can write the file can rebuild every hash, or cut off the end. It shows tampering only if you keep the last hash somewhere else (a backup, a ticket).
- **The command-line verifier remembers used ids in a local file** (`seen-ids.jsonl`): locked while checked, rewritten through a temp file and rename, and any damaged line refuses everything. Several machines need a shared store (use the library).
- **Standing approvals live in the gate's memory.** A restart ends them all (fail closed). The service counts uses itself; a service with several instances needs a shared use store.
- **Pending requests live in memory.** A restart drops them (the answer becomes no).
- **No outside review yet.** Treat it as pre-audit.

## Reporting

See [SECURITY.md](../SECURITY.md).
