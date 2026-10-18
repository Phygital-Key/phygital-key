# phygital-key-verify (JavaScript)

An independent JavaScript checker for [Phygital Key](../README.md) approvals: a person's signed yes, made with a passkey, fingerprint, face or hardware key, for one exact request. A service calls it before it acts, and acts only when `result.ok` is `true`.

- Written from the [spec](../docs/spec.md). It shares no code with the [Rust verifier](../crates/phygital-key-verify/README.md); both pass the same 141 shared test vectors. Two implementations agreeing is how unclear spots in the spec get caught.
- No dependencies: WebCrypto (`crypto.subtle`) for ES256 and SHA-256, and its own strict JSON parser. Node 20+ and modern browsers.
- Pure: no network, no clock (you pass `now`). Fails closed: `verify` never throws for bad input, it returns `{ ok: false, error }`.

Pre-audit, like the rest of the project.

## Install

Not published to npm yet. Install from a clone:

```bash
git clone https://github.com/Phygital-Key/phygital-key
npm install ./phygital-key/verifier-js
```

## Example

```js
import { verify, MemorySeenStore, keyringFrom } from "phygital-key-verify";

const keyring = keyringFrom(JSON.parse(keyringJson));            // from `phygital-key-mcp keys`
const config = { origins: ["https://approve.example.com"], rp_ids: ["approve.example.com"] };
const seen = new MemorySeenStore();                                // one process only

// The request you are about to run, built from your own data.
const request = { type: "deploy", params: { service: "api", env: "production", version: "2.4.1" } };
const result = await verify(approval, { aud: "https://deploy.example.com", request, level: "high" },
                            keyring, seen, config, Math.floor(Date.now() / 1000));
if (!result.ok) throw new Error(result.error);                     // e.g. "REQUEST_MISMATCH"
// ...now deploy. result.verified = { id, request_hash, credential_id, approver, level, iat, exp }
```

## Stores

`verify` and `verifyUse` await your stores, so plain and `async` methods both work. Each method must still record and answer in one atomic step (a unique-key `INSERT`, Redis `SET NX`), shared by every instance that acts.

- `seen.insertIfNew(id, exp, now)`: only an exact `true` means first use; anything else is `REPLAYED`.
- `uses.recordUse(standingId, useId, maxUses, until, now)`: returns the uses left, a safe integer from 0 to `maxUses - 1` (anything else is `MALFORMED`), or throws `new Refused("REPLAYED")` / `new Refused("USED_UP")`.
- `revoked.has(standingId)`: may be `async`; normally the `Set` from `revocationsFrom`.
- A store that throws or rejects with anything but a `Refused` gives `MALFORMED`: a refusal, never a pass.

Contract and an async database example: [docs/verifier-js.md](../docs/verifier-js.md#stores).

## Also included

| Export | For |
|---|---|
| `verifyUse`, `revocationsFrom`, `MemoryUseStore`, `standingParams`, `allows` | Standing approvals and signed revocations |
| `verifyHistory`, `verifyHistoryUpdate`, `fingerprint`, `keyringAud` | Signed key lists, checked from one pinned fingerprint |
| `checkKind`, `Refused`, `requestShapeOk`, `idOk`, `VERSION` | Lower-level checks and the error class stores throw |
| `canonical`, `hashB64`, `challenge`, `parse`, `b64decode`, `b64encode` | Canonical form, `request_hash`, strict JSON, base64url |

## Tests

```bash
npm test
```

Runs `node --test test/vectors.test.js test/wycheproof.test.js`, 12 tests: every case in `test-vectors/vectors.json`, `keyrings.json` and `standing.json` (141 in all), an approval working only once, three checks from security reviews, two checks that async stores are awaited, a proof made with `signProof` for an agent-bound approval, and every Wycheproof P-256 case. CI runs it on every push.

## Read more

- [JavaScript verifier reference](../docs/verifier-js.md): every export, return shapes, stores, browsers
- [Integrate](../docs/integrate.md)
- [Error codes](../docs/error-codes.md)
