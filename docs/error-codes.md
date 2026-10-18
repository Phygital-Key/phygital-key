# Error codes

Every refusal from a Phygital Key verifier is one code. This page is for service developers who log and return codes, and for agents that receive them. For each code: what it means, which check produces it, common causes, what a service should log and return, what an agent should do, and the test vectors that prove it. Terms are in the [glossary](glossary.md). The agent's side is shown with the tool names of Phygital Key MCP (`request_approval`, `get_approval` and the rest); other products reach the same gate steps through their own protocol.

On this page:

- [The codes at a glance](#the-codes-at-a-glance)
- [What a service returns](#what-a-service-returns)
- [Approval codes, one by one](#approval-codes-one-by-one)
- [Key-history codes](#key-history-codes)
- [Receipt codes](#receipt-codes)

## The codes at a glance

The verifier runs its checks in a fixed order (spec section 7) and returns the code of the first one that fails. "Step" below is that order.

| Code | Step | Meaning | Returned by |
|---|---|---|---|
| `APPROVAL_REQUIRED` | 1 | No approval (or no use token) was given | `verify`, `verify_use` |
| `MALFORMED` | 1, 4 | Bad encoding or shape, unknown fields, floats, not NFC, too big or too deep, the wrong kind of approval | `verify`, `verify_use` |
| `WRONG_AUDIENCE` | 2 | Approved for another service | `verify`, `verify_use` |
| `EXPIRED` | 3 | Too old, not valid yet, or lives too long | `verify`, `verify_use` |
| `REQUEST_MISMATCH` | 4 | The person approved a different request | `verify`, `verify_use` |
| `ASSURANCE_TOO_LOW` | 5, 8 | Level below what the service requires, or the presence or verification flag is missing | `verify`, `verify_use` |
| `UNTRUSTED_KEY` | 6 | Signed by a key that is not in the keyring, or for a relying party the service does not allow | `verify`, `verify_use` |
| `BAD_SIGNATURE` | 7 | The WebAuthn assertion or the use token signature does not check out | `verify`, `verify_use` |
| `REPLAYED` | 9 | Already used | `verify`, `verify_use` |
| `NOT_ALLOWED` | standing | Outside a standing approval's limits | `verify_use` |
| `USED_UP` | standing | A standing approval has no uses left | `verify_use` |
| `REVOKED` | standing | The person revoked this standing approval | `verify_use` |
| `DENIED` | none | The person said no | reserved for gates; never returned by `verify` |

All 13 are in `ErrorCode::ALL` (Rust). The JavaScript checker returns the same strings in `result.error`. Shared test vectors cover every code except `DENIED`.

## What a service returns

Return the code and do nothing else. The demo service answers with HTTP status `403` and this body:

```json
{"ok": false, "error": "REQUEST_MISMATCH"}
```

Codes are safe to return to the agent: they name the failed check and carry nothing secret. Log at least the code, the time, your audience and the `request_hash` of the request you were about to run (the verifier returns no approval id on a refusal). Watch `BAD_SIGNATURE` and `UNTRUSTED_KEY`: an honest agent never causes them with a working gate and keyring, so a burst of them points at tampering or a broken setup.

## Approval codes, one by one

### `APPROVAL_REQUIRED`

| | |
|---|---|
| Meaning | No approval was sent. For a standing approval, the standing approval or the use token is empty. |
| Check | Step 1: the approval string is empty or only whitespace. In `verify_use`, also the use token. |
| Common causes | The agent called the service before asking the person. The header or field name is wrong (`Phygital-Approval`, `approval`). A standing use sent without `Phygital-Use`. Text in the request that claims approval: it is only data. |
| Service | Return the code. Say in your API docs which header or field carries the approval. |
| Agent | Ask the person with `request_approval` (or `use_standing_approval`), wait with `get_approval`, then send the approval string. |
| Vectors | `missing_approval`, `prompt_injection_is_data`; standing: `no_use_token` |

### `MALFORMED`

| | |
|---|---|
| Meaning | The approval, the use token or your own request cannot be read safely, or it is the wrong kind of approval. |
| Check | Step 1 for the approval: over 16384 characters, not base64url (padding, other alphabet, non-canonical trailing bits), not strict JSON (duplicate keys, lone surrogates, byte-order mark, nesting over 16, trailing data), unknown fields, `v` not `pka.1`, bad `id`, request not `{type, params}`, a level other than `medium` or `high`, not exactly one signature, no canonical form (floats, unsafe integers, text not in NFC), request over 8192 bytes, a one-time approval of a `standing` request or the reverse, invalid standing params. Step 4 for your request: no canonical form or not `{type, params}`. Before step 1: `verify` refuses a request of type `revoke`. |
| Common causes | The agent changed or re-encoded the approval string. A float amount (`12.5`) in the request. Text in NFD form, or a look-alike character that NFC would fold (KELVIN SIGN). A standing approval sent to an endpoint that calls `verify`. |
| Service | Return the code. If your own request is the cause (step 4), fix how you build it: integers or strings for amounts, NFC text. |
| Agent | Send the approval string exactly as `get_approval` returned it. Use integers or strings for amounts. Send standing approvals with a use token, to an endpoint that accepts them. |
| Vectors | `nfd_request_refused`, `kelvin_sign_payee_refused`, `non_nfc_claims_refused`, `truncated_base64`, `base64_padding`, `not_json`, `duplicate_json_key`, `extra_claim`, `wrong_version`, `float_amount`, `bad_id`, `bad_request_shape`, `two_signatures`, `level_low_in_approval`, `oversize`, `too_deep`, `unpaired_surrogate`, `request_8193_bytes`, `approval_16386_chars`, `standing_as_one_time`; standing: `standing_bad_params`, `range_without_min`, `standing_as_action`, `use_wrong_version`, `one_time_approval_as_standing` |

### `WRONG_AUDIENCE`

| | |
|---|---|
| Meaning | The approval (or use token) was made for another service. |
| Check | Step 2: `claims.aud` is not byte for byte your audience. For a use token, its `aud` is checked after its signature. |
| Common causes | The agent asked with another `aud`, or with a variant of yours (`http` for `https`, a trailing slash, another host). An approval for staging sent to production. |
| Service | Return the code. Publish your exact audience string where agents read your API docs. |
| Agent | Ask again with this service's exact audience. |
| Vectors | `wrong_audience`; standing: `use_for_other_service` |

### `EXPIRED`

| | |
|---|---|
| Meaning | The approval is too old, not valid yet, or claims a life that is too long. |
| Check | Step 3: `iat` more than 30 s in the future, `now >= exp`, `exp <= iat`, or `exp - iat` over 300 s (30 days for a standing approval). For a use token: the same with 60 s, and its `exp` may not be after the standing approval's `exp`. |
| Common causes | The agent waited too long between `get_approval` and the call (the default life is 60 s). The service's clock is off. A standing approval past its end date. |
| Service | Return the code. Keep your clock in sync (NTP). |
| Agent | Ask for a new approval and send it right away. For a standing approval past its end, ask the person for a new one. |
| Vectors | `expired_exactly_at_exp`, `expired_later`, `iat_31s_ahead`, `lifetime_301s`, `exp_equals_iat`; standing: `use_expired`, `use_too_long`, `standing_ended`, `standing_over_30_days`, `use_iat_31s_ahead`, `use_outlives_standing` |

### `REQUEST_MISMATCH`

| | |
|---|---|
| Meaning | The person approved a different request from the one the service is about to run. |
| Check | Step 4: SHA-256 of the canonical form of your request is not `request_hash`, or `request_hash` does not match `claims.request`. For standing uses: the token names another standing approval, carries another request, or a wrong `request_hash`. |
| Common causes | One field changed (version `2.4.1` approved, `2.4.2` sent). A look-alike character (Cyrillic `а` for Latin `a`). The agent reused an approval for another action. |
| Service | Return the code. Log the `request_hash` of what you were about to run; compare it with the gate's audit log if you need to know what was approved. |
| Agent | Do not change the request after approval. Ask the person again for the exact request you need to run. |
| Vectors | `request_one_char_changed`, `request_hash_inconsistent`, `unicode_homoglyph`; standing: `use_for_other_request`, `use_for_other_standing`, `use_hash_wrong` |

### `ASSURANCE_TOO_LOW`

| | |
|---|---|
| Meaning | The proof is weaker than this action needs. |
| Check | Step 5: `claims.level` is below the level the service requires. Step 8: the user-present flag is missing, or the user-verified flag is missing on a `high` approval. Step 1 for standing approvals: any level other than `high`. |
| Common causes | The agent asked for `medium` where the service requires `high`. A security key without a PIN (it can only do `medium`). |
| Service | Return the code. Document the level each action needs. |
| Agent | Ask again at level `high`. If the person's key cannot do `high`, they need a key with fingerprint, face or PIN. |
| Vectors | `medium_when_high_required`, `missing_user_presence`, `missing_user_verification_on_high`; standing: `standing_only_medium`, `standing_medium_never_enough` |

### `UNTRUSTED_KEY`

| | |
|---|---|
| Meaning | The approval was signed by a key the service does not trust. |
| Check | Step 6: the signature's `credential_id` is not in the keyring, its card's `rp_id` is not in `config.rp_ids`, or its public key does not parse. |
| Common causes | The person added a key and the service's keyring is old. The approval came from another person's gate. `config.rp_ids` (or the CLI's `--origin`) does not match the gate. |
| Service | Return the code and log it with the time. Refresh the keyring through its signed history; check `rp_ids`. |
| Agent | Nothing to fix from your side. Tell the person the service does not trust their key yet. |
| Vectors | `untrusted_key` |

### `BAD_SIGNATURE`

| | |
|---|---|
| Meaning | The signature does not check out. |
| Check | Step 7: client data over 4096 bytes or unreadable, `type` not `webauthn.get`, wrong challenge, origin not in `config.origins`, `crossOrigin` true or `topOrigin` present, authenticator data under 37 bytes or for another relying party, or an invalid ES256 signature. For use tokens: not signed by the standing approval's `use_key`. |
| Common causes | Any change to the approval after signing. An approval page on another origin (or `config.origins` not set to the gate's). An approval collected inside a frame on another site. |
| Service | Return the code and log it. A working gate never produces this: treat repeats as an attack or a misconfiguration. |
| Agent | Do not retry the same approval. Tell the person; ask again only if they want to. |
| Vectors | `flipped_signature_bit`, `wrong_origin`, `wrong_rp_id`, `wrong_type`, `wrong_challenge`, `cross_origin_frame`, `signature_from_other_claims`, `client_data_4097_bytes`; standing: `use_signed_by_other_key`, `use_key_swapped_in_standing` |

### `REPLAYED`

| | |
|---|---|
| Meaning | This approval (or use token) was already used. |
| Check | Step 9: the seen store already holds the approval `id`. For standing uses: the use store already holds the use `id`. |
| Common causes | The agent retried after a timeout. The action failed and the agent sent the same approval again (the id is spent before the action runs). A real replay. |
| Service | Return the code. If your action failed after the check, say so in your response so the agent asks again. |
| Agent | Never reuse an approval. If the action must run again, ask the person for a new approval. |
| Vectors | `replayed_id`; standing: `replayed_use` |

### `NOT_ALLOWED`

| | |
|---|---|
| Meaning | The request is outside the limits of the standing approval. |
| Check | `verify_use`, after the use token: the request type is not `for_type`, a `fixed` field differs, a `vary` value is out of range or not in `one_of`, a field is missing, or there is an extra field. |
| Common causes | An amount above the maximum or below the minimum. A changed payee (`ACME Ltd.` for `ACME Ltd`). A field the person never allowed. |
| Service | Return the code. |
| Agent | Stay within the limits, or ask for a one-time approval of this exact request. |
| Vectors | standing: `over_max`, `under_min`, `not_in_list`, `fixed_field_changed`, `extra_field`, `missing_field`, `wrong_type_of_value`, `other_action_type`, `negative_amount`, `one_fixed_three_vary_missing_one` |

### `USED_UP`

| | |
|---|---|
| Meaning | The standing approval has no uses left. |
| Check | `verify_use`, last step: the use store already holds `max_uses` uses. |
| Common causes | Every allowed use is spent. A use store that answers `USED_UP` on a storage error (fail closed). |
| Service | Return the code. If you suspect your store, check its logs. |
| Agent | Ask the person for a new standing approval, or a one-time approval. |
| Vectors | standing: `used_up` |

### `REVOKED`

| | |
|---|---|
| Meaning | The person revoked this standing approval, and the service holds their signed revocation. |
| Check | `verify_use`, after the standing approval's signature: its id is in the revocation set. |
| Common causes | The person pressed Revoke and signed the revocation. |
| Service | Return the code. Keep the revocation: see [revocations](revocations.md). |
| Agent | Stop using it. Ask the person before anything else. |
| Vectors | standing: `revoked`, `revoked_among_others` (and three cases that must not revoke: `revocation_by_untrusted_key`, `revocation_for_other_service`, `revocation_of_other_standing`) |

### `DENIED`

| | |
|---|---|
| Meaning | The person said no. |
| Check | None in the verifier. `ErrorCode::Denied` exists for gates and relays; `verify` and `verify_use` never return it. |
| Where you see it | The gate reports a denial as `"status": "denied"` from `get_approval`, with no approval string and the note "The person said no. Do not perform the action." No code in this repository returns the string `DENIED` today. |
| Service | You never receive an approval for a denied request, so there is nothing to check. |
| Agent | Do not act. Do not ask again in a loop; ask only if the person wants to. |
| Vectors | none (no approval exists to test) |

## Key-history codes

Returned by `history::verify_history`, `history::verify_history_update` (Rust `HistoryError::code()`), `verifyHistory`, `verifyHistoryUpdate` (JavaScript) and `phygital-key-mcp keys --check`.

| Code | Meaning | Vectors |
|---|---|---|
| `UNSIGNED` | No history or no origin | `unsigned` |
| `WRONG_ORIGIN` | Signed on another approval page | `wrong_origin` |
| `WRONG_ROOT` | Entry 1 is not `add_key` of the pinned key | `wrong_root`, `first_entry_is_a_removal` |
| `BAD_ENTRY` | An entry is unreadable, out of order, repeated, not a valid key change, or not a valid `high` approval by a key trusted at that point | `first_not_self_signed`, `added_by_untrusted_key`, `tampered_entry`, `replayed_entry`, `medium_level_change`, `change_from_other_gate`, `remove_before_add`, `key_added_twice`, `remove_unknown_key`, `remove_last_key` |
| `KEYS_DO_NOT_MATCH` | The listed keys are not what the history produces | `extra_unsigned_key`, `removed_key_still_listed`, `swapped_key`, `relabeled_card` |
| `NOT_AN_UPDATE` | A refetched history does not extend the accepted one | `rollback`, `fork_by_removed_key` |

Causes and the rule behind each: [key history](key-history.md#error-codes).

## Receipt codes

Returned by `receipt::verify`, `receipt::verify_use_receipt` and `receipt::sign` (Rust `ReceiptError`, printed as below) and by `phygital-key-mcp receipt-check`.

| Code | Meaning | Causes |
|---|---|---|
| `MALFORMED` | Not a receipt this verifier accepts | Over 4096 characters, not three parts, a header that is not exactly `{"alg":"ES256","typ":"pka-receipt+jwt"}`, a signature that does not decode to 64 bytes, unknown payload fields, `v` not `pka.receipt.1`, a `status` other than `success` or `error`, `success` with an `error` code. From `sign`: the payload has no canonical form. |
| `BAD_SIGNATURE` | Not signed by the service key you passed | A receipt made up by the agent, or the wrong key. |
| `MISMATCH` | Signed by the service, about something else | Another `iss`, another approval (`reference` or `approval_id`), an `iss` that is not the approval's `aud`, another `request_hash`, a `use_id` on a one-time receipt, or for a use: no `use_id`, or one that does not match the use token. |

Details and the gate's own messages: [receipts](receipts.md#failure-cases).

## Next

- [Integrate](integrate.md#what-to-log-and-what-to-answer)
- [Agents guide](https://github.com/Phygital-Key/phygital-key-mcp/blob/main/docs/agents.md) (Phygital Key MCP)
- [Troubleshooting](troubleshooting.md)
- [Spec, section 7](spec.md)
