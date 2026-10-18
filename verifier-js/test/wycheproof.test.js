// Google's Wycheproof ECDSA P-256 / SHA-256 cases, run through the checker's own
// signature handling: DER (WebAuthn assertions) and raw r||s (use tokens).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { derToRaw } from "../src/verify.js";
import { scalarOk } from "../src/standing.js";

const dir = new URL("../../test-vectors/wycheproof/", import.meta.url);
const hex = (s) => Uint8Array.from(s.match(/../g) ?? [], (b) => parseInt(b, 16));

async function run(file, toRaw) {
  const doc = JSON.parse(readFileSync(new URL(file, dir), "utf8"));
  const failures = [];
  let count = 0;
  for (const g of doc.testGroups) {
    const key = await crypto.subtle.importKey("spki", hex(g.publicKeyDer), { name: "ECDSA", namedCurve: "P-256" }, false, ["verify"]);
    for (const t of g.tests) {
      count++;
      const raw = toRaw(hex(t.sig));
      const ok = raw !== null && (await crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, key, raw, hex(t.msg)));
      if ((t.result === "valid" && !ok) || (t.result === "invalid" && ok)) {
        failures.push(`tcId ${t.tcId} (${t.result}, got ${ok ? "accepted" : "refused"}): ${t.comment}`);
      }
    }
  }
  assert.equal(count, doc.numberOfTests, "ran every case");
  assert.deepEqual(failures, []);
}

test("wycheproof: DER signatures", () => run("ecdsa_secp256r1_sha256_test.json", derToRaw));

test("wycheproof: raw r||s signatures", () =>
  run("ecdsa_secp256r1_sha256_p1363_test.json", (sig) =>
    sig.length === 64 && scalarOk(sig.slice(0, 32)) && scalarOk(sig.slice(32)) ? sig : null,
  ));
