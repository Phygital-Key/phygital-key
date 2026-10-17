# phygital-key-verify (JavaScript)

An independent JavaScript checker for [Phygital Key](https://github.com/s17sh70x2jfl031ag/phygital-key-mcp) approvals: a person's signed yes, made with a passkey, fingerprint, face or hardware key, for one exact request.

- Written from the [spec](../docs/spec.md), sharing no code with the Rust verifier.
- Passes every shared test vector (`test-vectors/`), exactly like the Rust one. Two implementations agreeing is how unclear spots in the spec get caught.
- No dependencies. Node 20+ and modern browsers (WebCrypto).
- Pure: no network, no clock (pass `now`), fails closed.

```js
import { verify, MemorySeenStore, keyringFrom } from "phygital-key-verify";

const keyring = keyringFrom(JSON.parse(keyringJson));          // from `phygital-key keys`
const config = { origins: ["https://approve.example.com"], rp_ids: ["approve.example.com"] };
const seen = new MemorySeenStore();                              // use a shared, durable store in production

const request = { type: "deploy", params: { service: "api", env: "production", version: "2.4.1" } };
const result = await verify(approval, { aud: "https://deploy.example.com", request, level: "high" },
                            keyring, seen, config, Math.floor(Date.now() / 1000));
if (!result.ok) throw new Error(result.error);                   // e.g. "REQUEST_MISMATCH"
// ...now deploy.
```

Also included: `verifyUse` and `revocationsFrom` (standing approvals), `verifyHistory` and `verifyHistoryUpdate` (signed key lists).

Run the tests: `npm test`. Pre-audit, like the rest of the project.
