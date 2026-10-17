# Canonical form

A signature covers bytes, not ideas. This page gives the exact rules that turn a request (and the claims around it) into one byte string, so the gate, the approval page and every [verifier](glossary.md#verifier) hash the same bytes. It is for implementers and reviewers. The code is `crates/phygital-key-verify/src/canonical.rs` and `strict.rs`; the JavaScript verifier has its own copy in `verifier-js/src/canonical.js` and `strict.js`.

## On this page

- [Why a canonical form](#why-a-canonical-form)
- [The rules](#the-rules)
- [Worked example](#worked-example)
- [The challenge](#the-challenge)
- [base64url](#base64url)
- [Inputs that are refused](#inputs-that-are-refused)

## Why a canonical form

The same JSON value can be written many ways: `{"a":1,"b":2}`, `{ "b": 2, "a": 1 }`, `{"a":1.0,"b":2}`. If the person's key signed one spelling and the service hashed another, a correct approval would fail, or worse, two different values could share one approval. So Phygital Key fixes one spelling:

$$\mathrm{canonical}(x) = \mathrm{JCS}(x) \quad \text{if } x \text{ passes the rules below, otherwise refused}$$

JCS is the JSON Canonicalization Scheme, RFC 8785. Phygital Key adds rules around it so that "what was signed" and "what is run" are the same bytes, with no rewriting anywhere.

## The rules

| Rule | Exact behaviour | Why |
|---|---|---|
| Strict JSON | Duplicate keys, lone UTF-16 surrogates (`"\ud800"`), data after the value, and nesting deeper than 16 are refused. A UTF-8 byte-order mark before the JSON is refused. | One text, one meaning. Parsers disagree on duplicate keys. |
| Text must already be NFC | Every string and every object key must already be in Unicode Normalization Form C. Text that is not is refused, never rewritten. | See [NFC](#nfc-refused-never-rewritten). |
| Safe integers only | A number must be an integer from $-(2^{53}-1)$ to $2^{53}-1$, that is $-9007199254740991$ to $9007199254740991$. | Larger integers cannot be held exactly by JavaScript, so two verifiers could read different values. |
| No floats | `1.5`, `1e3`, `1.0` and every number with a fraction or exponent are refused. Amounts are strings (`"120.00"`) or integers in minor units (`12000`). | Float formatting is the most common canonicalization bug. |
| No `-0` | `-0` is refused (it is parsed as a float). | It has no integer form and would serialize as `0`. |
| Keys sorted by UTF-16 code units | Object members are ordered by comparing their keys as arrays of UTF-16 code units, as RFC 8785 section 3.2.3 says. | JavaScript sorts this way, so both verifiers agree. |
| No whitespace | No spaces or newlines between tokens. | One spelling. |
| String escaping | See [string escaping](#string-escaping). | One spelling. |

In the gate and the verifier, a request must also be exactly `{"type": "<name>", "params": {...}}`: `type` is 1 to 64 characters from `a-z 0-9 _ . -`, `params` is an object, nothing else. Its canonical text is at most 8192 bytes. The gate also refuses a request nested deeper than 13 levels, so the whole approval object stays within 16.

### NFC: refused, never rewritten

Unicode lets one visible text be stored as different code points. NFC is the standard composed form. Normalizing text to NFC would seem friendly, but NFC also folds some single characters into others. KELVIN SIGN is one:

| Text | Code points | UTF-8 bytes | NFC? |
|---|---|---|---|
| `Kirk` | U+004B U+0069 U+0072 U+006B | `4b 69 72 6b` | Yes |
| `Kirk` with KELVIN SIGN | U+212A U+0069 U+0072 U+006B | `e2 84 aa 69 72 6b` | No: NFC turns U+212A into U+004B |

If the verifier normalized, a person who approved a payment to the first `Kirk` would also have approved one to the second, and the service would act on bytes the person never saw. So the rule is: if `is_nfc(text)` is false, the value has no canonical form. GREEK QUESTION MARK (U+037E, which NFC turns into `;`) and composition exclusions such as U+FB33 are refused the same way. Accented text already in NFC, such as `José` written with U+00E9, is fine.

Look-alike letters that are already NFC, such as a Cyrillic `а` (U+0430) in place of a Latin `a`, are not refused. They are different bytes, so they give a different `request_hash`, and the verifier refuses with `REQUEST_MISMATCH`. The [approval page](approval-page.md) also highlights mixed scripts.

### Key order

Keys are compared as UTF-16 code unit sequences. For characters below U+FFFF this is the same as code point order. Above it, a character is two code units starting with a surrogate (`0xD800` to `0xDBFF`), which sorts before code points from U+E000 up. RFC 8785's own example gives this order:

| Key | Code point | First UTF-16 unit |
|---|---|---|
| `\r` | U+000D | `0x000D` |
| `1` | U+0031 | `0x0031` |
| `\u0080` | U+0080 | `0x0080` |
| `ö` | U+00F6 | `0x00F6` |
| `€` | U+20AC | `0x20AC` |
| 😀 | U+1F600 | `0xD83D` |
| `דּ` | U+FB33 | `0xFB33` |

By code point, U+1F600 would come last. The unit test `rfc8785_utf16_key_order` checks this order (on raw JCS, since U+FB33 is not NFC).

### String escaping

Strings are written in UTF-8 between double quotes. Only these characters are escaped:

| Character | Written as |
|---|---|
| `"` | `\"` |
| `\` | `\\` |
| U+0008 | `\b` |
| U+0009 | `\t` |
| U+000A | `\n` |
| U+000C | `\f` |
| U+000D | `\r` |
| Other U+0000 to U+001F | `\u00XX` with lowercase hex, for example `\u0000`, `\u001f` |

Everything else is written as itself: `/`, U+007F and every non-ASCII character. The unit test `rfc8785_string_escapes` checks this exact output. Integers are written as plain decimal digits with a leading `-` for negatives. `true`, `false` and `null` are written as is.

## Worked example

Take this request, with its keys in any order:

```json
{"type":"deploy","params":{"version":"2.4.1","service":"api","env":"production"}}
```

Its canonical text sorts every key and removes all whitespace. It is 81 bytes:

```json
{"params":{"env":"production","service":"api","version":"2.4.1"},"type":"deploy"}
```

`request_hash` is the base64url (no padding) SHA-256 of those bytes:

$$\texttt{request\_hash} = \mathrm{base64url}\big(\mathrm{SHA256}(\mathrm{canonical}(\text{request}))\big)$$

```bash
python3 -c 'import hashlib,base64; c=b"{\"params\":{\"env\":\"production\",\"service\":\"api\",\"version\":\"2.4.1\"},\"type\":\"deploy\"}"; print(base64.urlsafe_b64encode(hashlib.sha256(c).digest()).rstrip(b"=").decode())'
```

```text
hUXvlpWHC4xarFlhKwaU26H6q05m3gBpMoyE_YKRK7o
```

In hex the digest is `8545ef9695870b8c5aac59612b0694dba1faab4e66de0069328c84fd82912bba`. The same value appears as `request_hash` in the claims of the `valid_medium` test vector, and a running gate returned exactly this `request_jcs` and `request_hash` for this request on its `/api/r/{id}` endpoint.

## The challenge

The person's key does not sign the request directly. It signs a WebAuthn challenge computed from the canonical claims, with a fixed prefix for domain separation:

$$\text{challenge} = \mathrm{SHA256}\big(\texttt{"phygital-key/approval/v1"} \,\|\, \texttt{0x00} \,\|\, \mathrm{canonical}(\text{claims})\big)$$

The prefix is the 24 ASCII bytes of `phygital-key/approval/v1` followed by one zero byte (`CHALLENGE_PREFIX`). The claims are `v`, `id`, `aud`, `request`, `request_hash`, `level`, `iat` and `exp`, so the signature covers the request, the audience, the level and the time window at once.

<details>
<summary>The challenge of the <code>valid_medium</code> test vector</summary>

Canonical claims (one line, keys sorted):

```json
{"aud":"https://deploy.example.com","exp":1760700060,"iat":1760700000,"id":"BwcHBwcHBwcHBwcHBwcHBw","level":"medium","request":{"params":{"env":"production","service":"api","version":"2.4.1"},"type":"deploy"},"request_hash":"hUXvlpWHC4xarFlhKwaU26H6q05m3gBpMoyE_YKRK7o","v":"pka.1"}
```

Challenge, base64url:

```text
F8G3lstZ0FiIgdmhUz8JTz_w63xHE9jUx2ERuO8aAFs
```

The vector's `clientDataJSON` carries exactly this value in its `challenge` field. The approval page computes it the same way in the browser; see [approval page](approval-page.md#computing-the-challenge).

</details>

## base64url

Every binary field (the approval string, credential ids, keys, signatures, hashes) is base64url without padding, decoded strictly: padding (`=`), the standard alphabet's `+` and `/`, and non-canonical trailing bits are refused.

## Inputs that are refused

The verifier returns `MALFORMED` for every input below, before any signature work. The gate refuses the same inputs when an agent asks for an approval, with a message instead of a code.

| Input | Error | Proof |
|---|---|---|
| Approval with base64 padding | `MALFORMED` | vector `base64_padding` |
| Approval base64url cut short | `MALFORMED` | vector `truncated_base64` |
| Valid base64url that is not JSON | `MALFORMED` | vector `not_json` |
| Duplicate JSON key | `MALFORMED` | vector `duplicate_json_key` |
| Nesting deeper than 16 | `MALFORMED` | vector `too_deep` |
| Lone UTF-16 surrogate | `MALFORMED` | vector `unpaired_surrogate` |
| Float amount | `MALFORMED` | vector `float_amount` |
| Request to run is in NFD (decomposed accents) | `MALFORMED` | vector `nfd_request_refused` |
| Payee written with KELVIN SIGN | `MALFORMED` | vector `kelvin_sign_payee_refused` |
| Signed claims that are not NFC | `MALFORMED` | vector `non_nfc_claims_refused` |
| Canonical request of 8193 bytes | `MALFORMED` | vector `request_8193_bytes` (8192 passes: `request_exactly_8192_bytes`) |
| Approval string over 16384 characters | `MALFORMED` | vectors `approval_16386_chars`, `oversize` (16384 passes: `approval_exactly_16384_chars`) |
| Request that is not exactly `{type, params}` | `MALFORMED` | vector `bad_request_shape` |
| Unknown claim | `MALFORMED` | vector `extra_claim` |
| Integer $2^{53}$ | `MALFORMED` | unit test `floats_are_refused` |
| Data after the JSON value | `MALFORMED` | unit test `refuses_duplicates_and_junk` |
| UTF-8 byte-order mark before the approval JSON | `MALFORMED` | JavaScript test "a byte-order mark before the approval JSON is refused" |
| `-0` | Refused by the gate ("request has no canonical form") | no shared vector yet |

Two related cases are not `MALFORMED`, because the text is valid but different:

| Input | Error | Proof |
|---|---|---|
| Same request with keys in another order | Accepted (same canonical form) | vector `valid_key_order_differs` |
| Cyrillic `а` in place of Latin `a` | `REQUEST_MISMATCH` | vector `unicode_homoglyph` |

The end-to-end suite also checks the gate side: "floats are refused at the gate", "look-alike KELVIN SIGN payee refused at the gate (not NFC)" and "too deeply nested request refused before the person sees it".

## Next

- [WebAuthn](webauthn.md): how the challenge is signed and checked
- [Approval page](approval-page.md): how the page shows the canonical text
- [Test vectors](../test-vectors/README.md): every case above, as data
- [Spec](spec.md): the normative rules
