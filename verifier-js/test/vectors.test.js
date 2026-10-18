// Every shared test vector must pass, exactly like the Rust verifier.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { verify, MemorySeenStore, keyringFrom } from "../src/verify.js";
import { verifyHistory, verifyHistoryUpdate } from "../src/history.js";
import { verifyUse, revocationsFrom, MemoryUseStore } from "../src/standing.js";
import { b64encode, b64decode } from "../src/strict.js";

const load = (f) => JSON.parse(readFileSync(new URL(`../../test-vectors/${f}`, import.meta.url), "utf8"));

test("every approval vector", async () => {
  const failures = [];
  for (const v of load("vectors.json").vectors) {
    const i = v.input;
    const seen = new MemorySeenStore();
    for (const id of i.seen_ids) seen.insertIfNew(id, Number.MAX_SAFE_INTEGER, i.now);
    const got = await verify(i.approval, { aud: i.aud, request: i.request, level: i.level }, keyringFrom(i.keyring), seen, i.config, i.now, { proof: i.proof, required: i.require_binding === true });
    const want = v.expected.ok ? "ok" : v.expected.error;
    const have = got.ok ? "ok" : got.error;
    if (want !== have) failures.push(`${v.name}: want ${want}, got ${have}`);
  }
  assert.deepEqual(failures, []);
});

test("every key-history vector", async () => {
  const failures = [];
  for (const v of load("keyrings.json").vectors) {
    const ring = keyringFrom(v.keyring);
    const origin = v.origin ?? "https://approve.example.com";
    const err = v.accepted
      ? await verifyHistoryUpdate(keyringFrom(v.accepted), ring, v.root_fingerprint, origin)
      : await verifyHistory(ring, v.root_fingerprint, origin);
    const want = v.expected.ok ? "ok" : v.expected.error;
    const have = err ?? "ok";
    if (want !== have) failures.push(`${v.name}: want ${want}, got ${have}`);
  }
  assert.deepEqual(failures, []);
});

test("every standing-approval vector", async () => {
  const failures = [];
  const sid = b64encode(new Uint8Array(16).fill(40));
  for (const v of load("standing.json").vectors) {
    const i = v.input;
    const keyring = keyringFrom(i.keyring);
    const uses = new MemoryUseStore();
    for (const id of i.prior_uses) uses.recordUse(sid, id, 3, Number.MAX_SAFE_INTEGER, i.now);
    const revoked = await revocationsFrom(i.revocations ?? [], i.aud, keyring, i.config);
    const got = await verifyUse(i.standing, i.use, { aud: i.aud, request: i.request, level: i.level }, keyring, uses, revoked, i.config, i.now);
    const want = v.expected.ok ? "ok" : v.expected.error;
    const have = got.ok ? "ok" : got.error;
    if (want !== have) failures.push(`${v.name}: want ${want}, got ${have}`);
  }
  assert.deepEqual(failures, []);
});

test("an approval works once", async () => {
  const v = load("vectors.json").vectors[0];
  const i = v.input;
  const seen = new MemorySeenStore();
  const e = { aud: i.aud, request: i.request, level: "medium" };
  assert.equal((await verify(i.approval, e, keyringFrom(i.keyring), seen, i.config, i.now)).ok, true);
  assert.equal((await verify(i.approval, e, keyringFrom(i.keyring), seen, i.config, i.now)).error, "REPLAYED");
});

// Differences found by the security review (Rust refuses all of these).
import { b64decode as dec } from "../src/strict.js";
test("a level that is a JavaScript built-in name is refused", async () => {
  const v = load("vectors.json").vectors[0];
  const raw = new TextDecoder().decode(dec(v.input.approval)).replace('"level":"medium"', '"level":"toString"');
  const tampered = b64encode(new TextEncoder().encode(raw));
  const i = v.input;
  const got = await verify(tampered, { aud: i.aud, request: i.request, level: "high" }, keyringFrom(i.keyring), new MemorySeenStore(), i.config, i.now);
  assert.equal(got.error, "MALFORMED");
});
test("a byte-order mark before the approval JSON is refused", async () => {
  const v = load("vectors.json").vectors[0];
  const bytes = dec(v.input.approval);
  const withBom = new Uint8Array([0xef, 0xbb, 0xbf, ...bytes]);
  const i = v.input;
  const got = await verify(b64encode(withBom), { aud: i.aud, request: i.request, level: "medium" }, keyringFrom(i.keyring), new MemorySeenStore(), i.config, i.now);
  assert.equal(got.error, "MALFORMED");
});
test("junk in a revocation list is ignored, never thrown", async () => {
  const s = load("standing.json").vectors[0].input;
  const junk = b64encode(new TextEncoder().encode(JSON.stringify({ claims: { iat: 1, request: { type: "revoke", params: { standing_id: "Kirk" } } } })));
  const ids = await revocationsFrom([junk, "!!", 5], s.aud, keyringFrom(s.keyring), s.config);
  assert.equal(ids.size, 0);
});

test("async stores are awaited: a database-style seen store still stops a replay", async () => {
  const i = load("vectors.json").vectors[0].input;
  const inner = new MemorySeenStore();
  const seen = { insertIfNew: async (id, exp, now) => inner.insertIfNew(id, exp, now) };
  const e = { aud: i.aud, request: i.request, level: "medium" };
  assert.equal((await verify(i.approval, e, keyringFrom(i.keyring), seen, i.config, i.now)).ok, true);
  assert.equal((await verify(i.approval, e, keyringFrom(i.keyring), seen, i.config, i.now)).error, "REPLAYED");
  // Anything but an exact `true` from the store is a no.
  for (const answer of [1, "yes", {}, undefined]) {
    const odd = { insertIfNew: async () => answer };
    assert.equal((await verify(i.approval, e, keyringFrom(i.keyring), odd, i.config, i.now)).error, "REPLAYED");
  }
});

test("async stores are awaited: a database-style use store still counts and stops reuse", async () => {
  const ok = load("standing.json").vectors.find((v) => v.expected.ok && v.input.prior_uses.length === 0);
  const i = ok.input;
  const keyring = keyringFrom(i.keyring);
  const inner = new MemoryUseStore();
  const uses = { recordUse: async (...a) => inner.recordUse(...a) };
  const e = { aud: i.aud, request: i.request, level: i.level };
  const first = await verifyUse(i.standing, i.use, e, keyring, uses, new Set(), i.config, i.now);
  assert.equal(first.ok, true);
  assert.equal(Number.isSafeInteger(first.usesLeft), true);
  assert.equal((await verifyUse(i.standing, i.use, e, keyring, uses, new Set(), i.config, i.now)).error, "REPLAYED");
  const odd = { recordUse: async () => "many" };
  assert.equal((await verifyUse(i.standing, i.use, e, keyring, odd, new Set(), i.config, i.now)).error, "MALFORMED");
});

test("signProof makes a proof the checker accepts for a bound approval", async () => {
  const { signProof } = await import("../src/bound.js");
  const keys = JSON.parse(readFileSync(new URL("../../test-vectors/keys/test-keys.json", import.meta.url), "utf8")).keys;
  const agent = keys.find((k) => k.name === "agent");
  const spki = b64decode(agent.card.public_key);
  const pub = await crypto.subtle.importKey("spki", spki, { name: "ECDSA", namedCurve: "P-256" }, true, ["verify"]);
  const jwk = await crypto.subtle.exportKey("jwk", pub);
  const priv = await crypto.subtle.importKey("jwk", { ...jwk, d: agent.private_scalar_b64, key_ops: ["sign"] }, { name: "ECDSA", namedCurve: "P-256" }, false, ["sign"]);
  const v = load("vectors.json").vectors.find((x) => x.name === "valid_bound");
  const i = v.input;
  const claims = JSON.parse(new TextDecoder().decode(b64decode(i.approval))).claims;
  const proof = await signProof({ approvalId: claims.id, aud: claims.aud, requestHash: claims.request_hash, iat: i.now }, priv);
  const e = { aud: i.aud, request: i.request, level: i.level };
  const ok = await verify(i.approval, e, keyringFrom(i.keyring), new MemorySeenStore(), i.config, i.now, { proof });
  assert.equal(ok.ok, true, ok.error);
  const late = await signProof({ approvalId: claims.id, aud: claims.aud, requestHash: claims.request_hash, iat: i.now - 31 }, priv);
  assert.equal((await verify(i.approval, e, keyringFrom(i.keyring), new MemorySeenStore(), i.config, i.now, { proof: late })).error, "BAD_PROOF");
  assert.equal((await verify(i.approval, e, keyringFrom(i.keyring), new MemorySeenStore(), i.config, i.now)).error, "PROOF_REQUIRED");
});
