# Audit log

The gate writes every request, decision, key change and receipt to an append-only log in which each line carries the hash of the line before. This page gives the line format, every event, the chain math, what `phygital-key audit-check` proves and what it cannot. It is for operators and reviewers. The code is `crates/phygital-key/src/audit.rs`; the events are written in `state.rs`.

## On this page

- [Where it lives](#where-it-lives)
- [Line format](#line-format)
- [Events](#events)
- [The hash chain](#the-hash-chain)
- [What audit-check verifies](#what-audit-check-verifies)
- [What it detects, and what it does not](#what-it-detects-and-what-it-does-not)
- [Anchor the last hash](#anchor-the-last-hash)
- [Privacy](#privacy)

## Where it lives

| Install | File |
|---|---|
| Quick try (`phygital-key mcp`, `serve`, `demo`) | `~/.phygital-key/audit.log`, or `<home>/audit.log` with `--home` or `PHYGITAL_KEY_HOME` |
| Secure install, macOS | `/Library/Application Support/PhygitalKey/audit.log` (folder readable only by the gate user) |
| Secure install, Linux | `/var/lib/phygital-key/audit.log` (folder readable only by the gate user) |

The file is created with mode `0600`. Each line is appended and flushed to disk (`sync_data`) before the call that caused it returns.

## Line format

One JSON object per line, written in [canonical form](canonical-form.md) (keys sorted, no spaces). These fields are on every line:

| Field | Meaning |
|---|---|
| `seq` | 1 for the first line, then +1 each line |
| `ts` | Unix seconds when the gate wrote it |
| `event` | One of the events below |
| `prev` | The `hash` of the line before, or `genesis` on the first line |
| `hash` | base64url SHA-256 of this line without `hash` (below) |

The real first line of a fresh gate (v0.3.0, port 18410). It is the first key's `add_key` request, because no other request can be made before a key is enrolled:

```json
{"aud":"phygital-key:keyring:http://localhost:18410","event":"requested","hash":"KSPyoSkoXoNlkiMqlmiBjHRPcnXA6QGqrvpmwlTRXWc","id":"HLKRl1YJ0jvAXvhgqNP5bQ","level":"high","prev":"genesis","request_hash":"dONPaQRmDZlb13qYbSjnIOH7lsG97dCPuPaOhD4OvEs","seq":1,"ts":1792275231}
```

A later line from the same log, a deploy request:

```json
{"aud":"https://deploy.example.com","event":"requested","hash":"nHhwWTf7vlnsAUaOFT9wWD_Dj8WHyJNoaDL-NN-a5vI","id":"iDBSupd4RQifp6_8L5Legw","level":"high","prev":"IUPBosAVruV4HMHXCpbgFgzQaXTtENwAdhVVwMKW_FY","request_hash":"hUXvlpWHC4xarFlhKwaU26H6q05m3gBpMoyE_YKRK7o","seq":4,"ts":1792275231}
```

## Events

| Event | When | Extra fields |
|---|---|---|
| `requested` | A request is created (actions, standing approvals, key changes, revocations) | `id`, `aud`, `level`, `request_hash` |
| `approved` | The gate checked the signature and stored the approval | `id`, `aud`, `level`, `request_hash`, `credential_id` (the key that signed) |
| `denied` | The person pressed Deny | `id`, `aud`, `request_hash` |
| `cancelled` | The agent withdrew a waiting request | `id`, `aud`, `request_hash` |
| `revoked` | An approved standing approval was revoked (its use key thrown away) | `id`, `aud`, `request_hash` |
| `revocation_published` | The person signed the revocation and the gate published it | `standing_id`, `approved_with` |
| `key_added` | A key change was approved and saved | `credential_id`, `name`, `approved_with` |
| `key_removed` | A key removal was approved and saved | `credential_id`, `name`, `approved_with` |
| `standing_used` | The gate signed a use token | `id` (the standing approval), `use_id`, `aud`, `request_hash` (this use's request) |
| `receipt` | A receipt signed by the pinned service key was recorded | `id`, `use_id` (or `null`), `aud`, `status`, `reference`, `verified` (`true`) |

`expired` is never logged: it is worked out from the clock when someone looks. Failed signature attempts are not logged either; they change nothing.

## The hash chain

Let $e_n$ be line $n$ as a JSON object without its `hash` field, and $h_n$ its hash:

$$h_n = \mathrm{base64url}\big(\mathrm{SHA256}(\mathrm{JCS}(e_n))\big)$$

$$e_n.\texttt{prev} = \begin{cases} \texttt{"genesis"} & n = 1 \\ h_{n-1} & n > 1 \end{cases} \qquad e_n.\texttt{seq} = n$$

Because $e_n$ contains `prev`, each hash covers the line before it, and so on back to the first line. Changing one byte of line $k$ changes $h_k$, which no longer matches `prev` on line $k+1$.

You can check a line with Python's standard library: sorting keys and dropping spaces gives the same bytes as JCS for these lines, because every value is ASCII text or a small integer.

```bash
python3 -c 'import json,hashlib,base64,sys
prev="genesis"
for n,l in enumerate(open(sys.argv[1]),1):
    e=json.loads(l); h=e.pop("hash")
    c=json.dumps(e,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()
    print(n, base64.urlsafe_b64encode(hashlib.sha256(c).digest()).rstrip(b"=").decode()==h and e["prev"]==prev); prev=h' ~/.phygital-key/audit.log
```

## What audit-check verifies

```bash
phygital-key audit-check
```

```text
audit log ok: 12 entries, last hash JtqlskGHFa5GshgtkRzFPKUK62TpXvrhM2WDiphRzHs
```

That log came from one run: a first key, a deploy approval, a standing approval used once, revoked, and its revocation signed.

For every line, in order, `audit-check` (`verify_file`):

1. parses it with the strict JSON parser and requires an object;
2. requires a string `hash` and removes it;
3. requires `prev` to equal the previous line's hash (`genesis` for the first);
4. requires `seq` to be exactly one more than the previous line's;
5. recomputes the hash and requires it to equal the stored one.

The first failure stops with the line number and the reason, and exit code 1. After one letter of a key name on line 3 was changed:

```text
phygital-key: audit log line 3: hash does not match (line changed)
```

Other reasons: `not a JSON object`, `no hash`, `chain broken (prev does not match)`, `sequence broken`.

When there is no `audit.log` at all, `audit-check` looks for `keyring.json` next to it:

| Data folder | Output | Exit code |
|---|---|---|
| Neither file, or a `keyring.json` with no keys | `no audit log yet: nothing has happened on this gate` | 0 |
| `keyring.json` with a key, but no `audit.log` | `no audit log at <path>, but a key is enrolled: the log was deleted or moved` (printed after `phygital-key: `) | 1 |
| `keyring.json` that cannot be read or is damaged, and no `audit.log` | `<path>: <error>` or `<path>: damaged keyring (<reason>)` (printed after `phygital-key: `) | 1 |

The gate runs the chain check every time it starts: a log whose chain fails stops the gate from starting, with the same message and exit code 1. The checks for a missing log in the table above run only in `audit-check`; a gate started without `audit.log` begins a new one. With the secure install, run it as `phygital-key-admin audit-check`.

## What it detects, and what it does not

| Change to the file | Detected |
|---|---|
| A field edited in any line | Yes: that line's hash no longer matches |
| A line removed from the middle | Yes: `prev` or `seq` of the next line breaks |
| Lines reordered | Yes |
| A line inserted | Yes, unless every later line is rewritten too |
| Lines cut off at the end | No |
| The whole file rewritten with new hashes | No |

> [!IMPORTANT]
> The chain has no secret key. Anyone who can write the file can rebuild every hash, or cut off the last lines, and `audit-check` will still pass. It shows tampering only against a hash you kept somewhere else.

The unit test `chain_detects_changes` checks both sides: an edited line is caught, and a cut-off end is not.

## Anchor the last hash

To detect a rewrite or a cut-off end, copy the last hash out of the gate's reach from time to time: into a ticket, a backup, a chat message to yourself, a commit in another repository. Later, find that hash in the log:

```bash
grep -c '"hash":"JtqlskGHFa5GshgtkRzFPKUK62TpXvrhM2WDiphRzHs"' ~/.phygital-key/audit.log
```

If `audit-check` passes and the anchored hash is still there, every line up to it is unchanged. If it is missing, the log was rewritten or cut.

## Privacy

The log never holds the request itself, only its `request_hash`. It does hold audiences, credential ids, key names, use ids and times. A hash does not hide a request that is easy to guess: someone who suspects the request was `{"type":"deploy","params":{"env":"production","service":"api","version":"2.4.1"}}` can hash it and compare. Treat the log as private, and keep secrets out of requests. See [privacy](privacy.md).

## Next

- [Architecture](architecture.md): when each event is written
- [Secure install](secure-install.md): who can read and write the data folder
- [Threat model](threat-model.md): the audit log among the known limits
- [CLI](cli.md): `audit-check` and the other commands
