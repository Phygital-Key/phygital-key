// Agent-bound approvals (pka.2): the agent's proof (spec section 7d).
// A compact JWS, ES256 with raw r||s, signed by the key in claims.cnf.key.

import { b64decode, b64encode, parseBytes } from "./strict.js";
import { canonical, utf8 } from "./canonical.js";

export const PROOF_TYP = "pka-proof+jwt";
export const PROOF_VERSION = "pka.proof.1";
export const MAX_PROOF_BYTES = 4096;
const SKEW = 30;

const HEADER = b64encode(utf8(`{"alg":"ES256","typ":"${PROOF_TYP}"}`));
const isStr = (v) => typeof v === "string";
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

/** True when `spki` (base64url SPKI DER) is a P-256 public key. */
export async function agentKeyOk(spki) {
  return isStr(spki) && (await importKey(spki)) !== null;
}

/**
 * Check `jws` against a checked pka.2 approval `ok` (from checkKind), at `now`.
 * Returns null when it checks out, or an error code.
 */
export async function proofError(jws, ok, now) {
  if (!isStr(jws) || jws.trim() === "") return "PROOF_REQUIRED";
  if (new TextEncoder().encode(jws).length > MAX_PROOF_BYTES) return "BAD_PROOF";
  const parts = jws.split(".");
  if (parts.length !== 3) return "BAD_PROOF";
  const [h, p, s] = parts;
  if (h !== HEADER) return "BAD_PROOF";
  const sig = b64decode(s);
  if (!sig || sig.length !== 64) return "BAD_PROOF";
  const key = await importKey(ok.agent_key);
  if (!key) return "BAD_PROOF";
  const good = await crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, key, sig, utf8(`${h}.${p}`));
  if (!good) return "BAD_PROOF";
  let body;
  try {
    const raw = b64decode(p);
    body = raw && parseBytes(raw);
  } catch {
    return "BAD_PROOF";
  }
  if (!keysAre(body, ["v", "approval_id", "aud", "request_hash", "iat"])) return "BAD_PROOF";
  const [v, approvalId, aud, requestHash, iat] = ["v", "approval_id", "aud", "request_hash", "iat"].map((k) => body.get(k));
  if (![v, approvalId, aud, requestHash].every(isStr) || !isInt(iat)) return "BAD_PROOF";
  const fresh = iat >= now - SKEW && iat <= now + SKEW && iat >= ok.iat - SKEW && iat < ok.exp;
  if (v !== PROOF_VERSION || approvalId !== ok.id || aud !== ok.aud || requestHash !== ok.request_hash || !fresh) return "BAD_PROOF";
  return null;
}

/**
 * For agents: sign a proof for an approval with the agent's private key
 * (a WebCrypto ECDSA P-256 CryptoKey). Returns the compact JWS.
 */
export async function signProof({ approvalId, aud, requestHash, iat }, privateKey) {
  const payload = b64encode(utf8(canonical(new Map([["v", PROOF_VERSION], ["approval_id", approvalId], ["aud", aud], ["request_hash", requestHash], ["iat", iat]]))));
  const input = `${HEADER}.${payload}`;
  const sig = new Uint8Array(await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, privateKey, utf8(input)));
  return `${input}.${b64encode(sig)}`;
}
