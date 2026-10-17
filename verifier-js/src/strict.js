// Strict JSON and base64url, written from RFC 8259 and the Phygital Key spec.
// Refuses: duplicate keys, lone surrogates, nesting deeper than 16, trailing data.
// Numbers keep their text, so the canonical step can refuse floats, -0 and huge ints.

export const MAX_DEPTH = 16;

/** A JSON number that is not a safe integer (fraction, exponent, -0, too big). */
export class NotSafeInteger {
  constructor(text) {
    this.text = text;
  }
}

const B64URL = /^[A-Za-z0-9_-]*$/;
const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/** base64url without padding. Returns Uint8Array, or null if not canonical. */
export function b64decode(text) {
  if (typeof text !== "string" || !B64URL.test(text) || text.length % 4 === 1) return null;
  const out = new Uint8Array(Math.floor((text.length * 3) / 4));
  let acc = 0;
  let bits = 0;
  let o = 0;
  for (const ch of text) {
    acc = (acc << 6) | ALPHABET.indexOf(ch);
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[o++] = (acc >> bits) & 0xff;
    }
    acc &= (1 << bits) - 1;
  }
  // Refuse non-canonical encodings (non-zero trailing bits).
  if (acc !== 0) return null;
  return out;
}

export function b64encode(bytes) {
  let out = "";
  let i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    out += ALPHABET[n >> 18] + ALPHABET[(n >> 12) & 63] + ALPHABET[(n >> 6) & 63] + ALPHABET[n & 63];
  }
  if (bytes.length - i === 1) {
    const n = bytes[i] << 16;
    out += ALPHABET[n >> 18] + ALPHABET[(n >> 12) & 63];
  } else if (bytes.length - i === 2) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8);
    out += ALPHABET[n >> 18] + ALPHABET[(n >> 12) & 63] + ALPHABET[(n >> 6) & 63];
  }
  return out;
}

const MAX_SAFE = 9007199254740991n;

/** Parse JSON text strictly. Returns the value, or undefined on any problem. */
export function parse(text) {
  let i = 0;
  let depth = 0;
  const fail = () => {
    throw new SyntaxError("bad json");
  };
  const ws = () => {
    while (i < text.length && (text[i] === " " || text[i] === "\t" || text[i] === "\n" || text[i] === "\r")) i++;
  };
  const value = () => {
    ws();
    const c = text[i];
    if (c === "{") return object();
    if (c === "[") return array();
    if (c === '"') return string();
    if (c === "t" && text.startsWith("true", i)) return (i += 4), true;
    if (c === "f" && text.startsWith("false", i)) return (i += 5), false;
    if (c === "n" && text.startsWith("null", i)) return (i += 4), null;
    if (c === "-" || (c >= "0" && c <= "9")) return number();
    return fail();
  };
  const enter = () => {
    depth++;
    if (depth > MAX_DEPTH) fail();
  };
  const object = () => {
    enter();
    i++;
    const out = new Map();
    ws();
    if (text[i] === "}") return i++, depth--, out;
    for (;;) {
      ws();
      if (text[i] !== '"') fail();
      const key = string();
      if (out.has(key)) fail();
      ws();
      if (text[i] !== ":") fail();
      i++;
      out.set(key, value());
      ws();
      if (text[i] === ",") {
        i++;
        continue;
      }
      if (text[i] === "}") return i++, depth--, out;
      fail();
    }
  };
  const array = () => {
    enter();
    i++;
    const out = [];
    ws();
    if (text[i] === "]") return i++, depth--, out;
    for (;;) {
      out.push(value());
      ws();
      if (text[i] === ",") {
        i++;
        continue;
      }
      if (text[i] === "]") return i++, depth--, out;
      fail();
    }
  };
  const hex4 = () => {
    const h = text.slice(i, i + 4);
    if (!/^[0-9a-fA-F]{4}$/.test(h)) fail();
    i += 4;
    return parseInt(h, 16);
  };
  const string = () => {
    i++;
    let out = "";
    for (;;) {
      if (i >= text.length) fail();
      const c = text[i];
      const code = text.charCodeAt(i);
      if (c === '"') return i++, out;
      if (code < 0x20) fail();
      if (code >= 0xd800 && code <= 0xdfff) {
        // A raw surrogate in the text must be a proper pair.
        const next = text.charCodeAt(i + 1);
        if (code > 0xdbff || !(next >= 0xdc00 && next <= 0xdfff)) fail();
        out += text[i] + text[i + 1];
        i += 2;
        continue;
      }
      if (c !== "\\") {
        out += c;
        i++;
        continue;
      }
      i++;
      const e = text[i++];
      const simple = { '"': '"', "\\": "\\", "/": "/", b: "\b", f: "\f", n: "\n", r: "\r", t: "\t" };
      if (e in simple) {
        out += simple[e];
        continue;
      }
      if (e !== "u") fail();
      const u = hex4();
      if (u >= 0xd800 && u <= 0xdbff) {
        if (text[i] !== "\\" || text[i + 1] !== "u") fail();
        i += 2;
        const low = hex4();
        if (!(low >= 0xdc00 && low <= 0xdfff)) fail();
        out += String.fromCharCode(u, low);
      } else if (u >= 0xdc00 && u <= 0xdfff) {
        fail();
      } else {
        out += String.fromCharCode(u);
      }
    }
  };
  const number = () => {
    const m = /^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?/.exec(text.slice(i));
    if (!m) fail();
    i += m[0].length;
    const t = m[0];
    // Only plain integers within +-(2^53 - 1) are safe; "-0" is refused like a float.
    if (m[2] || m[3] || t === "-0") return new NotSafeInteger(t);
    const big = BigInt(t);
    if (big > MAX_SAFE || big < -MAX_SAFE) return new NotSafeInteger(t);
    return Number(t);
  };
  try {
    const v = value();
    ws();
    if (i !== text.length) return undefined;
    return v;
  } catch {
    return undefined;
  }
}

/** Parse UTF-8 bytes strictly (invalid UTF-8 is refused). */
export function parseBytes(bytes) {
  let text;
  try {
    // ignoreBOM: keep a byte-order mark, so the parser refuses it (one approval, one encoding).
    text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
  } catch {
    return undefined;
  }
  return parse(text);
}

/** Turn plain JSON.parse output (trusted test input, service-built values) into our model. */
export function fromPlain(v) {
  if (Array.isArray(v)) return v.map(fromPlain);
  if (v !== null && typeof v === "object") return new Map(Object.entries(v).map(([k, x]) => [k, fromPlain(x)]));
  if (typeof v === "number" && !Number.isSafeInteger(v)) return new NotSafeInteger(String(v));
  if (Object.is(v, -0)) return new NotSafeInteger("-0");
  return v;
}

/** Turn our model back into plain values (for reading fields). */
export function get(v, ...path) {
  let cur = v;
  for (const p of path) {
    if (cur instanceof Map) cur = cur.get(p);
    else if (Array.isArray(cur) && typeof p === "number") cur = cur[p];
    else return undefined;
  }
  return cur;
}
