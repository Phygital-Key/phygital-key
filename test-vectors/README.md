# Test vectors

Test vectors are shared test cases written as data: an input, and the exact result every verifier must give. They are part of the [spec](../docs/spec.md): every implementation, in any language, must pass every vector, and a spec change needs new vectors first. This page lists the files, their exact shapes, how to run and regenerate them, and every vector by name.

There are 141 vectors: 73 approvals, 25 key histories and 43 standing-approval uses. The Rust verifier and the independent JavaScript verifier both pass all of them.

## On this page

- [Files](#files)
- [Shapes](#shapes)
- [Running them](#running-them)
- [How they are generated](#how-they-are-generated)
- [Every vector](#every-vector)

## Files

| File | Tests | Function under test |
|---|---|---|
| `vectors.json` | One-time approvals | `verify(approval, expected, keyring, seen_store, config, now)` |
| `keyrings.json` | Signed key histories | `verify_history(keyring, root_fingerprint, origin)`, or `verify_history_update(accepted, keyring, root_fingerprint, origin)` when `accepted` is set |
| `standing.json` | Uses of standing approvals | `verify_use(standing, use_token, expected, keyring, use_store, revocations, config, now)` |
| `keys/test-keys.json` | The private keys that made every signature | None: for writing new vectors and other tools |

> [!CAUTION]
> The keys in `keys/test-keys.json` are TEST ONLY. Their private halves are published in this repository, so anyone can sign with them. Never put them, or any key card from these files, in a real keyring.

Every file is an object with `spec`, `note` and `vectors`. Every vector has `name`, `description` and `expected`. `expected` is `{"ok": true}` or `{"ok": false, "error": "<CODE>"}`.

## Shapes

### vectors.json

Each vector has `input` with:

| Field | Type | Meaning |
|---|---|---|
| `approval` | string | The approval string (base64url), or `""` |
| `request` | JSON value | The request the verifier is about to act on |
| `aud` | string | The verifier's own audience |
| `level` | string | The lowest level it accepts: `medium` or `high` |
| `now` | integer | The time, Unix seconds |
| `keyring` | object | `{"keys": [key cards]}` |
| `config` | object | `{"origins": [...], "rp_ids": [...]}` |
| `seen_ids` | array of strings | Approval ids already used: put them in the seen store before calling `verify` |

Errors are the [error codes](../docs/error-codes.md) of `verify`: `APPROVAL_REQUIRED`, `MALFORMED`, `WRONG_AUDIENCE`, `EXPIRED`, `REQUEST_MISMATCH`, `BAD_SIGNATURE`, `UNTRUSTED_KEY`, `ASSURANCE_TOO_LOW`, `REPLAYED`.

### keyrings.json

Each vector has these fields at the top level (no `input`):

| Field | Type | Meaning |
|---|---|---|
| `keyring` | object | The keyring to check: `keys`, `origin`, `history` |
| `root_fingerprint` | string | The pinned fingerprint of the first key |
| `origin` | string | The approval page origin the checker expects |
| `accepted` | object or `null` | The keyring accepted before. When set, call `verify_history_update`. |

Errors are `UNSIGNED`, `WRONG_ROOT`, `BAD_ENTRY`, `KEYS_DO_NOT_MATCH`, `WRONG_ORIGIN`, `NOT_AN_UPDATE`.

### standing.json

Each vector has `input` with:

| Field | Type | Meaning |
|---|---|---|
| `standing` | string | The person's standing approval |
| `use` | string | The use token (compact JWS), or `""` |
| `request` | JSON value | The request the service is about to run |
| `aud`, `level`, `now`, `keyring`, `config` | | As in `vectors.json` |
| `prior_uses` | array of strings | Use ids already recorded for this standing approval |
| `revocations` | array of strings | Signed revocations the service has picked up: build `Revocations::from_approvals(revocations, aud, keyring, config)` from them |

The standing approval id in every vector is base64url of 16 bytes of value 40 (`KCgoKCgoKCgoKCgoKCgoKA`). Record each `prior_uses` entry against it with `max_uses` 3, as the Rust and JavaScript test runners do. Errors add `NOT_ALLOWED`, `USED_UP` and `REVOKED` to the codes of `vectors.json`.

## Running them

Rust (`crates/phygital-key-verify/tests/vectors.rs`, part of the workspace tests):

```bash
cargo test --workspace --locked
```

JavaScript (`verifier-js/test/vectors.test.js`, Node 20 or later, no dependencies; output from a run on Node 26, durations removed):

```bash
cd verifier-js
npm test
```

```text
✔ every approval vector
✔ every key-history vector
✔ every standing-approval vector
✔ an approval works once
✔ a level that is a JavaScript built-in name is refused
✔ a byte-order mark before the approval JSON is refused
✔ junk in a revocation list is ignored, never thrown
✔ async stores are awaited: a database-style seen store still stops a replay
✔ async stores are awaited: a database-style use store still counts and stops reuse
ℹ tests 9
ℹ pass 9
```

A new implementation should load each file, run the function under test on each vector, and compare: `ok` must match, and on a refusal the code must be exactly `expected.error`. The Rust test `every_refusal_code_is_covered` also checks that every refusal code has at least one vector in `vectors.json`, except `DENIED` (only gates return it) and the three standing-approval codes, which `standing.json` covers.

## How they are generated

`crates/phygital-key-testgen` writes all four files:

```bash
cargo run -q --release -p phygital-key-testgen --locked -- test-vectors
```

The output is deterministic, so running it twice gives the same bytes:

- the test keys come from fixed seeds: each private key is SHA-256 of `phygital-key TEST ONLY <name>`, and each credential id is the first 16 bytes of SHA-256 of `credential <name>` (names `alice`, `mallory`, `bob`, `gate use key`);
- ECDSA signatures are deterministic (RFC 6979);
- times are fixed: approvals are issued at `iat` 1760700000, and 67 of the 73 approval vectors check at `now` 1760700010.

On every push, CI runs the generator and then `git diff --exit-code test-vectors`, so the committed files must be exactly what the generator writes. To add a vector, change the generator, run it, and commit both.

## Every vector

### vectors.json (55 vectors)

#### Valid approvals

| Name | Expected | What it tests |
|---|---|---|
| `valid_medium` | `ok` | Touch-level approval for the exact request. |
| `valid_high` | `ok` | Biometric or PIN approval, required level high. |
| `valid_high_meets_medium` | `ok` | A high approval satisfies a medium requirement. |
| `valid_key_order_differs` | `ok` | Same request with keys in another order has the same canonical form. |
| `valid_accented_nfc` | `ok` | Accented text in NFC is fine. |
| `valid_iat_30s_ahead` | `ok` | iat up to 30 s in the future is allowed (clock drift). |
| `valid_last_second` | `ok` | One second before exp is still valid. |
| `valid_max_lifetime` | `ok` | A 300 s lifetime is the maximum allowed. |

#### Missing approval and injected text

| Name | Expected | What it tests |
|---|---|---|
| `missing_approval` | `APPROVAL_REQUIRED` | No approval at all. |
| `prompt_injection_is_data` | `APPROVAL_REQUIRED` | Text claiming approval inside params is just data. Still needs a signed approval. |

#### Canonical form and Unicode

| Name | Expected | What it tests |
|---|---|---|
| `nfd_request_refused` | `MALFORMED` | The request to run is not NFC. Text is never rewritten, so it is refused. |
| `kelvin_sign_payee_refused` | `MALFORMED` | Approved a payment to Kirk; the service is about to pay Kirk (KELVIN SIGN). NFC would fold them together, so non-NFC text is refused. |
| `non_nfc_claims_refused` | `MALFORMED` | Signed claims that are not NFC are refused. |
| `float_amount` | `MALFORMED` | Floats are refused. Amounts are strings or integers. |
| `unpaired_surrogate` | `MALFORMED` | Lone UTF-16 surrogates are refused. |
| `unicode_homoglyph` | `REQUEST_MISMATCH` | Cyrillic a looks the same but is a different request. |

#### Encoding and parsing

| Name | Expected | What it tests |
|---|---|---|
| `truncated_base64` | `MALFORMED` | Base64url cut short. |
| `base64_padding` | `MALFORMED` | Padding is not allowed. |
| `not_json` | `MALFORMED` | Valid base64url but not JSON. |
| `duplicate_json_key` | `MALFORMED` | Duplicate keys are refused. |
| `too_deep` | `MALFORMED` | Deep nesting is refused. |

#### Shape of the approval

| Name | Expected | What it tests |
|---|---|---|
| `extra_claim` | `MALFORMED` | Unknown claims are refused, even when signed. |
| `wrong_version` | `MALFORMED` | Only pka.1 is understood. |
| `bad_id` | `MALFORMED` | The id must be 128 bits of base64url. |
| `bad_request_shape` | `MALFORMED` | A request is exactly {type, params}. |
| `two_signatures` | `MALFORMED` | v0.1 takes exactly one signature. |
| `level_low_in_approval` | `MALFORMED` | Low means no approval is needed, so it is never signed. |
| `standing_as_one_time` | `MALFORMED` | A request of type standing is never a one-time approval. |

#### Size limits

| Name | Expected | What it tests |
|---|---|---|
| `oversize` | `MALFORMED` | Approvals over 16 KB are refused before parsing. |
| `request_exactly_8192_bytes` | `ok` | Canonical request size limit is 8192 bytes. |
| `request_8193_bytes` | `MALFORMED` | Canonical request size limit is 8192 bytes. |
| `client_data_exactly_4096_bytes` | `ok` | Client data JSON size limit is 4096 bytes. |
| `client_data_4097_bytes` | `BAD_SIGNATURE` | Client data JSON size limit is 4096 bytes. |
| `approval_exactly_16384_chars` | `ok` | Encoded approval size limit is 16384 characters. |
| `approval_16386_chars` | `MALFORMED` | Encoded approval size limit is 16384 characters. |

#### Audience

| Name | Expected | What it tests |
|---|---|---|
| `wrong_audience` | `WRONG_AUDIENCE` | Approved for another service. |

#### Time

| Name | Expected | What it tests |
|---|---|---|
| `expired_exactly_at_exp` | `EXPIRED` | At exp it is already expired. |
| `expired_later` | `EXPIRED` | Long after exp. |
| `iat_31s_ahead` | `EXPIRED` | iat more than 30 s in the future. |
| `lifetime_301s` | `EXPIRED` | exp - iat over 300 s. |
| `exp_equals_iat` | `EXPIRED` | Zero lifetime. |

#### Exact request

| Name | Expected | What it tests |
|---|---|---|
| `request_one_char_changed` | `REQUEST_MISMATCH` | The service is about to run 2.4.2, the person approved 2.4.1. |
| `request_hash_inconsistent` | `REQUEST_MISMATCH` | request_hash does not match claims.request. |

#### Level and flags

| Name | Expected | What it tests |
|---|---|---|
| `medium_when_high_required` | `ASSURANCE_TOO_LOW` | The service needs high, the person only touched. |
| `missing_user_presence` | `ASSURANCE_TOO_LOW` | No touch recorded. |
| `missing_user_verification_on_high` | `ASSURANCE_TOO_LOW` | High level without fingerprint, face or PIN. |

#### Key and signature

| Name | Expected | What it tests |
|---|---|---|
| `untrusted_key` | `UNTRUSTED_KEY` | Signed by a key that is not in the keyring. |
| `flipped_signature_bit` | `BAD_SIGNATURE` | One bit of the signature changed. |
| `wrong_origin` | `BAD_SIGNATURE` | Approval page on another origin. |
| `wrong_rp_id` | `BAD_SIGNATURE` | Authenticator data for another relying party. |
| `wrong_type` | `BAD_SIGNATURE` | A registration, not an assertion. |
| `wrong_challenge` | `BAD_SIGNATURE` | Signature over another challenge. |
| `cross_origin_frame` | `BAD_SIGNATURE` | Collected inside a frame on another site. |
| `signature_from_other_claims` | `BAD_SIGNATURE` | A real signature, but over different claims. |

#### Replay

| Name | Expected | What it tests |
|---|---|---|
| `replayed_id` | `REPLAYED` | The id was already used. |

### keyrings.json (25 vectors)

#### Valid histories

| Name | Expected | What it tests |
|---|---|---|
| `valid_single_key` | `ok` | The first key approved itself. |
| `valid_two_keys` | `ok` | Alice adds Bob. |
| `valid_add_then_remove` | `ok` | Alice adds Bob, Bob removes Alice. |
| `valid_same_second` | `ok` | Two key changes in the same second are fine. |
| `valid_update` | `ok` | The new history extends the accepted one. |

#### Missing or foreign history

| Name | Expected | What it tests |
|---|---|---|
| `unsigned` | `UNSIGNED` | No history at all. |
| `wrong_origin` | `WRONG_ORIGIN` | A real history, but signed on another gate's page. |
| `change_from_other_gate` | `BAD_ENTRY` | Approved on another gate's page. |

#### The root

| Name | Expected | What it tests |
|---|---|---|
| `wrong_root` | `WRONG_ROOT` | The pinned fingerprint is Bob's, the history starts with Alice. |
| `first_not_self_signed` | `BAD_ENTRY` | Alice's first entry was signed by Bob. |
| `first_entry_is_a_removal` | `WRONG_ROOT` | A history must start by adding the root. |

#### Who signed each change

| Name | Expected | What it tests |
|---|---|---|
| `added_by_untrusted_key` | `BAD_ENTRY` | Mallory approved her own addition. |
| `medium_level_change` | `BAD_ENTRY` | Key changes need level high. |
| `tampered_entry` | `BAD_ENTRY` | A signed entry was edited afterwards. |

#### Order and repeats

| Name | Expected | What it tests |
|---|---|---|
| `replayed_entry` | `BAD_ENTRY` | The same entry twice. |
| `remove_before_add` | `BAD_ENTRY` | Out of order. |
| `key_added_twice` | `BAD_ENTRY` | The same key added again. |
| `remove_unknown_key` | `BAD_ENTRY` | Removes a key that was never added. |
| `remove_last_key` | `BAD_ENTRY` | The last key cannot be removed. |

#### Listed keys versus signed keys

| Name | Expected | What it tests |
|---|---|---|
| `extra_unsigned_key` | `KEYS_DO_NOT_MATCH` | Mallory is listed but was never approved. |
| `removed_key_still_listed` | `KEYS_DO_NOT_MATCH` | Alice was removed but is still listed. |
| `swapped_key` | `KEYS_DO_NOT_MATCH` | Same number of keys, but Mallory instead of Bob. |
| `relabeled_card` | `KEYS_DO_NOT_MATCH` | Same key, but the listed name was changed after signing. |

#### Refetching (update of an accepted copy)

| Name | Expected | What it tests |
|---|---|---|
| `rollback` | `NOT_AN_UPDATE` | An old copy from before Alice was removed. |
| `fork_by_removed_key` | `NOT_AN_UPDATE` | Alice, already removed, signs a new key on top of an old copy. |

### standing.json (43 vectors)

#### Valid uses

| Name | Expected | What it tests |
|---|---|---|
| `valid_use` | `ok` | A payment inside every limit. |
| `valid_min_and_max` | `ok` | Exactly at the limits. |
| `valid_third_use` | `ok` | Two earlier uses, a third is allowed (max 3). |
| `use_iat_30s_ahead` | `ok` | Use token issued 30 s in the future: clock drift allowed. |
| `use_ends_with_standing` | `ok` | A use token may end exactly when the standing approval ends. |
| `one_fixed_three_vary` | `ok` | Field counting: one fixed and three varying fields. |

#### Counting uses

| Name | Expected | What it tests |
|---|---|---|
| `used_up` | `USED_UP` | Three earlier uses, max 3. |
| `replayed_use` | `REPLAYED` | The same use token twice. |

#### Limits

| Name | Expected | What it tests |
|---|---|---|
| `over_max` | `NOT_ALLOWED` | 2001 cents, max is 2000. |
| `under_min` | `NOT_ALLOWED` | 0 cents, min is 1. |
| `negative_amount` | `NOT_ALLOWED` | Below min. |
| `not_in_list` | `NOT_ALLOWED` | memo 'dinner' is not allowed. |
| `fixed_field_changed` | `NOT_ALLOWED` | Payee 'ACME Ltd.' is not 'ACME Ltd'. |
| `extra_field` | `NOT_ALLOWED` | A field the person never allowed. |
| `missing_field` | `NOT_ALLOWED` | A varying field is missing. |
| `wrong_type_of_value` | `NOT_ALLOWED` | Amount as a string, the limit is an integer range. |
| `other_action_type` | `NOT_ALLOWED` | Allowed payments, not refunds. |
| `one_fixed_three_vary_missing_one` | `NOT_ALLOWED` | Field counting: one varying field missing. |

#### Use token binding

| Name | Expected | What it tests |
|---|---|---|
| `use_for_other_request` | `REQUEST_MISMATCH` | The token is for 450 cents, the service runs 500. |
| `use_for_other_standing` | `REQUEST_MISMATCH` | Token names another standing approval. |
| `use_hash_wrong` | `REQUEST_MISMATCH` | Same request, but the token's request_hash is wrong. |
| `use_signed_by_other_key` | `BAD_SIGNATURE` | Token not signed by the standing approval's use key. |
| `use_key_swapped_in_standing` | `BAD_SIGNATURE` | The standing approval names another use key. |
| `use_for_other_service` | `WRONG_AUDIENCE` | Token made for another audience. |
| `use_wrong_version` | `MALFORMED` | Use token with an unknown version. |
| `no_use_token` | `APPROVAL_REQUIRED` | A standing approval alone is not enough. |

#### Time

| Name | Expected | What it tests |
|---|---|---|
| `use_expired` | `EXPIRED` | Token older than 60 s. |
| `use_too_long` | `EXPIRED` | Token life over 60 s. |
| `use_iat_31s_ahead` | `EXPIRED` | Use token issued 31 s in the future. |
| `use_outlives_standing` | `EXPIRED` | A use token may not end after the standing approval. |
| `standing_ended` | `EXPIRED` | After the standing approval's end. |
| `standing_over_30_days` | `EXPIRED` | A standing approval may last 30 days at most. |

#### The standing approval itself

| Name | Expected | What it tests |
|---|---|---|
| `standing_only_medium` | `ASSURANCE_TOO_LOW` | Standing approvals need fingerprint, face or PIN when the service requires high. |
| `standing_medium_never_enough` | `ASSURANCE_TOO_LOW` | Standing approvals need high even when the service asks for medium. |
| `standing_bad_params` | `MALFORMED` | max_uses 0 is not allowed. |
| `range_without_min` | `MALFORMED` | Integer limits need both min and max (no negative amounts by accident). |
| `standing_as_action` | `MALFORMED` | The service is about to run a request of type standing. |
| `one_time_approval_as_standing` | `MALFORMED` | A one-time approval is not a standing one. |

#### Revocations

| Name | Expected | What it tests |
|---|---|---|
| `revoked` | `REVOKED` | The person signed a revocation the service has picked up. |
| `revoked_among_others` | `REVOKED` | One valid revocation among junk still counts. |
| `revocation_by_untrusted_key` | `ok` | A revocation signed by an untrusted key is ignored. |
| `revocation_for_other_service` | `ok` | A revocation for another service is ignored. |
| `revocation_of_other_standing` | `ok` | A revocation of another standing approval changes nothing here. |

## Next

- [Spec](../docs/spec.md): the rules these vectors pin down
- [Canonical form](../docs/canonical-form.md): the parsing and Unicode cases explained
- [Testing](../docs/testing.md): the other tests (property tests, fuzzing, end-to-end)
- [JavaScript verifier](../verifier-js/README.md): the second implementation
