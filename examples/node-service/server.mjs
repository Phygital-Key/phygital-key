// A minimal deploy service that acts only with a person's valid approval.
// Plain Node (node:http), no dependencies. It checks every approval itself
// with the independent JavaScript checker in this repo, then "deploys"
// (nothing real happens).
//
// Run:  KEYRING=~/.phygital-key-mcp/keyring.json node server.mjs
// Env:  KEYRING      keyring file (required), from `phygital-key-mcp keys` or the gate's data folder
//       PORT         port to listen on (default 7600)
//       AUD          this service's audience id (default http://localhost:<PORT>)
//       GATE_ORIGIN  the approval page origin (default http://localhost:7468)

import http from "node:http";
import { readFileSync } from "node:fs";
import { verify, MemorySeenStore, keyringFrom } from "../../verifier-js/src/index.js";

const PORT = Number(process.env.PORT || 7600);
const AUD = process.env.AUD || `http://localhost:${PORT}`;
const GATE_ORIGIN = process.env.GATE_ORIGIN || "http://localhost:7468";
const KEYRING = process.env.KEYRING;
if (!KEYRING) {
  console.error("Set KEYRING to the keyring file, e.g. KEYRING=~/.phygital-key-mcp/keyring.json");
  process.exit(2);
}

// Where approvals may come from: the gate's page origin and its host name.
const config = { origins: [GATE_ORIGIN], rp_ids: [new URL(GATE_ORIGIN).hostname] };
// Used approval ids. One process only: use a shared, durable store with several instances.
const seen = new MemorySeenStore();

function send(res, status, body) {
  res.writeHead(status, { "Content-Type": "application/json" });
  res.end(JSON.stringify(body) + "\n");
}

function readBody(req, limit = 64 * 1024) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const parts = [];
    req.on("data", (c) => {
      size += c.length;
      if (size > limit) reject(new Error("too big"));
      else parts.push(c);
    });
    req.on("end", () => resolve(Buffer.concat(parts).toString("utf8")));
    req.on("error", reject);
  });
}

// The request this service is about to run, built from its own fields only.
// Anything else in the body is refused, so nothing unchecked rides along.
function deployRequest(body) {
  const fields = ["service", "env", "version"];
  if (!body || typeof body !== "object" || Array.isArray(body)) return null;
  if (Object.keys(body).length !== fields.length) return null;
  if (!fields.every((f) => typeof body[f] === "string" && body[f] !== "")) return null;
  return { type: "deploy", params: { service: body.service, env: body.env, version: body.version } };
}

const server = http.createServer(async (req, res) => {
  if (req.method === "GET" && req.url === "/") {
    return send(res, 200, { service: "example deploy service (nothing real happens)", aud: AUD, actions: { "POST /deploy": "level high, body {service, env, version}, header Phygital-Approval" } });
  }
  if (req.method !== "POST" || req.url !== "/deploy") return send(res, 404, { ok: false, error: "NOT_FOUND" });

  let body;
  try {
    body = JSON.parse(await readBody(req));
  } catch {
    return send(res, 400, { ok: false, error: "BAD_BODY" });
  }
  const request = deployRequest(body);
  if (!request) return send(res, 400, { ok: false, error: "BAD_BODY" });

  // Read the keyring on every call, so a changed file takes effect at once.
  // A missing or broken file means no trusted keys: everything is refused.
  let keyring;
  try {
    keyring = keyringFrom(JSON.parse(readFileSync(KEYRING, "utf8")));
  } catch {
    keyring = keyringFrom({ keys: [] });
  }

  const approval = req.headers["phygital-approval"] || "";
  // This service decides the level: every deploy needs fingerprint, face or PIN.
  const result = await verify(approval, { aud: AUD, request, level: "high" }, keyring, seen, config, Math.floor(Date.now() / 1000));
  if (!result.ok) {
    console.log(`refused ${request.params.service} ${request.params.version}: ${result.error}`);
    return send(res, 403, { ok: false, error: result.error });
  }

  const { id, approver, request_hash } = result.verified;
  // Log what the spec asks for: id, request hash, approver, result.
  console.log(`deploy ${request.params.service} ${request.params.version} to ${request.params.env}: approved by ${approver} (id ${id}, request_hash ${request_hash})`);
  // ...a real service deploys here.
  return send(res, 200, { ok: true, done: `Deployed ${request.params.service} ${request.params.version} to ${request.params.env} (example: nothing real happened)`, approval_id: id, approved_by: approver });
});

// 127.0.0.1 only: put an HTTPS proxy in front to reach it from elsewhere.
server.listen(PORT, "127.0.0.1", () => {
  console.log(`example deploy service on http://localhost:${PORT}, aud ${AUD}, trusting approvals from ${GATE_ORIGIN}`);
});
