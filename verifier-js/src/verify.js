// The verifier (spec section 7): every check, in order, fail closed.
// An independent implementation: it shares no code with the Rust verifier,
// only the spec and the test vectors.

import { MAX_DEPTH, NotSafeInteger, b64decode, b64encode, get, parseBytes, fromPlain } from "./strict.js";
import { canonical, tryCanonical, challenge, sha256, utf8, concat, equalBytes } from "./canonical.js";
import { agentKeyOk, proofError } from "./bound.js";

export const VERSION = "pka.1";
export const VERSION_BOUND = "pka.2";
export const MAX_APPROVAL_BYTES = 16 * 1024;
export const MAX_REQUEST_BYTES = 8 * 1024;
export const MAX_LIFETIME_SECS = 300;
export const MAX_STANDING_SECS = 30 * 24 * 3600;
export const MAX_CLOCK_SKEW_SECS = 30;
export const MAX_CLIENT_DATA = 4096;
export const STANDING_TYPE = "standing";
export const REVOKE_TYPE = "revoke";

// Own properties only: `"toString" in {}` is true in JavaScript.
const LEVELS = Object.freeze(Object.assign(Object.create(null), { medium: 1, high: 2 }));
const isLevel = (l) => l === "medium" || l === "high";

/** A refusal: `code` is one of the spec's error codes. */
export class Refused extends Error {
  constructor(code) {
    super(code);
    this.code = code;
  }
}
const no = (code) => {
  throw new Refused(code);
};

const isInt = (v) => typeof v === "number" && Number.isSafeInteger(v);
const isStr = (v) => typeof v === "string";
const keysAre = (m, keys) => m instanceof Map && m.size === keys.length && keys.every((k) => m.has(k));

export function requestShapeOk(r) {
  if (!keysAre(r, ["type", "params"])) return false;
  const t = r.get("type");
  return isStr(t) && t.length >= 1 && t.length <= 64 && /^[a-z0-9_.-]+$/.test(t) && r.get("params") instanceof Map;
}

export function idOk(id) {
  if (!isStr(id) || id.length !== 22) return false;
  const b = b64decode(id);
  return b !== null && b.length === 16;
}

function asModel(v) {
  return v instanceof Map || Array.isArray(v) ? v : fromPlain(v);
}

/** Keyring from plain JSON ({"keys": [...]}). */
export function keyringFrom(plain) {
  const keys = (plain?.keys ?? []).map((k) => ({ ...k }));
  return { keys, origin: plain?.origin ?? null, history: plain?.history ?? [] };
}

async function importKey(spkiB64) {
  const der = b64decode(spkiB64);
  if (!der) return null;
  try {
    return await crypto.subtle.importKey("spki", der, { name: "ECDSA", namedCurve: "P-256" }, false, ["verify"]);
  } catch {
    return null;
  }
}

// Strict DER: SEQUENCE { INTEGER r, INTEGER s } -> 64-byte r||s.
function derToRaw(der) {
  const fail = () => null;
  if (der.length < 8 || der[0] !== 0x30 || der[1] !== der.length - 2 || der[1] > 0x7f) return fail();
  let i = 2;
  const int = () => {
    if (der[i] !== 0x02) return null;
    const len = der[i + 1];
    if (len < 1 || len > 33 || i + 2 + len > der.length) return null;
    let v = der.slice(i + 2, i + 2 + len);
    // Minimal encoding: no needless leading zero, no negative numbers.
    if (v[0] & 0x80) return null;
    if (v.length > 1 && v[0] === 0 && !(v[1] & 0x80)) return null;
    if (v[0] === 0) v = v.slice(1);
    if (v.length > 32) return null;
    i += 2 + len;
    const out = new Uint8Array(32);
    out.set(v, 32 - v.length);
    return out;
  };
  const r = int();
  const s = r && int();
  if (!r || !s || i !== der.length) return fail();
  return concat(r, s);
}

/** Client data checks: type, challenge, origin, not cross-origin. */
export function clientDataOk(bytes, kind, wantChallenge, origins) {
  if (bytes.length > MAX_CLIENT_DATA) return false;
  const cd = parseBytes(bytes);
  if (!(cd instanceof Map)) return false;
  if (cd.get("type") !== kind) return false;
  const ch = isStr(cd.get("challenge")) ? b64decode(cd.get("challenge")) : null;
  if (!ch || !equalBytes(ch, wantChallenge)) return false;
  if (!isStr(cd.get("origin")) || !origins.includes(cd.get("origin"))) return false;
  const co = cd.get("crossOrigin");
  if (co === true || isStr(co)) return false;
  return !cd.has("topOrigin");
}

/** Every check except replay. `standing` selects one-time or standing approvals. */
export async function checkKind(approval, expected, keyring, config, now, standing) {
  // 1. Present, well formed, within limits.
  if (!isStr(approval) || approval.trim() === "") no("APPROVAL_REQUIRED");
  if (new TextEncoder().encode(approval).length > MAX_APPROVAL_BYTES) no("MALFORMED");
  const raw = b64decode(approval) ?? no("MALFORMED");
  const obj = parseBytes(raw);
  if (!keysAre(obj, ["claims", "signatures"])) no("MALFORMED");
  const claims = obj.get("claims");
  const sigs = obj.get("signatures");
  // pka.1 has exactly these claims; pka.2 adds cnf, the asking agent's key.
  const CLAIMS = ["v", "id", "aud", "request", "request_hash", "level", "iat", "exp"];
  const bound = claims instanceof Map && claims.get("v") === VERSION_BOUND;
  if (!keysAre(claims, bound ? [...CLAIMS, "cnf"] : CLAIMS)) no("MALFORMED");
  let agentKey = null;
  if (bound) {
    const cnf = claims.get("cnf");
    if (!keysAre(cnf, ["key"]) || !(await agentKeyOk(cnf.get("key")))) no("MALFORMED");
    agentKey = cnf.get("key");
  }
  const [v, id, aud, request, requestHash, level, iat, exp] = ["v", "id", "aud", "request", "request_hash", "level", "iat", "exp"].map((k) => claims.get(k));
  if (!isStr(v) || !isStr(id) || !isStr(aud) || !isStr(requestHash) || !isLevel(level) || !isInt(iat) || !isInt(exp)) no("MALFORMED");
  if (!Array.isArray(sigs) || !sigs.every((s) => keysAre(s, ["credential_id", "authenticator_data", "client_data_json", "signature"]) && [...s.values()].every(isStr))) no("MALFORMED");
  if ((v !== VERSION && v !== VERSION_BOUND) || !idOk(id) || !requestShapeOk(request)) no("MALFORMED");
  const isStanding = request.get("type") === STANDING_TYPE;
  if (isStanding !== standing || (standing && agentKey !== null)) no("MALFORMED");
  if (standing && level !== "high") no("ASSURANCE_TOO_LOW");
  const maxLife = standing ? MAX_STANDING_SECS : MAX_LIFETIME_SECS;
  if (sigs.length !== 1) no("MALFORMED");
  const sig = sigs[0];
  const chal = (await challenge(claims)) ?? no("MALFORMED");
  const claimsRequest = tryCanonical(request) ?? no("MALFORMED");
  if (utf8(claimsRequest).length > MAX_REQUEST_BYTES) no("MALFORMED");
  const cred = b64decode(sig.get("credential_id"));
  const authData = b64decode(sig.get("authenticator_data"));
  const clientData = b64decode(sig.get("client_data_json"));
  const sigDer = b64decode(sig.get("signature"));
  if (!cred || !authData || !clientData || !sigDer || cred.length === 0) no("MALFORMED");

  // 2. Made for us.
  if (aud !== expected.aud) no("WRONG_AUDIENCE");

  // 3. Time.
  if (iat > now + MAX_CLOCK_SKEW_SECS || now >= exp || exp <= iat || exp - iat > maxLife) no("EXPIRED");

  // 4. Exactly the request we are about to run.
  const expReq = asModel(expected.request);
  const ours = tryCanonical(expReq) ?? no("MALFORMED");
  if (!requestShapeOk(expReq)) no("MALFORMED");
  const oursHash = b64encode(await sha256(utf8(ours)));
  const theirsHash = b64encode(await sha256(utf8(claimsRequest)));
  if (oursHash !== requestHash || theirsHash !== requestHash) no("REQUEST_MISMATCH");

  // 5. Level, decided by the verifier.
  if (!isLevel(expected.level) || LEVELS[level] < LEVELS[expected.level]) no("ASSURANCE_TOO_LOW");

  // 6. A trusted key.
  const card = keyring.keys.find((k) => k.credential_id === sig.get("credential_id")) ?? no("UNTRUSTED_KEY");
  if (!config.rp_ids.includes(card.rp_id)) no("UNTRUSTED_KEY");
  const key = (await importKey(card.public_key)) ?? no("UNTRUSTED_KEY");

  // 7. The assertion.
  if (!clientDataOk(clientData, "webauthn.get", chal, config.origins)) no("BAD_SIGNATURE");
  if (authData.length < 37) no("BAD_SIGNATURE");
  if (!equalBytes(authData.slice(0, 32), await sha256(utf8(card.rp_id)))) no("BAD_SIGNATURE");
  const rawSig = derToRaw(sigDer) ?? no("BAD_SIGNATURE");
  const signed = concat(authData, await sha256(clientData));
  const good = await crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, key, rawSig, signed);
  if (!good) no("BAD_SIGNATURE");

  // 8. Presence and verification.
  const flags = authData[32];
  if (!(flags & 0x01) || (level === "high" && !(flags & 0x04))) no("ASSURANCE_TOO_LOW");

  return { id, request_hash: requestHash, credential_id: card.credential_id, approver: card.name, level, iat, exp, aud, agent_key: agentKey };
}

/** A seen store in memory. */
export class MemorySeenStore {
  #seen = new Map();
  insertIfNew(id, exp, now) {
    for (const [k, e] of this.#seen) if (e + MAX_CLOCK_SKEW_SECS <= now) this.#seen.delete(k);
    if (this.#seen.has(id)) return false;
    this.#seen.set(id, exp);
    return true;
  }
}

/**
 * Verify an approval before acting on `expected.request`.
 * Returns { ok: true, verified } or { ok: false, error }. Never throws for bad input.
 * `binding` (optional): { proof, required } for agent-bound approvals (pka.2):
 * `proof` is the agent's proof (the Phygital-Proof header); `required` refuses
 * approvals that are not bound.
 */
export async function verify(approval, expected, keyring, seen, config, now, binding = {}) {
  try {
    // A revocation is never an action to carry out.
    if (asModel(expected.request)?.get?.("type") === REVOKE_TYPE) no("MALFORMED");
    const verified = await checkKind(approval, expected, keyring, config, now, false);
    // 7b. Bound to the agent that asked: its proof must come along.
    if (verified.agent_key !== null) {
      if (binding?.proof === undefined || binding?.proof === null) no("PROOF_REQUIRED");
      const err = await proofError(binding.proof, verified, now);
      if (err) no(err);
    } else if (binding?.required === true) {
      no("PROOF_REQUIRED");
    }
    // The store may be sync or async; only an exact `true` means "first use".
    if ((await seen.insertIfNew(verified.id, verified.exp, now)) !== true) no("REPLAYED");
    return { ok: true, verified };
  } catch (e) {
    if (e instanceof Refused) return { ok: false, error: e.code };
    return { ok: false, error: "MALFORMED" };
  }
}

export { canonical, b64decode, b64encode, MAX_DEPTH, NotSafeInteger, get, derToRaw };
