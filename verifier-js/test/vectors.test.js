// Every shared test vector must pass, exactly like the Rust verifier.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { verify, MemorySeenStore, keyringFrom } from "../src/verify.js";
import { verifyHistory, verifyHistoryUpdate } from "../src/history.js";
import { verifyUse, revocationsFrom, MemoryUseStore } from "../src/standing.js";
import { b64encode } from "../src/strict.js";

const load = (f) => JSON.parse(readFileSync(new URL(`../../test-vectors/${f}`, import.meta.url), "utf8"));

test("every approval vector", async () => {
  const failures = [];
  for (const v of load("vectors.json").vectors) {
    const i = v.input;
    const seen = new MemorySeenStore();
    for (const id of i.seen_ids) seen.insertIfNew(id, Number.MAX_SAFE_INTEGER, i.now);
    const got = await verify(i.approval, { aud: i.aud, request: i.request, level: i.level }, keyringFrom(i.keyring), seen, i.config, i.now);
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
