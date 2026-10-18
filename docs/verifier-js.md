# JavaScript verifier

`verifier-js/` is an independent JavaScript checker for Phygital Key approvals. It runs in Node 20+ and in modern browsers, has no dependencies, and passes the same shared test vectors as the Rust verifier while sharing no code with it. This page lists every export with its exact signature and return shape. For where the check fits in a service, read [integrate](integrate.md); terms are in the [glossary](glossary.md).

On this page:

- [Install](#install)
- [Quick start](#quick-start)
- [Values: plain JSON and the strict model](#values-plain-json-and-the-strict-model)
- [Exports](#exports)
- [Standing approvals and revocations](#standing-approvals-and-revocations)
- [Key history](#key-history)
- [Stores](#stores)
- [Node and browsers](#node-and-browsers)
- [How it is tested](#how-it-is-tested)

## Install

The package is not published to npm yet. Install it from a clone of the repository:

```bash
git clone https://github.com/Phygital-Key/phygital-key-mcp
cd your-service
npm install ../phygital-key-mcp/verifier-js
```

npm records `"phygital-key-verify": "file:../phygital-key-mcp/verifier-js"` in your `package.json` and links the folder. Then:

```js
import { verify, MemorySeenStore, keyringFrom } from "phygital-key-verify";
```

You can also import the file directly: `import { verify } from "./phygital-key-mcp/verifier-js/src/index.js"`.

| Fact | Value |
|---|---|
| Package name | `phygital-key-verify` (version 0.3.0, ES module) |
| Entry | `src/index.js` |
| Node | 20 or later (`engines` in `package.json`) |
| Dependencies | none; cryptography is WebCrypto (`crypto.subtle`), JSON parsing is its own strict parser |
| Licence | Apache-2.0 |

## Quick start

```js
import { readFileSync } from "node:fs";
import { verify, MemorySeenStore, keyringFrom } from "phygital-key-verify";

const keyring = keyringFrom(JSON.parse(readFileSync("keyring.json", "utf8"))); // from `phygital-key keys`
const config = { origins: ["https://approve.example.com"], rp_ids: ["approve.example.com"] };
const seen = new MemorySeenStore(); // one process only

// Built from the values you are about to act on, never taken from the approval.
const request = { type: "deploy", params: { service: "api", env: "production", version: "2.4.1" } };
const expected = { aud: "https://deploy.example.com", request, level: "high" };

const result = await verify(approval, expected, keyring, seen, config, Math.floor(Date.now() / 1000));
if (!result.ok) throw new Error(result.error); // for example "REQUEST_MISMATCH"
// ...now deploy, then log result.verified.id and result.verified.request_hash.
```

Run with a fresh test approval (the keys in `test-vectors/keys/` are public test keys), the first call, the same approval again, and a changed version gave:

```text
{ ok: true, verified: { id: 'dHxVTnBboSgn-X8DNsfvHg', request_hash: 'hUXvlpWHC4xarFlhKwaU26H6q05m3gBpMoyE_YKRK7o',
  credential_id: 'vSzUGSD7ABGzgRrk5FijAg', approver: 'alice (TEST ONLY)', level: 'high', iat: 1792273469, exp: 1792273529 } }
{ ok: false, error: 'REPLAYED' }
{ ok: false, error: 'REQUEST_MISMATCH' }
```

## Values: plain JSON and the strict model

The checker parses approvals with its own strict parser (`parse`), which refuses duplicate keys, lone surrogates, nesting deeper than 16 and trailing data. It returns objects as `Map` and keeps numbers that are not safe integers as `NotSafeInteger`, so floats, `-0` and huge integers are refused later instead of being rounded.

| You pass | Form |
|---|---|
| `expected.request` to `verify` and `verifyUse` | Plain JSON (`{ type, params }`) or the `Map` model. Plain values are converted. |
| `keyring`, `config` | Plain objects. |
| `canonical`, `hashB64`, `challenge`, `requestShapeOk`, `allows`, `standingParams` | The `Map` model only. Turn text into it with `parse`. A plain object gives `canonical` an `unsupported value` error and `hashB64` returns `null`. |

## Exports

`src/index.js` exports 23 names.

### Checking approvals

| Export | Signature | Returns |
|---|---|---|
| `verify` | `verify(approval, expected, keyring, seen, config, now)` | `Promise<{ ok: true, verified } \| { ok: false, error }>`. Never throws for bad input; anything unexpected becomes `MALFORMED`. |
| `checkKind` | `checkKind(approval, expected, keyring, config, now, standing)` | `Promise<verified>`; throws `Refused`. Every check except replay. `standing` is `true` for a standing approval, `false` for a one-time one. It does not refuse a `revoke` request (only `verify` does). |
| `MemorySeenStore` | `new MemorySeenStore()`; `.insertIfNew(id, exp, now)` | `boolean`: `true` if new. Keeps ids while `exp + 30 > now`. |
| `keyringFrom` | `keyringFrom(plain)` | `{ keys, origin, history }` from `{"keys": [...]}` JSON (`origin` is `null` and `history` `[]` when absent). Does not check the keys: a bad key fails as `UNTRUSTED_KEY` when used. |
| `Refused` | `class Refused extends Error`; `new Refused(code)` | An error whose `.code` is an error code. Throw it from your stores. |
| `requestShapeOk` | `requestShapeOk(r)` | `boolean`: `r` (a `Map`) is exactly `{type, params}`, `type` 1 to 64 of `a-z 0-9 _ . -`, `params` a `Map`. |
| `idOk` | `idOk(id)` | `boolean`: 22 base64url characters decoding to 16 bytes. |
| `VERSION` | constant | `"pka.1"` |

`verified` has this shape:

```js
{ id, request_hash, credential_id, approver, level, iat, exp }
// approver: the `name` on the key card that signed; level: "medium" or "high"
```

Arguments:

| Argument | Shape |
|---|---|
| `approval` | The approval string from the agent (base64url, at most 16384 characters). |
| `expected` | `{ aud, request, level }`: your audience, the request you are about to run, the lowest level you accept (`"medium"` or `"high"`). |
| `keyring` | `{ keys: [{ credential_id, public_key, rp_id, name, created }] }`, usually from `keyringFrom`. |
| `seen` | Any object with `insertIfNew(id, exp, now)` returning `true` for a new id, directly or as a Promise. See [stores](#stores). |
| `config` | `{ origins: [...], rp_ids: [...] }`. |
| `now` | Unix seconds, an integer. The checker never reads the clock. |

The checks and their order are the same as in Rust; see [verifier-rust](verifier-rust.md#verify) and [error codes](error-codes.md).

### Standing approvals

| Export | Signature | Returns |
|---|---|---|
| `verifyUse` | `verifyUse(standing, useToken, expected, keyring, uses, revoked, config, now)` | `Promise<{ ok: true, usesLeft, standingId, useId } \| { ok: false, error }>`. Never throws for bad input. |
| `revocationsFrom` | `revocationsFrom(approvals, aud, keyring, config)` | `Promise<Set<string>>` of revoked standing ids. Reads at most 1000 entries; invalid entries are skipped. |
| `MemoryUseStore` | `new MemoryUseStore()`; `.recordUse(standingId, useId, maxUses, until, now)` | `number` of uses left; throws `Refused("REPLAYED")` or `Refused("USED_UP")`. |
| `standingParams` | `standingParams(request)` | `Promise<{ forType, fixed, vary, maxUses, useKey }>` (`fixed` and `vary` are `Map`s); throws `Refused("MALFORMED")` for invalid params. |
| `allows` | `allows(sp, request)` | `boolean`: `request` (a `Map`) has type `forType`, exactly the fixed fields with equal canonical values and the vary fields within limits, nothing else. |

`uses` is any object with `recordUse(...)`, and `revoked` any object with `has(standingId)` (normally the `Set` from `revocationsFrom`); both may be `async`. See [stores](#stores).

### Key history

| Export | Signature | Returns |
|---|---|---|
| `verifyHistory` | `verifyHistory(ring, rootFingerprint, origin)` | `Promise<null \| code>`: `null` when the history checks out, else `"UNSIGNED"`, `"WRONG_ORIGIN"`, `"WRONG_ROOT"`, `"BAD_ENTRY"` or `"KEYS_DO_NOT_MATCH"`. |
| `verifyHistoryUpdate` | `verifyHistoryUpdate(accepted, next, rootFingerprint, origin)` | `Promise<null \| code>`; `"NOT_AN_UPDATE"` when `next` does not start with every entry of `accepted`, else as `verifyHistory(next, ...)`. |
| `fingerprint` | `fingerprint(card)` | `Promise<string>`: base64url SHA-256 of the card's SPKI public key. |
| `keyringAud` | `keyringAud(origin)` | `"phygital-key:keyring:" + origin` |

Unlike the Rust `HistoryError::BadEntry`, the JavaScript result does not say which entry failed.

### Canonical form, JSON and base64url

| Export | Signature | Returns |
|---|---|---|
| `canonical` | `canonical(v)` | RFC 8785 text of a model value; throws `Error` if a string or key is not NFC, a number is not a safe integer, or the value is a plain object. |
| `hashB64` | `hashB64(v)` | `Promise<string \| null>`: base64url SHA-256 of `canonical(v)` (`request_hash`), `null` if there is no canonical form. |
| `challenge` | `challenge(claims)` | `Promise<Uint8Array \| null>`: SHA-256 of `"phygital-key/approval/v1"`, a zero byte and `canonical(claims)`. |
| `parse` | `parse(text)` | The model value, or `undefined` on any problem. |
| `b64decode` | `b64decode(text)` | `Uint8Array`, or `null` for padding, other alphabets or non-canonical trailing bits. |
| `b64encode` | `b64encode(bytes)` | base64url text without padding. |

```js
import { parse, canonical, hashB64 } from "phygital-key-verify";

const req = parse('{"type":"deploy","params":{"version":"2.4.1","service":"api","env":"production"}}');
canonical(req);      // '{"params":{"env":"production","service":"api","version":"2.4.1"},"type":"deploy"}'
await hashB64(req);  // 'hUXvlpWHC4xarFlhKwaU26H6q05m3gBpMoyE_YKRK7o'
parse('{"a":1,"a":2}'); // undefined: duplicate key
```

## Standing approvals and revocations

The agent sends the person's standing approval (`Phygital-Approval`) and a use token for this one action (`Phygital-Use`). Fetch the gate's `/.well-known/phygital-key-revoked.json` now and then, and keep every id you accepted.

```js
import { verifyUse, revocationsFrom, MemoryUseStore, keyringFrom } from "phygital-key-verify";

const uses = new MemoryUseStore();   // one process only: see "Stores"
const revoked = new Set();           // every revocation you ever accepted

async function refreshRevocations(gateOrigin, aud, keyring, config) {
  const res = await fetch(`${gateOrigin}/.well-known/phygital-key-revoked.json`);
  const { revocations } = await res.json();
  for (const id of await revocationsFrom(revocations, aud, keyring, config)) revoked.add(id); // merge, never replace
}

const expected = { aud: "https://pay.example.com", request, level: "high" };
const result = await verifyUse(standing, useToken, expected, keyring, uses, revoked, config, Math.floor(Date.now() / 1000));
if (!result.ok) refuse(result.error); // NOT_ALLOWED, USED_UP, REVOKED, ...
// result.usesLeft, result.standingId, result.useId
```

Against `test-vectors/standing.json` (with each vector's own `now`), `valid_use` gave `{ ok: true, usesLeft: 2, standingId: 'KCgoKCgoKCgoKCgoKCgoKA', useId: 'AQEBAQEBAQEBAQEBAQEBAQ' }` and the same token again `REPLAYED`; `revoked` gave a set with that standing id and `REVOKED`; `over_max` gave `NOT_ALLOWED`. Why invalid list entries are ignored and how often to refresh: [revocations](revocations.md).

## Key history

```js
import { verifyHistory, verifyHistoryUpdate, keyringFrom } from "phygital-key-verify";

const PINNED = "Pp0IrvTI9djhcmFJeel-FJXJd1_u_tJ2R8j8hpMh1xo"; // learned from the person
const ORIGIN = "https://approve.example.com";

const next = keyringFrom(await (await fetch(`${ORIGIN}/.well-known/phygital-key.json`)).json());
const error = accepted
  ? await verifyHistoryUpdate(accepted, next, PINNED, ORIGIN)
  : await verifyHistory(next, PINNED, ORIGIN);
if (error) throw new Error(`keyring refused: ${error}`);
accepted = next; // store it; use it as the keyring for verify
```

With the vectors in `test-vectors/keyrings.json`: `valid_single_key` gives `null`, `extra_unsigned_key` gives `"KEYS_DO_NOT_MATCH"`, `valid_update` gives `null`, `rollback` gives `"NOT_AN_UPDATE"`. Every rule: [key history](key-history.md).

## Stores

`verify` and `verifyUse` call your stores last, after every other check, and `await` each answer, so a store method can be a plain function or an `async` one that talks to a database. Awaiting does not make a store safe on its own: each method must record and answer in one atomic step, shared by every instance that acts, and durable across restarts.

| Store | Method | Must return | Anything else |
|---|---|---|---|
| Seen store (`seen`) | `insertIfNew(id, exp, now)` | Exactly `true` when `id` was new and is now recorded. `false` when it was seen before. | Any other value (`1`, `"yes"`, `undefined`, an object) is `REPLAYED`. A throw or a rejected promise (other than a `Refused`) is `MALFORMED`. |
| Use store (`uses`) | `recordUse(standingId, useId, maxUses, until, now)` | The uses left after this one: a safe integer `n` with $0 \le n < \texttt{maxUses}$. | Throw `new Refused("REPLAYED")` or `new Refused("USED_UP")` for those cases. Any other return value, and any other error, is `MALFORMED`. |
| Revocation set (`revoked`) | `has(standingId)` | Truthy when the standing approval is revoked. Usually the `Set` from `revocationsFrom`. | May be `async`. A throw or a rejected promise (other than a `Refused`) is `MALFORMED`. |

So a store that fails gives a refusal, never a pass. These rules are checked by the tests "async stores are awaited" in `verifier-js/test/vectors.test.js`.

### Seen store

Record `id` and answer in one atomic step; return `true` only if it was new; keep it until at least `exp + 30`. Share it across every instance that acts. Designs for SQL and Redis: [integrate](integrate.md#the-seen-store).

### Use store

| Outcome | What to do |
|---|---|
| New use, within `maxUses` | Record it; return `maxUses` minus the uses recorded so far, this one included. |
| `useId` already recorded for `standingId` | `throw new Refused("REPLAYED")` |
| `maxUses` already recorded | `throw new Refused("USED_UP")` |
| Storage error | Throw. Any non-`Refused` error becomes `MALFORMED`, so the use is refused. |

Keep entries until `until` (the standing approval's end) plus 30 seconds. The check and the insert must be one step: this store is the only thing that enforces `maxUses` at your service.

```js
import { Refused } from "phygital-key-verify";

// Sketch of the logic, with a Map standing in for storage.
class MapUseStore {
  #rows = new Map(); // standingId -> Set of useIds
  recordUse(standingId, useId, maxUses, until, now) {
    const set = this.#rows.get(standingId) ?? new Set();
    if (set.has(useId)) throw new Refused("REPLAYED");
    if (set.size >= maxUses) throw new Refused("USED_UP");
    set.add(useId);
    this.#rows.set(standingId, set);
    return maxUses - set.size;
  }
}
```

### An async database

Pass `async` methods straight in. `db` is yours to write; the SQL is the sketch from [integrate](integrate.md#the-seen-store).

```js
import { verify, verifyUse, Refused } from "phygital-key-verify";

// db.query(sql, params) -> Promise<{ rowCount }> (node-postgres style). Sketch.
const seen = {
  async insertIfNew(id, exp, now) {
    const r = await db.query(
      "INSERT INTO pka_seen (id, keep_until) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
      [id, exp + 30],
    );
    return r.rowCount === 1; // exactly true only for a new id; a rejected query refuses
  },
};

// db.recordUse(...) -> Promise<{ left } | { error: "REPLAYED" | "USED_UP" }>, one transaction.
const uses = {
  async recordUse(standingId, useId, maxUses, until, now) {
    const row = await db.recordUse(standingId, useId, maxUses, until + 30);
    if (row.error) throw new Refused(row.error);
    return row.left;
  },
};

const once = await verify(approval, expected, keyring, seen, config, now);
const use = await verifyUse(standing, useToken, expected, keyring, uses, revoked, config, now);
```

Run against an in-memory stand-in for `db`, the seen store accepted a test approval once and answered `REPLAYED` the second time, a `db` that rejected gave `MALFORMED`, and the use store accepted the `valid_use` vector once (`usesLeft: 2`) and answered `REPLAYED` the second time.

## Node and browsers

The source uses only `crypto.subtle`, `TextEncoder` and `TextDecoder`, which Node 20+ and current browsers provide as globals. It imports nothing outside its own folder.

| Where | Notes |
|---|---|
| Node 20+ | Works as is. CI runs the tests on Node 24. |
| Browsers | Load `src/index.js` as an ES module (`<script type="module">`). `crypto.subtle` exists only in secure contexts: pages served over HTTPS or from `localhost`. |

A check in a browser can show a person whether an approval is valid. It does not protect a service: the service must run its own check on its own server, where the agent cannot change the code or the clock.

## How it is tested

`verifier-js/test/vectors.test.js` runs with `node --test` (`npm test` in `verifier-js/`), and CI runs it on every push (Node 24). It has 9 tests:

| Test | What it checks |
|---|---|
| every approval vector | All 55 cases in `test-vectors/vectors.json` give the expected result. |
| every key-history vector | All 25 cases in `keyrings.json`, using `verifyHistoryUpdate` when a vector has an `accepted` copy. |
| every standing-approval vector | All 43 cases in `standing.json`, with prior uses and revocations from each vector. |
| an approval works once | The second `verify` of the same approval is `REPLAYED`. |
| a level that is a JavaScript built-in name | `"level":"toString"` is `MALFORMED`. |
| a byte-order mark before the approval JSON | Refused as `MALFORMED`. |
| junk in a revocation list | Ignored, never thrown. |
| async stores are awaited: a database-style seen store still stops a replay | An `async insertIfNew` stops the second use; answers `1`, `"yes"`, `{}` and `undefined` are each `REPLAYED`. |
| async stores are awaited: a database-style use store still counts and stops reuse | An `async recordUse` counts the use and stops the second one; an answer that is not a safe integer is `MALFORMED`. |

The level, byte-order mark and revocation-junk tests came from security reviews of differences between the two implementations. A local run with Node 26:

```text
ℹ tests 9
ℹ pass 9
ℹ fail 0
``` The Rust verifier runs the same 123 vectors: [verifier-rust](verifier-rust.md#how-it-is-tested), [test vectors](../test-vectors/README.md).

## Next

- [Integrate](integrate.md)
- [Error codes](error-codes.md)
- [Revocations](revocations.md)
- [Rust verifier](verifier-rust.md)
