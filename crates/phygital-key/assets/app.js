// Phygital Key approval page. Plain JS, no dependencies.
// The page shows the canonical request built by the gate (never AI prose),
// and signs a challenge it computes itself from the exact claims it was given.
"use strict";

const PREFIX = "phygital-key/approval/v1\u0000";
const $ = (id) => document.getElementById(id);

function b64u(bytes) {
  let s = "";
  new Uint8Array(bytes).forEach((b) => { s += String.fromCharCode(b); });
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function unb64u(text) {
  const s = atob(text.replace(/-/g, "+").replace(/_/g, "/"));
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}
async function sha256(bytes) {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
}
function utf8(text) {
  return new TextEncoder().encode(text);
}
function concat(a, b) {
  const out = new Uint8Array(a.length + b.length);
  out.set(a, 0);
  out.set(b, a.length);
  return out;
}
function note(text, kind) {
  const n = $("note");
  if (!n) return;
  n.textContent = text;
  n.className = "note" + (kind ? " " + kind : "");
}
async function api(path, body) {
  const opts = body === undefined
    ? { method: "GET" }
    : { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) };
  const res = await fetch(path, opts);
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || "Something went wrong (" + res.status + ").");
  return data;
}
function webauthnSupported() {
  return window.PublicKeyCredential && navigator.credentials && window.isSecureContext;
}
function friendly(err) {
  if (err && err.name === "NotAllowedError") return "The key prompt was closed or timed out. Nothing was signed.";
  if (err && err.name === "InvalidStateError") return "This key is already added.";
  if (err && err.name === "SecurityError") return "This page address cannot use passkeys. Open it through the exact link you were given.";
  return (err && err.message) || "Something went wrong.";
}
function countdown(el, until, done) {
  const tick = () => {
    const left = until - Math.floor(Date.now() / 1000);
    if (left <= 0) { el.textContent = "time is up"; if (done) done(); return; }
    const m = Math.floor(left / 60), s = left % 60;
    el.textContent = (m ? m + " min " : "") + s + " s";
    setTimeout(tick, 1000);
  };
  tick();
}

// Flatten params into rows: path -> value. Keys that are not plain words are
// quoted, so {"a.b": 1} and {"a": {"b": 1}} never look the same.
function seg(k) {
  return /^[A-Za-z0-9_-]+$/.test(k) ? k : JSON.stringify(k);
}
function rows(value, path, out) {
  if (value !== null && typeof value === "object") {
    const arr = Array.isArray(value);
    const keys = arr ? value.map((_, i) => i) : Object.keys(value);
    if (keys.length === 0) out.push([path || "(empty)", arr ? "[]" : "{}"]);
    keys.forEach((k) => {
      const part = arr ? "[" + k + "]" : seg(String(k));
      rows(value[k], path ? path + (arr ? part : " › " + part) : part, out);
    });
  } else {
    out.push([path, typeof value === "string" ? value : JSON.stringify(value)]);
  }
  return out;
}
// Invisible, direction-changing and filler characters, and mixed scripts,
// can make two different texts look the same. Highlight them.
const INVISIBLE = /[­͏؜ᅟᅠ឴឵᠋-᠏​-‏‪-‮⁠-⁯ㅤ︀-️﻿ﾠ￹-￻]|\uDB40[\uDC00-\uDC7F]|[\u0000-\u0008\u000B-\u001F\u007F-\u009F]/;
function suspicious(text) {
  const latin = /[A-Za-z]/.test(text);
  const otherScript = /[Ͱ-ϿЀ-ԯ԰-֏Ꭰ-᏿＀-￯]/.test(text);
  return INVISIBLE.test(text) || /^\s|\s$|\s\s/.test(text) || (latin && otherScript);
}

async function approvePage() {
  const id = location.pathname.split("/").pop();
  let view;
  try {
    view = await api("/api/r/" + encodeURIComponent(id));
  } catch (e) {
    $("title").textContent = "Request not found";
    $("lead").textContent = "This link is wrong or the request was removed.";
    return;
  }
  const request = JSON.parse(view.request_jcs);
  $("type").textContent = view.first_key ? "Trust this key (your first key)"
    : view.adds_key ? "Add a new key to your trusted keys"
    : view.removes_key ? "Remove a key from your trusted keys"
    : view.standing ? "Standing permission for: " + view.standing.for_type
    : view.revokes ? "Revoke a standing permission"
    : request.type;
  $("aud").textContent = (view.adds_key || view.removes_key) ? "This approval gate (your list of trusted keys)" : view.aud;
  $("level").textContent = view.level === "high" ? "Fingerprint, face or PIN" : "A touch on your key";
  $("raw").textContent = view.request_jcs;
  $("hash").textContent = view.request_hash;
  const body = $("params").querySelector("tbody");
  rows(request.params, "", []).forEach(([k, v]) => {
    const tr = document.createElement("tr");
    const th = document.createElement("th");
    const td = document.createElement("td");
    th.textContent = view.standing && k === "use_key" ? "use_key (technical: signs each use)" : k;
    td.textContent = v;
    if (suspicious(k) || suspicious(v)) {
      tr.classList.add("warn");
      tr.title = "Look closely: contains look-alike, invisible or extra space characters";
    }
    tr.append(th, td);
    body.append(tr);
  });
  if (view.standing) {
    // A plain summary of the limits. The full signed text is still shown below.
    const p = request.params;
    const lines = [];
    const fixed = Object.entries(p.fixed || {}).map(([k, v]) => k + " = " + (typeof v === "string" ? v : JSON.stringify(v)));
    if (fixed.length) lines.push("Always exactly: " + fixed.join(", "));
    Object.entries(p.vary || {}).forEach(([k, lim]) => {
      if (lim && lim.one_of) lines.push("Can change: " + k + ", only one of: " + lim.one_of.join(", "));
      else if (lim) lines.push("Can change: " + k + ", from " + lim.min + " to " + lim.max);
    });
    lines.push("Anything else: refused");
    lines.push("At most " + p.max_uses + " times, until " + new Date(view.standing.until * 1000).toLocaleString());
    const ul = $("summary-list");
    lines.forEach((t) => {
      const li = document.createElement("li");
      li.textContent = t;
      if (suspicious(t)) li.classList.add("warn");
      ul.append(li);
    });
    $("summary").hidden = false;
  }
  $("card").hidden = false;

  const done = {
    approved: ["Approved", "Signed by " + (view.approved_by || "your key") + ". You can close this page."],
    denied: ["Denied", "You said no. Nothing will happen."],
    cancelled: ["Cancelled", "The request was withdrawn."],
    expired: ["Expired", "Nobody decided in time, so the answer is no."],
  };
  const until = view.standing ? new Date(view.standing.until * 1000).toLocaleString() : "";
  if (view.standing && view.status === "approved") {
    $("title").textContent = "Standing permission active";
    $("lead").textContent = "Used " + view.standing.uses + " of " + view.standing.max_uses + " times. Ends " + until + ".";
    if (view.standing.usable) {
      $("actions").hidden = false;
      $("approve").hidden = true;
      $("deny").textContent = "Revoke now";
      $("deny").onclick = async () => {
        try {
          const r = await api("/api/r/" + encodeURIComponent(id) + "/revoke", {});
          // Step 2: sign the revocation so the service stops accepting it too.
          location.href = r.sign_url;
        } catch (e) { note(friendly(e), "bad"); }
      };
    }
    return;
  }
  if (view.status !== "pending") {
    const [t, l] = done[view.status] || ["Closed", ""];
    $("title").textContent = t;
    $("lead").textContent = l;
    if (view.service_reported) {
      note(view.service_reported === "success"
        ? "The service reported: done."
        : "The service reported: it did not complete.", view.service_reported === "success" ? "good" : "bad");
    }
    return;
  }
  $("title").textContent = view.first_key ? "Confirm your key"
    : view.adds_key ? "Approve a new key?"
    : view.removes_key ? "Remove this key?"
    : view.standing ? "Give a standing permission?"
    : view.revokes ? "Tell the service it is revoked?"
    : "Approve this request?";
  $("lead").textContent = view.first_key
    ? "Sign once with the key you just made. This proves it works and starts your signed key list."
    : view.adds_key
    ? "Someone is adding a new key. Approve only if you are adding it yourself, right now."
    : view.removes_key
    ? "This key will no longer be able to approve anything."
    : view.revokes
    ? "Revoked here already: no new uses can be signed. Sign once more so the service also refuses it, even uses handed out in the last minute."
    : view.standing
    ? "The AI may do this on its own, up to " + view.standing.max_uses + " times until " + until +
      ", only within the limits below. You can revoke it at any time."
    : "Check every detail. Your key will sign exactly what is shown here.";
  countdown($("deadline"), view.deadline, () => { $("actions").hidden = true; note("Time is up. The answer is no.", "bad"); });
  if (!webauthnSupported()) {
    note("This browser cannot use passkeys here. Open the link in a recent browser, over https or localhost.", "bad");
    return;
  }
  $("actions").hidden = false;

  $("deny").onclick = async () => {
    try {
      await api("/api/r/" + encodeURIComponent(id) + "/deny", {});
      $("actions").hidden = true;
      $("title").textContent = "Denied";
      $("lead").textContent = "You said no. Nothing will happen.";
      note("");
    } catch (e) { note(friendly(e), "bad"); }
  };

  $("approve").onclick = async () => {
    $("approve").disabled = true;
    $("deny").disabled = true;
    try {
      note("Waiting for your key…");
      const started = await api("/api/r/" + encodeURIComponent(id) + "/start", {});
      const claims = JSON.parse(started.claims_jcs);
      // Sign only if the claims say exactly what this page shows.
      const shownHash = b64u(await sha256(utf8(view.request_jcs)));
      if (claims.aud !== view.aud || claims.level !== view.level || claims.request_hash !== shownHash ||
          claims.request_hash !== view.request_hash || JSON.stringify(claims.request) !== JSON.stringify(request) ||
          claims.id !== view.id || claims.v !== "pka.1" ||
          (view.standing && claims.exp !== view.standing.until)) {
        throw new Error("What would be signed does not match what is shown. Nothing was signed.");
      }
      const challenge = await sha256(concat(utf8(PREFIX), utf8(started.claims_jcs)));
      const cred = await navigator.credentials.get({
        publicKey: {
          challenge,
          rpId: started.rp_id,
          allowCredentials: started.allow.map((c) => ({ type: "public-key", id: unb64u(c) })),
          userVerification: view.level === "high" ? "required" : "preferred",
          timeout: 120000,
        },
      });
      const r = cred.response;
      const result = await api("/api/r/" + encodeURIComponent(id) + "/finish", {
        credential_id: b64u(cred.rawId),
        authenticator_data: b64u(r.authenticatorData),
        client_data_json: b64u(r.clientDataJSON),
        signature: b64u(r.signature),
      });
      $("actions").hidden = true;
      $("title").textContent = "Approved";
      $("lead").textContent = "Signed by " + (result.approved_by || "your key") + ". It works once, within " + (claims.exp - claims.iat) + " seconds.";
      note("Done. You can close this page.", "good");
    } catch (e) {
      note(friendly(e), "bad");
      $("approve").disabled = false;
      $("deny").disabled = false;
    }
  };
}

async function enrollPage() {
  const token = new URLSearchParams(location.hash.slice(1)).get("token") || "";
  history.replaceState(null, "", location.pathname); // keep the token out of history
  if (!token) {
    note("This page needs the link printed by `phygital-key enroll`.", "bad");
    $("enroll").disabled = true;
    return;
  }
  if (!webauthnSupported()) {
    note("This browser cannot use passkeys here. Use a recent browser, over https or localhost.", "bad");
    $("enroll").disabled = true;
    return;
  }
  $("enroll").onclick = async () => {
    $("enroll").disabled = true;
    try {
      const name = $("name").value.trim() || "My key";
      const o = await api("/api/enroll/begin", { token, name });
      note("Follow your browser's prompt…");
      const cred = await navigator.credentials.create({
        publicKey: {
          challenge: unb64u(o.challenge),
          rp: { id: o.rp_id, name: "Phygital Key" },
          user: { id: unb64u(o.user_id), name: o.name, displayName: o.name },
          pubKeyCredParams: [{ type: "public-key", alg: -7 }],
          excludeCredentials: o.exclude.map((c) => ({ type: "public-key", id: unb64u(c) })),
          authenticatorSelection: { residentKey: "preferred", userVerification: "preferred" },
          attestation: "none",
          timeout: 120000,
        },
      });
      const r = cred.response;
      if (typeof r.getPublicKey !== "function" || r.getPublicKeyAlgorithm() !== -7 || !r.getPublicKey()) {
        throw new Error("This key type is not supported yet. Use a passkey or a FIDO2 key with ES256.");
      }
      const result = await api("/api/enroll/finish", {
        token,
        credential_id: b64u(cred.rawId),
        public_key: b64u(r.getPublicKey()),
        client_data_json: b64u(r.clientDataJSON),
      });
      note(result.first_key
        ? "Almost done: confirm with the key you just made."
        : "Almost done: approve this new key with a key you already trust.", "good");
      location.href = result.approve_url;
    } catch (e) {
      note(friendly(e), "bad");
      $("enroll").disabled = false;
    }
  };
}

async function indexPage() {
  try {
    const s = await api("/api/status");
    const list = $("keys");
    list.textContent = "";
    if (s.keys.length === 0) {
      const li = document.createElement("li");
      li.textContent = "No keys yet.";
      list.append(li);
    }
    s.keys.forEach((k) => {
      const li = document.createElement("li");
      const label = document.createElement("span");
      label.textContent = k.name + " · added " + k.created;
      li.append(label);
      if (s.keys.length > 1) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "link";
        btn.textContent = "Remove";
        btn.onclick = async () => {
          try {
            const r = await api("/api/keys/remove", { credential_id: k.credential_id });
            location.href = r.approve_url;
          } catch (e) { note(friendly(e), "bad"); }
        };
        li.append(" ", btn);
      }
      list.append(li);
    });
    if (s.root_fingerprint) {
      $("root").textContent = s.root_fingerprint;
      $("signed").textContent = s.history_ok
        ? "Every key change is signed. Share this fingerprint so others can check your key list."
        : "Warning: the key history does not check out.";
      $("rootbox").hidden = false;
    }
  } catch (e) { note(friendly(e), "bad"); }
}

const page = document.body.dataset.page;
if (page === "approve") approvePage();
else if (page === "enroll") enrollPage();
else indexPage();
