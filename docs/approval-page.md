# The approval page

The approval page is where a person reads a request and decides. This page explains how it is built, what it shows, how it marks suspicious text, and what it compares before it lets the key sign. It is for reviewers and contributors. The code is `crates/phygital-key/assets/approve.html` and `app.js` (about 410 lines of plain JavaScript, no dependencies), served by the gate from `web.rs`.

## On this page

- [Built from the canonical request](#built-from-the-canonical-request)
- [What it shows](#what-it-shows)
- [Suspicious characters](#suspicious-characters)
- [Sign only what is shown](#sign-only-what-is-shown)
- [After the decision](#after-the-decision)
- [No third-party resources](#no-third-party-resources)
- [What the page cannot protect against](#what-the-page-cannot-protect-against)

## Built from the canonical request

The agent never writes anything the person reads on this page. The page fetches `/api/r/{id}` from the gate and gets:

| Field | Source |
|---|---|
| `request_jcs` | The [canonical form](canonical-form.md) of the request, built by the gate |
| `request_hash` | base64url SHA-256 of `request_jcs` |
| `id`, `aud`, `level`, `deadline` | Stored by the gate when the request was made |
| `status`, `approved_by`, `exp` | Where the request stands, and who signed |
| `standing` | For a standing approval: `for_type`, `max_uses`, `uses`, `until`, `usable` |
| `adds_key`, `first_key`, `removes_key`, `revokes` | What kind of request this is |
| `revocation_signed` | For a revoked standing approval: whether the person's revocation is signed |
| `service_reported` | `success` or `error`, set only from a receipt signed by the service's pinned key |

There is no free-text "reason" or "summary" field an agent could fill. Every text is put on the page with `textContent`, never as HTML, so text inside a request cannot become markup or script. The end-to-end suite checks that "page shows the injected text as plain data".

<img src="images/approve-dark-phone.png" alt="Approval page for a deploy: title Approve this request, carried out by http://localhost:7469, proof needed fingerprint, face or PIN, decide within 10 min 0 s, and every detail: env production, service api, version 2.4.1" width="320">

## What it shows

| Part | Content |
|---|---|
| Title | `Approve this request?`, or the special titles for a first key, a new key, a key removal, a standing permission or a revocation |
| Card title | The request `type` (or "Standing permission for: payment", "Add a new key to your trusted keys", and so on) |
| Carried out by | The audience (`aud`). For key changes: "This approval gate (your list of trusted keys)". |
| Proof needed | `high`: "Fingerprint, face or PIN". `medium`: "A touch on your key". |
| Decide within | A live countdown to the 10-minute deadline. At zero the buttons hide and the page says "Time is up. The answer is no." |
| In short | Standing approvals only: "Always exactly: to = ACME Ltd, currency = EUR", "Can change: amount_cents, from 1 to 2000" (or "Can change: memo, only one of: coffee, lunch"), "Anything else: refused", "At most 5 times, until ...". |
| Every detail | One row per value in `params`, nothing truncated |
| Exact text you are signing | A folded section with `request_jcs` as is, and its SHA-256 (`request_hash`), labelled "Fingerprint (SHA-256)" |

Every value in `params` becomes a row. Nested objects are flattened into paths joined with ` › `, array items are written `[0]`, `[1]`, and an empty object or array shows as `{}` or `[]`. Strings are shown as they are; numbers, booleans and `null` are shown as JSON.

### Quoting keys

A key is shown bare only when it matches `^[A-Za-z0-9_-]+$`. Any other key is shown as a JSON string, with quotes. So `{"a.b": 1}` shows as `"a.b"` and `{"a": {"b": 1}}` shows as `a › b`, and the two never look the same. A key with a space, a dot, a look-alike letter or an invisible character is always quoted.

## Suspicious characters

The page marks a row (and a line of the standing summary) when its path or its value is suspicious. A marked row gets a coral background and the label "Check this: look-alike, invisible or extra-space characters". A marked summary line gets the coral background only.

<img src="images/check-this-dark.png" alt="Approval page for a payment where the rows note (paypal) and reference (Invoice 118) are marked with Check this: look-alike, invisible or extra-space characters" width="320">

`suspicious(text)` is true when any of these hold:

| Rule | Exact test in `app.js` |
|---|---|
| Invisible or format character | `INVISIBLE` matches (table below) |
| Odd spacing | `/^\s\|\s$\|\s\s/`: leading whitespace, trailing whitespace, or two whitespace characters in a row |
| Mixed scripts | The text has a Latin letter (`/[A-Za-z]/`) and a character from one of the ranges below |

`INVISIBLE`:

| Code points | What they are |
|---|---|
| U+00AD | Soft hyphen |
| U+034F | Combining grapheme joiner |
| U+061C | Arabic letter mark |
| U+115F, U+1160 | Hangul choseong and jungseong fillers |
| U+17B4, U+17B5 | Khmer inherent vowels |
| U+180B to U+180F | Mongolian free variation selectors and vowel separator |
| U+200B to U+200F | Zero-width space, non-joiner, joiner, left-to-right mark, right-to-left mark |
| U+202A to U+202E | Direction embeddings and overrides |
| U+2060 to U+206F | Word joiner, invisible operators, direction isolates, deprecated format characters |
| U+3164 | Hangul filler |
| U+FE00 to U+FE0F | Variation selectors |
| U+FEFF | Zero-width no-break space (byte-order mark) |
| U+FFA0 | Halfwidth Hangul filler |
| U+FFF9 to U+FFFB | Interlinear annotation characters |
| U+E0000 to U+E007F | Tag characters (matched as `\uDB40[\uDC00-\uDC7F]`) |
| U+0000 to U+0008, U+000B to U+001F, U+007F to U+009F | Control characters (tab and line feed are not flagged) |

Other scripts counted for "mixed scripts" (`otherScript`):

| Code points | Block |
|---|---|
| U+0370 to U+03FF | Greek and Coptic |
| U+0400 to U+052F | Cyrillic and Cyrillic Supplement |
| U+0530 to U+058F | Armenian |
| U+13A0 to U+13FF | Cherokee |
| U+FF00 to U+FFEF | Halfwidth and Fullwidth Forms |

> [!NOTE]
> The highlighting is a help, not a guarantee. Plenty of look-alikes fall outside these ranges. The protection is that the exact bytes are signed and the service compares the bytes it is about to run: a look-alike is a different request (`REQUEST_MISMATCH`), and text that is not NFC is refused outright.

## Sign only what is shown

When the person presses **Approve with my key**, the page calls `/api/r/{id}/start`. The gate fixes the claims and returns their canonical text, `claims_jcs`. Before the key is asked for anything, the page parses those claims and compares them with what it showed. Any difference stops it with "What would be signed does not match what is shown. Nothing was signed."

| Comparison in `app.js` | What it stops |
|---|---|
| `claims.aud === view.aud` | Signing for another service than the one shown |
| `claims.level === view.level` | Signing at another level than the one shown |
| `claims.request_hash === b64u(SHA-256(view.request_jcs))` | A hash that does not belong to the text shown (computed in the browser with WebCrypto) |
| `claims.request_hash === view.request_hash` | A hash other than the one shown under "Exact text you are signing" |
| `JSON.stringify(claims.request) === JSON.stringify(request)` | A request other than the one shown, where `request` is `JSON.parse(view.request_jcs)` |
| `claims.id === view.id` | Claims for another request id |
| `claims.v === "pka.1"` | Another format version |
| `claims.exp === view.standing.until` (standing only) | A standing approval that ends at another time than the one shown |

The gate itself keeps the claims fixed while they are valid: a second `start` returns the same claims, so another program cannot swap them while the person is signing (end-to-end check "claims cannot be swapped while the person signs").

### Computing the challenge

The page does not take a challenge from the gate. It computes it from the exact `claims_jcs` text it was given:

$$\text{challenge} = \mathrm{SHA256}\big(\texttt{"phygital-key/approval/v1"} \,\|\, \texttt{0x00} \,\|\, \mathrm{UTF8}(\texttt{claims\_jcs})\big)$$

```js
const challenge = await sha256(concat(utf8(PREFIX), utf8(started.claims_jcs)));
```

Then it calls `navigator.credentials.get` with that challenge, the gate's RP ID, the allowed credential ids (only the new key itself for a first key), `userVerification: "required"` for `high` (`"preferred"` for `medium`) and a 120-second timeout. It sends the four base64url fields of the assertion to `/api/r/{id}/finish`, where the gate runs every verifier check before it stores anything (see [architecture](architecture.md#how-finish-checks-before-it-stores)).

If the browser has no WebAuthn, or the page is not in a secure context (HTTPS or localhost), the page says so and shows no buttons.

## After the decision

The page reads the request again and shows where it stands. These texts are from a run of v0.3.0 with a virtual passkey (dates depend on the browser's locale):

| State | Stamp | Title | Lead |
|---|---|---|---|
| One-time request, approved | `✓ Signed` | Approved | "Signed by Test laptop. It works once, within 60 seconds." |
| Standing approval, approved | `✓ Signed` | Approved | "Signed by Test laptop. Active until 10/24/2026, 3:13:51 PM, at most 5 uses." |
| Standing approval, in use | `✓ Signed · active` | Standing permission active | "Used 1 of 5 times. Ends ..." and a **Revoke now** button |
| Revoked, revocation not signed yet | `✕ Revoked` | Revoked here | "No new uses can be signed. Sign the revocation so the service refuses this permission too." and a **Sign the revocation** button |
| Revoked, revocation signed | `✕ Revoked` | Revoked | "Revoked and signed. No new uses can be signed, and the service refuses this permission as soon as it reads the revocation list." |
| Denied | `✕ Refused` | Denied | "You said no. Nothing will happen." |
| Nobody decided in time | `✕ Expired` | Expired | "Nobody decided in time, so the answer is no." |
| Signed, but not used before `exp` | `✕ Expired` | Expired | "Signed by <name>, but its time ran out. It no longer works anywhere." |

When a receipt signed by the service's pinned key was recorded, the page adds "The service reported: done." (or "The service reported: it did not complete."), and a used one-time approval shows the stamp `● Used once`. The end-to-end suite checks the standing end date and use cap, and the revoked page.

## No third-party resources

The page loads nothing from any other site. The gate sends this Content Security Policy on every page response:

```text
default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
```

| Directive | Effect |
|---|---|
| `default-src 'none'` | Anything not listed is blocked |
| `script-src 'self'` | Only `/assets/app.js` from the gate. No inline scripts. |
| `style-src 'self'`, `font-src 'self'` | Only the gate's own stylesheets and fonts |
| `connect-src 'self'` | `fetch` can reach only the gate |
| `img-src 'self' data:` | Only the gate's images |
| `base-uri 'none'`, `form-action 'none'` | No base-URL tricks, no form posts anywhere |
| `frame-ancestors 'none'` | No site can frame the page (with `X-Frame-Options: DENY`) |

The fonts (Anybody, Hanken Grotesk, JetBrains Mono, under the SIL Open Font License 1.1) and the logos are compiled into the binary and served from `/assets/`. A third-party script or font host could change what the page shows or learn when someone approves; with this policy there is none. Responses also carry `Cache-Control: no-store`, `Referrer-Policy: no-referrer` and `X-Content-Type-Options: nosniff`.

## What the page cannot protect against

> [!WARNING]
> The page is served by the gate. Whoever controls the machine that runs the gate controls what the page shows, and could show one request while asking the key to sign another. Authenticators do not display what they sign. Run the gate where the agent cannot change it (the secure install, or another machine).

The page also cannot protect against a fully compromised browser or device of the person. See the [threat model](threat-model.md) and [secure install](secure-install.md).

## Next

- [Approving](approving.md): the page from the person's side
- [Canonical form](canonical-form.md): where `request_jcs` comes from
- [WebAuthn](webauthn.md): what the key signs and how it is checked
- [Threat model](threat-model.md): who is trusted with what
