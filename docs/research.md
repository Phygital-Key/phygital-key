# Phygital Key MCP: research notes

Written 2026-10-17. I read docs and READMEs, not every source file. The CoSig GitHub repo returned a 404, so CoSig facts come from its PyPI page.

## 1. The idea

**Name:** Phygital Key = physical touch + digital rules.

**One line:** an open-source approval gate. Before an AI does something risky (deploy code, delete data, send money), a human approves that exact action with a passkey, fingerprint or hardware key. The service that runs the action verifies the signed approval itself.

**How it started:** a YubiKey as an MCP. A YubiKey exists to prove a human is present, so letting an AI use it directly (auto-login, reading codes) defeats the point. The better idea is to use the key as the approval step for AI actions.

**Use cases**
- Approval gate for AI actions: deploy, delete, send, pay.
- Developers: signing commits or releases with a touch.
- Teams and IT: key inventory and key checks.
- Not good: auto-login, or reading codes for the AI to type in.

## 2. Background: YubiKey open source

- Yubico publishes: `yubikey-manager` (`ykman`), `python-fido2`, `yubico-piv-tool`, Yubico Authenticator.
- YubiKey firmware is not open source. We build compatible, not a clone.
- Open hardware keys exist: OpenSK, SoloKeys, Nitrokey.
- A plain `ykman` MCP (device info, list accounts) is weak: you can already do that on the command line.

## 3. Ways a human can say "yes" (approver options)

| Option | Notes |
|---|---|
| Phone passkey (fingerprint or face) | Free, strongest of the easy options |
| Laptop Touch ID or Windows Hello | Free, nothing to buy |
| Hardware key (YubiKey, SoloKey, Nitrokey) | Strong, needs a touch (and PIN for user verification) |
| Authenticator app code (TOTP) | Easy, but phishable |
| Phone push "Approve/Deny" | Simple, but people tap without reading |
| Telegram or WhatsApp bot | Quick to build, weaker proof |
| Typed confirmation ("SEND 500") | Weakest. Only for testing |
| Software-only passkey | Secret lives on the computer, malware can copy it |

The build is the same for all of them. Only the approve step changes. Real use: WebAuthn (passkey, Touch ID, Windows Hello, security key).

## 4. How a service can verify an approval

1. **Signed approval (chosen):** the human's device signs the exact action. The service checks the signature, action, audience, expiry and one-time ID locally, with no call to anyone.
2. **Ask the Phygital server:** the service asks "is approval 123 valid for this action?" Simple, but needs an always-on server.
3. **Service runs the WebAuthn check itself:** strongest, but the service must run the whole flow.

**Services we don't control** (a bank, say) will not check our approval. The gate then only works if our own tool sits in front and holds the credentials, so the agent can only reach the service through that tool (the "vault" pattern from SynAuth).

## 5. Landscape (what already exists)

| Project | License | What it does | Take from it | Gap |
|---|---|---|---|---|
| notary-mcp | MIT | Human approves a hash of the exact action. Canonical JSON (RFC 8785), single use, idempotency, 56-check attack suite | Canonical hash, attack tests | One demo action, no hardware key or WebAuthn |
| approval-gate-mcp | MIT | Fail-closed gate, human decides in own terminal, TTL, audit log | Out-of-band decisions, fail closed | No signatures, state in plain files |
| CoSig | BSD-3-Clause (alpha) | WebAuthn tap (YubiKey, Touch ID, Windows Hello) per tool call, one-line decorator, replay counter, hash-chained audit log | WebAuthn flow, decorator idea, audit chain | Needs a CoSig Cloud server (self-host or paid), alpha |
| SynAuth MCP | MIT (backend not checked) | Face ID approval on iPhone, risk rules, spending limits, vault execution | Rules engine, vault pattern | Needs their iOS app and backend |
| iVALT agent-auth | MIT | Phone push with biometrics, PKI attestation | Async request flow | Its own README says the detached signature is usually empty, so the proof is weak. Uses iVALT's service |
| Kontrol Freek | MIT | Risk scoring, contradiction checks, Telegram and Slack approval, HMAC hash-chained audit | Multi-channel approval, audit | Not hardware-key based |
| linkedin-mcp | not checked | Two-step writes (propose then execute), elicitation approval, refuses on clients without elicitation | Propose/execute split, fail closed on weak clients | LinkedIn only |
| AP2 | Apache-2.0 | Signed "mandates" for agent payments | See section 6 | Payments only, no generic actions |

Also seen: SecureAuth (CIBA approval, enterprise), Rayrun and IdentArk gateways, and several small Telegram/Slack approval MCPs.

## 6. AP2 (Agent Payments Protocol)

- Google announced it in September 2025. Donated to the FIDO Alliance on 2026-04-28 (Payments Technical Working Group). v0.2 published then. Repo: Apache-2.0.
- **Not A2A.** A2A is agent-to-agent messaging. MCP is agent-to-tool. AP2 sits on top of them for payments and works with UCP (Universal Commerce Protocol).
- **Roles:** Shopping Agent, Credential Provider, Merchant, Merchant Payment Processor, Trusted Surface.
- **Trusted Surface** must be non-agentic: plain code, no LLM. It shows the action to the human and gets signed consent. This is our approval page.
- **Mandates** (SD-JWT with key binding):
  - Checkout Mandate (`mandate.checkout.1`): what is bought. Bound to the merchant-signed checkout by hash.
  - Payment Mandate (`mandate.payment.1`): the payment. Bound to the same checkout by hash.
  - Receipts come back from the verifier.
- **Human present:** the user approves the closed mandates directly.
- **Human not present:** the user approves **open** mandates (constraints, agent public key as `cnf`, short `exp`). The agent signs closed mandates within them. An agent must not present a new open mandate without a rejection receipt for the last one.
- **Verification rules** run in plain code at each role.
- **Extension points:** new constraint types (unique `type`, schema, evaluation algorithm), checkout object, payment instrument, VDC formats.
- **Notes:** the checkout JWT must use a randomized signature scheme (ECDSA), not deterministic Ed25519. Python SDK lives in `code/sdk/python/ap2`; PyPI package not published yet. Go and Android samples exist.
- **Gap for us:** built around checkout and payment. No generic "action mandate". No tiny verifier library for any service. Mandate selection, agent-to-agent delegation and mandate retrieval are out of scope in v0.2.

## 7. MCP facts that matter

- **Elicitation** lets a server ask the user for input mid-call.
  - Form mode: data goes through the client. Never for passwords, keys or payment credentials.
  - URL mode (spec 2025-11-25): sends the user to a page outside the client. Data is not exposed to the client or the LLM. Meant for sensitive steps. This fits our approval.
  - "Accept" in URL mode only means the user agreed to open the URL. Completion is out of band. Servers may send `notifications/elicitation/complete`.
  - URL rules: no secrets in the URL, no pre-authenticated URLs, HTTPS outside development, server must verify the person who opens the URL is the same user who started the request.
  - Clients must show the full URL and get consent before opening it.
- **Tool annotations** (`destructiveHint`, `readOnlyHint`) are advice only, not a security boundary.
- Some servers run a tool without confirmation when the client lacks elicitation. Our gate must fail closed.
- **MCP authorization:** OAuth 2.1 with PKCE between client and server. Separate from our approval.
- **SDKs:** Python `mcp` (FastMCP), TypeScript, and the official Rust SDK `rmcp` (tokio; features for server, stdio, streamable HTTP, OAuth, elicitation). I did not check whether `rmcp` supports URL mode.

## 8. Design decisions so far

- Build on AP2 ideas and WebAuthn. No new approval format from scratch.
- v0.1: one human-present approval per action. Standing approvals (AP2 open mandates) and SD-JWT encoding are v0.2.
- The human's device signs a hash of the exact action. Service verifies offline.
- Approval page = Trusted Surface. Gate and approver contain no LLM.
- URL-mode elicitation for the approval step, with a safe fallback link.
- Fail closed. Single use. 60 s default life, 300 s max.
- Self-hosted by default. Apache-2.0.
- Test vectors shared by all language versions. Two or more independent implementations.

## 9. Language options

- All of Python, Go, TypeScript and Rust are memory-safe. The real risk is logic mistakes (a skipped check), not memory bugs. The real safety comes from audited libraries and shared test vectors.
- Decision: Rust for the core (verifier and gate), on the official `rmcp` SDK. One checked implementation, a single static binary that runs anywhere, and later wrappers for Python and JavaScript (see `docs/decisions.md`).
- Candidate libraries (all unverified, check existence, license, maintenance and last release before using): Python `cryptography`, `webauthn`, `rfc8785`, `mcp`; TypeScript `@simplewebauthn/server`, `canonicalize`, `jose`; Rust `webauthn-rs`, `p256`, `serde_jcs` or similar, `rmcp`.

## 10. Where we can win

1. Generic action approvals (not just payments), as an AP2 extension.
2. A tiny verifier library any service can add in about 5 lines.
3. Real signatures made on the user's device (iVALT's is weak).
4. Fully self-hosted, no cloud server (CoSig needs one).
5. Fail closed, with the attack test suite copied from notary-mcp.
6. Optional later: a gate that wraps other MCP servers' tools.

## 11. Risks

- **Adoption:** services won't add a verifier until people use it, and people won't use it until services do. Start where one team controls both ends (internal deploy tools), then grow.
- **Big players:** Google and the FIDO Alliance push AP2. A new format would lose. Stay compatible.
- **Trust:** "secure" is earned. Use audited libraries, publish a threat model, label the project pre-audit until an outside review.

## 12. Launch plan

1. Open spec.
2. Free client MCP (the Gate).
3. Verifier libraries (Python, JavaScript, later Rust).
4. Later, a hosted option for companies (logs, team rules).

## 13. Links

- AP2 spec: https://ap2-protocol.org/ap2/specification/
- AP2 flows: https://ap2-protocol.org/ap2/flows/
- AP2 checkout mandate: https://ap2-protocol.org/ap2/checkout_mandate/
- AP2 payment mandate: https://ap2-protocol.org/ap2/payment_mandate/
- AP2 repo: https://github.com/google-agentic-commerce/AP2
- FIDO and AP2: https://ppc.land/fido-alliance-forms-working-groups-to-lock-down-ai-agent-payments/
- UCP AP2 extension: https://ucp.dev/specification/payment/extensions/ap2-mandates/
- MCP elicitation spec: https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation
- MongoDB MCP security notes (annotations are advisory): https://www.mongodb.com/docs/mcp-server/remote-mcp/security/
- Rust SDK docs: https://docs.rs/crate/rmcp
- notary-mcp: https://github.com/salmanneedsajob/notary-mcp
- approval-gate-mcp: https://github.com/itcustomsolution/approval-gate-mcp
- CoSig: https://pypi.org/project/cosig/
- SynAuth MCP: https://pypi.org/project/synauth-mcp/
- iVALT agent-auth: https://github.com/iVALT-Inc/agent-auth
- Kontrol Freek: https://pypi.org/project/kontrol-freek-mcp/
- linkedin-mcp: https://glama.ai/mcp/servers/PeterEkwere/linkedin-mcp
- A2A vs MCP: https://vercel.com/i/a2a-vs-mcp
