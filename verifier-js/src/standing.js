// Standing approvals (spec section 7c): the person's approval with limits, plus
// one short use token per action, signed by the gate's use key.

import { b64decode, b64encode, parse, parseBytes, get, fromPlain } from "./strict.js";
import { canonical, tryCanonical, sha256, utf8, equalBytes } from "./canonical.js";
import {
  checkKind, Refused, requestShapeOk, idOk, MAX_APPROVAL_BYTES, MAX_CLOCK_SKEW_SECS, STANDING_TYPE, REVOKE_TYPE,
} from "./verify.js";

export const MAX_USES = 1000;
export const MAX_USE_SECS = 60;
export const MAX_ONE_OF = 20;
export const USE_TYP = "pka-use+jwt";

const no = (code) => {
  throw new Refused(code);
};
const isInt = (v) => typeof v === "number" && Number.isSafeInteger(v);
const keysAre = (m, keys) => m instanceof Map && m.size === keys.length && keys.every((k) => m.has(k));

async function importKey(spki) {
  const der = b64decode(spki);
  if (!der) return null;
  try {
    return await crypto.subtle.importKey("spki", der, { name: "ECDSA", namedCurve: "P-256" }, false, ["verify"]);
  } catch {
    return null;
  }
}

function limitOk(l) {
  if (keysAre(l, ["min", "max"])) return isInt(l.get("min")) && isInt(l.get("max")) && l.get("min") <= l.get("max");
  if (keysAre(l, ["one_of"])) {
    const list = l.get("one_of");
    return Array.isArray(list) && list.length >= 1 && list.length <= MAX_ONE_OF && list.every((s) => typeof s === "string");
  }
  return false;
}

/** Read and check the params of a standing request (throws MALFORMED). */
export async function standingParams(request) {
  if (get(request, "type") !== STANDING_TYPE) no("MALFORMED");
  const p = get(request, "params");
  if (!keysAre(p, ["for_type", "fixed", "vary", "max_uses", "use_key"])) no("MALFORMED");
  const forType = p.get("for_type");
  const fixed = p.get("fixed");
  const vary = p.get("vary");
  const maxUses = p.get("max_uses");
  if (typeof forType !== "string" || !(fixed instanceof Map) || !(vary instanceof Map) || !isInt(maxUses) || typeof p.get("use_key") !== "string") no("MALFORMED");
  const shape = new Map([["type", forType], ["params", new Map()]]);
  if (!requestShapeOk(shape) || forType === STANDING_TYPE) no("MALFORMED");
  if (maxUses < 1 || maxUses > MAX_USES) no("MALFORMED");
  for (const k of fixed.keys()) if (vary.has(k)) no("MALFORMED");
  for (const l of vary.values()) if (!limitOk(l)) no("MALFORMED");
  if (!(await importKey(p.get("use_key")))) no("MALFORMED");
  return { forType, fixed, vary, maxUses, useKey: p.get("use_key") };
}

/** Exactly the fixed fields with the same values, the varying fields within limits, nothing else. */
export function allows(sp, request) {
  if (get(request, "type") !== sp.forType) return false;
  const params = get(request, "params");
  if (!(params instanceof Map) || params.size !== sp.fixed.size + sp.vary.size) return false;
  for (const [k, want] of sp.fixed) {
    if (!params.has(k)) return false;
    const a = tryCanonical(params.get(k));
    const b = tryCanonical(want);
    if (a === null || b === null || a !== b) return false;
  }
  for (const [k, l] of sp.vary) {
    if (!params.has(k)) return false;
    const v = params.get(k);
    if (l.has("one_of")) {
      if (typeof v !== "string" || !l.get("one_of").includes(v)) return false;
    } else if (!isInt(v) || v < l.get("min") || v > l.get("max")) {
      return false;
    }
  }
  return true;
}

// Raw 64-byte r||s with r, s in [1, n-1] (what the spec's JWS uses).
const N = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551n;
function scalarOk(bytes) {
  let v = 0n;
  for (const b of bytes) v = (v << 8n) | BigInt(b);
  return v > 0n && v < N;
}

async function readUse(jws, useKey) {
  if (typeof jws !== "string" || jws.trim() === "") no("APPROVAL_REQUIRED");
  if (new TextEncoder().encode(jws).length > MAX_APPROVAL_BYTES) no("MALFORMED");
  const parts = jws.split(".");
  if (parts.length !== 3) no("MALFORMED");
  const [h, p, s] = parts;
  // The header must be exactly these bytes (canonical JSON), as the spec says.
  if (h !== b64encode(utf8(`{"alg":"ES256","typ":"${USE_TYP}"}`))) no("MALFORMED");
  const sig = b64decode(s);
  if (!sig || sig.length !== 64 || !scalarOk(sig.slice(0, 32)) || !scalarOk(sig.slice(32))) no("MALFORMED");
  const key = (await importKey(useKey)) ?? no("MALFORMED");
  const good = await crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, key, sig, utf8(`${h}.${p}`));
  if (!good) no("BAD_SIGNATURE");
  const pb = b64decode(p);
  const u = pb && parseBytes(pb);
  const fields = ["v", "standing_id", "id", "aud", "request", "request_hash", "iat", "exp"];
  if (!keysAre(u, fields)) no("MALFORMED");
  const val = Object.fromEntries(fields.map((k) => [k, u.get(k)]));
  for (const k of ["v", "standing_id", "id", "aud", "request_hash"]) if (typeof val[k] !== "string") no("MALFORMED");
  if (!isInt(val.iat) || !isInt(val.exp)) no("MALFORMED");
  if (val.v !== "pka.use.1" || !idOk(val.id)) no("MALFORMED");
  return val;
}

/** Counts uses of standing approvals, in memory. */
export class MemoryUseStore {
  #uses = new Map();
  recordUse(standingId, useId, maxUses, until, now) {
    for (const [k, [u]] of this.#uses) if (u + MAX_CLOCK_SKEW_SECS <= now) this.#uses.delete(k);
    if (!this.#uses.has(standingId)) this.#uses.set(standingId, [until, new Set()]);
    const set = this.#uses.get(standingId)[1];
    if (set.has(useId)) no("REPLAYED");
    if (set.size >= maxUses) no("USED_UP");
    set.add(useId);
    return maxUses - set.size;
  }
}

/** Revoked standing ids, each proven by a trusted signature (unsigned entries are ignored). */
export async function revocationsFrom(approvals, aud, keyring, config) {
  const ids = new Set();
  for (const a of (approvals ?? []).slice(0, 1000)) {
    if (typeof a !== "string" || a.length > MAX_APPROVAL_BYTES) continue;
    const raw = b64decode(a);
    const claims = get(raw && parseBytes(raw), "claims");
    const request = get(claims, "request");
    const iat = get(claims, "iat");
    const sid = get(request, "params", "standing_id");
    if (!isInt(iat) || typeof sid !== "string") continue;
    const want = new Map([["type", REVOKE_TYPE], ["params", new Map([["standing_id", sid]])]]);
    const wantText = tryCanonical(want);
    if (wantText === null || tryCanonical(request) !== wantText) continue;
    try {
      await checkKind(a, { aud, request, level: "medium" }, keyring, config, iat, false);
      ids.add(sid);
    } catch {
      // not a valid revocation: ignored
    }
  }
  return ids;
}

/** Verify one use. Returns { ok: true, usesLeft } or { ok: false, error }. */
export async function verifyUse(standing, useToken, expected, keyring, uses, revoked, config, now) {
  try {
    if (typeof standing !== "string" || standing.trim() === "") no("APPROVAL_REQUIRED");
    const raw = standing.length <= MAX_APPROVAL_BYTES ? b64decode(standing) : null;
    if (!raw) no("MALFORMED");
    const signedRequest = get(parseBytes(raw), "claims", "request");
    if (signedRequest === undefined) no("MALFORMED");
    const sp = await standingParams(signedRequest);
    const person = await checkKind(standing, { aud: expected.aud, request: signedRequest, level: expected.level }, keyring, config, now, true);
    if (revoked.has(person.id)) no("REVOKED");
    const u = await readUse(useToken, sp.useKey);
    if (u.standing_id !== person.id) no("REQUEST_MISMATCH");
    if (u.aud !== expected.aud) no("WRONG_AUDIENCE");
    if (u.iat > now + MAX_CLOCK_SKEW_SECS || now >= u.exp || u.exp <= u.iat || u.exp - u.iat > MAX_USE_SECS || u.exp > person.exp) no("EXPIRED");
    const req = expected.request instanceof Map ? expected.request : fromPlain(expected.request);
    if (!requestShapeOk(req) || req.get("type") === STANDING_TYPE) no("MALFORMED");
    const ours = tryCanonical(req) ?? no("MALFORMED");
    const theirs = tryCanonical(u.request) ?? no("MALFORMED");
    const oursHash = b64encode(await sha256(utf8(ours)));
    if (ours !== theirs || oursHash !== u.request_hash) no("REQUEST_MISMATCH");
    if (!allows(sp, req)) no("NOT_ALLOWED");
    const usesLeft = uses.recordUse(person.id, u.id, sp.maxUses, person.exp, now);
    return { ok: true, usesLeft, standingId: person.id, useId: u.id };
  } catch (e) {
    if (e instanceof Refused) return { ok: false, error: e.code };
    return { ok: false, error: "MALFORMED" };
  }
}

export { parse, equalBytes };
