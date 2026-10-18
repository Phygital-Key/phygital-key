// Cross-check against an independent WebAuthn library.
//
// Every approval in test-vectors/vectors.json carries one WebAuthn assertion.
// SimpleWebAuthn (@simplewebauthn/server, a widely used relying-party library
// that shares no code with us) checks that assertion on its own: signature,
// challenge, origin, RP ID, user presence and, for "high", user verification.
// Where our verifiers accept, it must accept. Where they refuse for a passkey
// reason, it must refuse too, or the case must be in STRICTER below: a rule we
// add on purpose that a general WebAuthn library does not have.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { verifyAuthenticationResponse } from "@simplewebauthn/server";
import { isoCBOR } from "@simplewebauthn/server/helpers";
import { parse, b64decode, b64encode } from "../verifier-js/src/strict.js";
import { challenge } from "../verifier-js/src/canonical.js";

const vectors = JSON.parse(readFileSync(new URL("../test-vectors/vectors.json", import.meta.url), "utf8")).vectors;

// Refused by us, accepted by a general WebAuthn library, on purpose.
const STRICTER = {
  cross_origin_frame: "we refuse crossOrigin: true even with no topOrigin; SimpleWebAuthn only checks topOrigin when present",
  client_data_4097_bytes: "our 4096-byte client data limit",
};
// Refusals that are not about the passkey assertion (expiry, request, audience, replay, format).
const PASSKEY_ERRORS = new Set(["BAD_SIGNATURE", "ASSURANCE_TOO_LOW"]);
const POLICY_ONLY = new Set(["medium_when_high_required"]); // the assertion is fine; the service wants more

// P-256 SPKI (DER) -> COSE EC2 key, as WebAuthn stores it.
function cose(spki) {
  const point = spki.slice(spki.length - 65);
  assert.equal(point[0], 4, "uncompressed point");
  return isoCBOR.encode(new Map([[1, 2], [3, -7], [-1, 1], [-2, point.slice(1, 33)], [-3, point.slice(33)]]));
}

async function theirs(v) {
  const bytes = b64decode(v.input.approval);
  const obj = bytes && parse(new TextDecoder().decode(bytes));
  const claims = obj?.get?.("claims");
  const sig = obj?.get?.("signatures")?.[0];
  if (!claims || !sig) return null;
  const card = v.input.keyring.keys.find((k) => k.credential_id === sig.get("credential_id"));
  const chal = await challenge(claims);
  if (!card || !chal) return null;
  const id = sig.get("credential_id");
  try {
    const r = await verifyAuthenticationResponse({
      response: {
        id, rawId: id, type: "public-key", clientExtensionResults: {},
        response: { authenticatorData: sig.get("authenticator_data"), clientDataJSON: sig.get("client_data_json"), signature: sig.get("signature") },
      },
      expectedChallenge: b64encode(chal),
      expectedOrigin: v.input.config.origins,
      expectedRPID: v.input.config.rp_ids,
      credential: { id, publicKey: cose(b64decode(card.public_key)), counter: 0 },
      requireUserVerification: claims.get("level") === "high",
    });
    return r.verified ? "accepted" : "refused";
  } catch (e) {
    return "refused: " + e.message;
  }
}

test("SimpleWebAuthn agrees on the passkey part of every approval vector", async () => {
  const disagree = [];
  let compared = 0;
  for (const v of vectors) {
    const ours = v.expected.ok ? "accepted" : v.expected.error;
    if (!v.expected.ok && (!PASSKEY_ERRORS.has(ours) || POLICY_ONLY.has(v.name))) continue;
    const got = await theirs(v);
    if (got === null) continue;
    compared++;
    const agree = v.expected.ok ? got === "accepted" : got !== "accepted";
    if (agree === !(v.name in STRICTER)) continue;
    disagree.push(`${v.name}: ours ${ours}, SimpleWebAuthn ${got}${v.name in STRICTER ? " (listed as stricter, but they agree now)" : ""}`);
  }
  assert.ok(compared >= 20, `compared ${compared}`);
  assert.deepEqual(disagree, []);
});
