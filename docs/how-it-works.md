# How it works, in plain words

## The idea

An AI agent (or an app, or a business) wants to do something that matters: deploy code, pay, delete, send. Before it may, **a person says yes to that exact thing, with their own key**: a passkey on their phone or laptop, Touch ID, Windows Hello, or a hardware key like a YubiKey.

The yes is a small signed note. The service that does the work checks the note itself, before acting. If anything about it is off, the answer is no.

## The four parts

| Part | Who | Job |
|---|---|---|
| **Requester** | The AI agent (or any app) | Asks for a yes, for one exact request |
| **Gate** | `phygital-key`, running on your machine or a server | Shows the request to the person, collects their signature, hands back the signed note |
| **Approver** | You, with your key | Reads the request on the approval page, then touches your key, or says no |
| **Verifier** | The service that does the work | Checks the signed note before acting |

The gate is a normal MCP server, so any AI app can talk to it. The verifier is a small library (Rust or JavaScript) or one command.

## One approval, step by step

1. The agent calls `request_approval` with the exact request, for example `{"type": "deploy", "params": {"service": "api", "env": "production", "version": "2.4.1"}}`, plus who will carry it out (the service) and how strong the proof must be.
2. The gate turns the request into one exact text (the "canonical form") and opens the approval page for you.
3. You see every field. You press **Approve with my key** and touch your key (or use your fingerprint, face or PIN). Or you press **Deny**.
4. Your device signs a fingerprint of the exact text, the service's name, the time and the level. Your key never leaves your device.
5. The agent gets the signed note and sends it with the action to the service.
6. The service checks it: same request (to the last character), meant for this service, still in time (60 seconds), signed by a key it trusts, not used before. Only then does it act.

## Why it can't be faked

- **Only your key can sign.** The AI never has it, and neither does the gate.
- **The signature covers the exact text.** Change `2.4.1` to `2.4.2`, add a field, or swap the action, and the check fails.
- **One service, one use, one minute.** The note names the service, has a single-use id, and runs out fast.
- **Look-alike text is refused.** Text that only looks the same (a Cyrillic "а", a special "K" sign) is a different text, and text in odd encodings is refused outright.
- **Fake pages don't work.** Passkeys only sign for the real address of your gate.
- **The AI can't touch the gate.** With the secure install, the gate runs as its own locked-down user: the AI, even acting as you, can't change its program, read its data or write its keys.

## Standing approvals (approve once, with limits)

For small repeated actions you can approve a **standing permission** once:

> Allow **payment** to **ACME Ltd** in **EUR**, from **1** to **2000** cents each, **at most 5 times**, **for 7 days**.

Each use then gets a one-minute token from the gate, and the service checks both your approval and the token, and counts uses itself. Anything outside the limits is refused. You can revoke it any time: new uses stop at once, and one more touch signs a revocation the service will see.

## Your keys

- **Adding your first key** uses a one-time link that only your terminal prints. Your first key signs itself.
- **Adding or removing more keys** needs a yes from a key you already trust.
- **Your key list is signed.** Share one fingerprint (on the gate's home page) and anyone can check your whole key list, and spot an old copy.

## Receipts

After acting, a service can send back a signed receipt ("done" or "failed"). The gate shows it on your approval page only if it is signed by that service's pinned key, so the AI can't fake one.

## What it does not do

- It does not make a bad request safe. It makes sure a person agreed to exactly that request.
- It cannot protect a service that does not check the note.
- It is not encrypted: whoever holds a note can read the request. Keep secrets out of requests.
- It has not had an outside audit yet.

More detail: [spec](spec.md), [threat model](threat-model.md), [decisions](decisions.md).
