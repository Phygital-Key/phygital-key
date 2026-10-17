# Questions and answers

**Do I need special hardware?**
No. A phone with a passkey, Touch ID, Windows Hello or any FIDO2 security key works. A hardware key is the strongest choice.

**Does it work on Windows?**
The gate, the checker and the demo run on Windows, macOS and Linux. The secure install (separate system user) is macOS and Linux for now.

**Can the AI approve on its own?**
No. Only your key can sign, and your key never leaves your device. The AI can ask, wait, cancel and report, nothing else.

**What if the AI changes the request after I approve?**
The service checks the request it is about to run against the one you signed, to the last character. Any change is refused.

**What if the AI runs on my computer and edits the gate's files?**
Use the secure install. The gate then runs as its own locked-down user: the AI, even acting as you, can't change its program, read its data or write its keys.

**What if someone copies the approval page?**
Passkeys only sign for the real address of your gate. A copy on another address gets nothing usable. On your own computer, the gate also holds both localhost addresses, so nothing else can pretend to be it there.

**Why 60 seconds?**
An approval is meant for right now. A short life means a stolen one is almost useless, and it still works only once, for one service.

**I approve the same small thing every day. Do I have to touch my key each time?**
No: give a standing permission with limits (what, to whom, how much, how often, until when). You can revoke it any time.

**What if I lose my phone?**
Add two keys from the start (phone and laptop). Remove the lost one with the other. If every key is lost, start a new key list; services then need your new fingerprint.

**Is my request private?**
It is signed, not encrypted. The gate and the service see it, and anyone holding the signed note can read it. Keep secrets (passwords, card numbers) out of requests. See the [decision on encryption](decisions.md).

**Does it send anything to the internet?**
No. No analytics, no cloud, no account. The secure install downloads the official release from GitHub once, to check it.

**Does it work with AP2 (agent payments)?**
It follows the same ideas (closed and open mandates, a Trusted Surface, receipts). How it maps, and what an AP2 export would cost, is in [ap2.md](ap2.md).

**Has it been audited?**
Not by an outside firm yet. It has had several independent security reviews, every finding was fixed, and it is tested on every change (shared test cases in two independent checkers, fuzzing, mutation testing, end-to-end attacks, and a real secure install on macOS and Linux). Treat it as pre-audit.

**Can I use it commercially?**
Yes. It is Apache-2.0. Keep the `NOTICE` file (it credits s17sh70x2jfl031ag) with any copy or modified version.
