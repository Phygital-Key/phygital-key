// Canonical form (spec section 3): every string already NFC, safe integers only,
// then RFC 8785 (JCS): keys sorted by UTF-16 code units, strings as JSON.stringify.

import { NotSafeInteger, b64encode } from "./strict.js";

export const CHALLENGE_PREFIX = new TextEncoder().encode("phygital-key/approval/v1\u0000");

/** Throws if the value has no canonical form. Returns the canonical text. */
export function canonical(v) {
  if (v === null) return "null";
  if (v === true) return "true";
  if (v === false) return "false";
  if (v instanceof NotSafeInteger) throw new Error("not a safe integer");
  if (typeof v === "number") {
    if (!Number.isSafeInteger(v) || Object.is(v, -0)) throw new Error("not a safe integer");
    return String(v);
  }
  if (typeof v === "string") {
    if (v.normalize("NFC") !== v) throw new Error("not NFC");
    return JSON.stringify(v);
  }
  if (Array.isArray(v)) return "[" + v.map(canonical).join(",") + "]";
  if (v instanceof Map) {
    const keys = [...v.keys()];
    for (const k of keys) if (k.normalize("NFC") !== k) throw new Error("not NFC");
    // Default JavaScript sort compares UTF-16 code units, exactly as RFC 8785 asks.
    keys.sort();
    return "{" + keys.map((k) => JSON.stringify(k) + ":" + canonical(v.get(k))).join(",") + "}";
  }
  throw new Error("unsupported value");
}

/** Canonical text, or null. */
export function tryCanonical(v) {
  try {
    return canonical(v);
  } catch {
    return null;
  }
}

export async function sha256(bytes) {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
}

export const utf8 = (s) => new TextEncoder().encode(s);

export function concat(...parts) {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

export function equalBytes(a, b) {
  if (!a || !b || a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a[i] ^ b[i];
  return d === 0;
}

/** base64url SHA-256 of the canonical bytes, or null. */
export async function hashB64(v) {
  const c = tryCanonical(v);
  return c === null ? null : b64encode(await sha256(utf8(c)));
}

/** SHA-256(prefix || canonical(claims)), or null. */
export async function challenge(claims) {
  const c = tryCanonical(claims);
  return c === null ? null : sha256(concat(CHALLENGE_PREFIX, utf8(c)));
}
