# How Phygital Key maps to AP2 and Verifiable Intent

Phygital Key does the same job as AP2's "closed mandate" (a person's signed yes for one exact thing), but for any action, and the person's own passkey signs it. This page is for people who know AP2 or Verifiable Intent: it shows how the pieces line up, where they differ, and what an AP2 export would look like. There is no export code yet, on purpose ([decision 28](decisions.md)): it gets built when someone needs it.

Sources (checked 2026-10-17): the AP2 specification and schemas (`google-agentic-commerce/AP2`, v0.2.0), RFC 9901 (SD-JWT), draft-gco-oauth-delegate-sd-jwt-00, Mastercard Verifiable Intent (`agent-intent/verifiable-intent`, draft v0.1), and the FIDO Alliance announcement of 2026-04-28.

## Pieces side by side

| Phygital Key | AP2 v0.2 | Verifiable Intent v0.1 |
|---|---|---|
| One-time approval (`pka.1`) | Closed mandate (`mandate.checkout.1`, `mandate.payment.1`) | L2 user-signed mandate, immediate mode |
| Standing approval (type `standing`, `use_key`) | Open mandate (`mandate.*.open.1`) with `constraints` and `cnf` | L2 autonomous mode with agent `cnf.jwk` |
| Use token (`pka-use+jwt`, signed by `use_key`) | Agent-signed closed mandate inside the open one (KB-SD-JWT) | L3 agent-signed mandates |
| `fixed` / `vary` limits (`max`, `min`, `one_of`) | Constraint types (`payment.amount_range`, `payment.allowed_payees`, ...) | Same constraints, `mandate.` prefix |
| Approval page (no LLM) | Trusted Surface (must be non-agentic) | Wallet / issuer UI |
| Receipt (`pka-receipt+jwt`, `reference` = hash of the approval) | Receipt JWT (`reference` = hash of the final SD-JWT) | (not specified) |
| Key history, `.well-known/phygital-key.json` | (out of scope) | Issuer keys via the credential |
| The person's passkey signs (WebAuthn assertion) | A wallet holder key, or a Trusted Agent Provider key, signs | A separate device key signs (not WebAuthn) |

## The one real difference

AP2 and Verifiable Intent never put a WebAuthn assertion in a mandate. A passkey signs `authenticatorData || SHA-256(clientDataJSON)`, which is not a JWS signing input, so it cannot be a normal JWS signature, and no standard carries it as one. AP2 uses a wallet or provider JWS key instead, and Verifiable Intent uses its own device keys.

Phygital Key keeps the passkey as the signer. That is its point: no extra key to protect, and the yes comes from the person's own authenticator.

## What an AP2 export would look like (not built)

If an AP2 tool needs to read our approvals, the gate would act as a Trusted Agent Provider:

1. Build a standard SD-JWT payload with `vct` (for example `org.phygitalkey.mandate.action.1`), `_sd_alg: sha-256`, the request fields as separate disclosures, `aud`, `iat`, `exp`, and `cnf` for standing approvals.
2. Make the passkey sign that exact payload (the challenge becomes the SHA-256 of the payload text), so every disclosure still traces back to the person.
3. Sign the JWS with a gate key (`typ` like `pka+sd-jwt`), carrying the passkey assertion in a protected header.

**Gained:** AP2 and SD-JWT tools can read it; selective disclosure; native `cnf`.
**Lost:** for a plain AP2 verifier, trust moves from the person's passkey to the gate's key, which becomes a valuable target. Our own verifier would still check the passkey.

So the export would be opt-in, off by default, and labeled "signed by this gate". Real payments would still need AP2's own `mandate.payment.1` fields.

## Rules we already follow from AP2 and RFC 9901

- Unknown constraint types fail closed: our `vary` accepts only integer ranges `{"min", "max"}` (both required) and `one_of` lists of 1 to 20 strings, and refuses anything else.
- Amounts are integers in minor units (AP2 `payment_amount.amount`); we refuse floats everywhere.
- The Trusted Surface is plain code, never an LLM.
- Receipts point at what they answer by hash.
- For a future SD-JWT encoding: refuse duplicate digests, disclosures nobody references, disclosures named `_sd` or `...`, and `alg: none` (RFC 9901), and check the encoding against an independent implementation.

## Known gaps in the sources

- AP2 writes the hop type as `kb-sd-jwt+kb` in the draft and `kb+sd-jwt+kb` in its SDK.
- AP2's receipt field is `result` in one place and `status` in its schemas.
- Verifiable Intent prefixes constraint names with `mandate.`; AP2 does not.
- The FIDO Agentic Authentication and Payments working groups had published no drafts as of 2026-10-17.

## Next

- [Standing approvals](standing-approvals.md)
- [Receipts](receipts.md)
- [Decisions](decisions.md)
- [Research notes](research.md)
