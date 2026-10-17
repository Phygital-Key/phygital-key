// Signed key history (spec section 5).

import { b64decode, b64encode, parseBytes, get } from "./strict.js";
import { sha256 } from "./canonical.js";
import { checkKind, Refused } from "./verify.js";

export const keyringAud = (origin) => `phygital-key:keyring:${origin}`;

const CARD_KEYS = ["credential_id", "public_key", "rp_id", "name", "created"];

function cardFrom(params) {
  if (!(params instanceof Map) || params.size !== CARD_KEYS.length) return null;
  const card = {};
  for (const k of CARD_KEYS) {
    const v = params.get(k);
    if (typeof v !== "string") return null;
    card[k] = v;
  }
  return card;
}

export async function fingerprint(card) {
  const der = b64decode(card.public_key) ?? new Uint8Array();
  return b64encode(await sha256(der));
}

function peek(approval) {
  const raw = b64decode(approval);
  const v = raw && parseBytes(raw);
  const claims = get(v, "claims");
  const iat = get(claims, "iat");
  return claims instanceof Map && typeof iat === "number" ? { claims, iat } : null;
}

const sameCard = (a, b) => CARD_KEYS.every((k) => a[k] === b[k]);

function sameKeys(a, b) {
  return a.length === b.length && a.every((x) => b.filter((y) => sameCard(x, y)).length === 1);
}

async function keyOk(spki) {
  const der = b64decode(spki);
  if (!der) return false;
  try {
    await crypto.subtle.importKey("spki", der, { name: "ECDSA", namedCurve: "P-256" }, false, ["verify"]);
    return true;
  } catch {
    return false;
  }
}

/** Returns null when the history checks out, else an error code. */
export async function verifyHistory(ring, rootFingerprint, origin) {
  if (!ring.origin || !ring.history || ring.history.length === 0) return "UNSIGNED";
  if (ring.origin !== origin) return "WRONG_ORIGIN";
  const aud = keyringAud(origin);
  let trusted = [];
  const seen = new Set();
  let lastIat = -Infinity;
  for (let n = 0; n < ring.history.length; n++) {
    const approval = ring.history[n];
    const p = peek(approval);
    if (!p) return "BAD_ENTRY";
    if (p.iat < lastIat) return "BAD_ENTRY";
    lastIat = p.iat;
    const id = get(p.claims, "id");
    if (seen.has(id)) return "BAD_ENTRY";
    seen.add(id);
    const request = get(p.claims, "request");
    const kind = get(request, "type");
    const params = get(request, "params");
    let signers;
    let add = null;
    if (kind === "add_key") {
      const card = cardFrom(params);
      if (!card || !(await keyOk(card.public_key))) return "BAD_ENTRY";
      if (n === 0) {
        if ((await fingerprint(card)) !== rootFingerprint) return "WRONG_ROOT";
        signers = [card];
      } else {
        if (trusted.some((k) => k.credential_id === card.credential_id)) return "BAD_ENTRY";
        signers = trusted.slice();
      }
      add = card;
    } else if (kind === "remove_key" && n > 0) {
      const cred = get(params, "credential_id");
      if (!trusted.some((k) => k.credential_id === cred)) return "BAD_ENTRY";
      if (trusted.length === 1) return "BAD_ENTRY";
      signers = trusted.slice();
    } else {
      return n === 0 ? "WRONG_ROOT" : "BAD_ENTRY";
    }
    const config = { origins: [origin], rp_ids: signers.map((k) => k.rp_id) };
    try {
      await checkKind(approval, { aud, request, level: "high" }, { keys: signers }, config, p.iat, false);
    } catch (e) {
      if (e instanceof Refused) return "BAD_ENTRY";
      throw e;
    }
    if (add) trusted.push(add);
    else {
      const cred = get(params, "credential_id");
      trusted = trusted.filter((k) => k.credential_id !== cred);
    }
  }
  return sameKeys(trusted, ring.keys) ? null : "KEYS_DO_NOT_MATCH";
}

/** A refetched keyring must extend the accepted one, entry for entry. */
export async function verifyHistoryUpdate(accepted, next, rootFingerprint, origin) {
  const prefix = (next.history ?? []).slice(0, accepted.history.length);
  if (prefix.length !== accepted.history.length || prefix.some((h, i) => h !== accepted.history[i])) return "NOT_AN_UPDATE";
  return verifyHistory(next, rootFingerprint, origin);
}
