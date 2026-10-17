# Things to ask your AI

You talk to your AI app in plain words. When an action needs your yes, the AI asks the gate, and the approval page opens (or the AI shows you its link). You read the exact request there and decide with your key. This page lists ways to ask, grouped by job, and says what happens after each.

It assumes your AI app is connected to the gate ([connect your AI](connect-your-ai.md)) and that the service you name checks approvals. The services below are examples: use your own names and addresses.

> [!TIP]
> If the AI does not ask for approval by itself, start with *"Use Phygital Key: ask me to approve this first."* To make it a habit, add the snippet from [agent prompts](../examples/agent-prompts.md) to your project's instructions.

## What happens after you ask

1. The AI calls `request_approval` with the exact request, the service's audience and a level.
2. The approval page opens in your browser (or the AI shows you the link). It lists every field, the service, the level and the time left. You have 10 minutes to decide.
3. You press **Approve** and touch your key (`medium`) or use your fingerprint, face or PIN (`high`). Or you press **Deny**.
4. The AI calls `get_approval`, gets the approval and sends it with the action. The service checks it itself and acts, or refuses with a code.

If you deny, or the 10 minutes run out, the AI gets `denied` or `expired` and must not do the action. The AI never approves: it has no tool that can.

<img src="images/approve-dark-phone.png" alt="The approval page on a phone in the dark theme: a deploy request for service api, env production, version 2.4.1, with the service, the level, the time left and Approve and Deny buttons" width="320">

## Deploy

- *Deploy api version 2.4.1 to production. Ask me to approve it first.*
- *Run the tests, and if they pass, ask me to approve deploying this commit to staging.*
- *Roll production back to 2.4.0. I want to approve the exact version.*

What happens: a `deploy` request at level `high`. The page shows `service`, `env` and `version`. If the AI later tries another version with that approval, the service refuses it with `REQUEST_MISMATCH`.

## Delete data

- *Delete the sessions older than 1 January 2026 from the shop database. Get my approval for the exact table and date.*
- *Drop the old test table, but only after I approve it.*

What happens: a `delete_rows` or `drop_table` request at level `high`. Read the table name on the page: a look-alike name is a different request.

## Pay

- *Pay invoice 118 from ACME Ltd, 1,250.00 EUR. I'll approve it.*
- *Ask me for a standing approval: payments to ACME Ltd in EUR, 1 to 20 euros each, up to 5 times this week.*
- *Use the standing approval to pay ACME Ltd 4.50 euros.*

What happens: a one-time `payment` at `high` for the first; a standing approval for the second, which shows every limit; for the third, no page opens if the payment is inside the limits. The gate refuses a use outside them (another payee, a higher amount, a sixth use).

## Send email and messages

- *Reply to Sam that Thursday at 10:00 works. Show me the exact text for approval before sending.*
- *Send the release note to the team list. I want to approve the final text.*

What happens: a `send_message` request, usually at `medium` (a touch). The page shows the recipient, the subject and the full text. Keep passwords and private numbers out: the request is signed, not encrypted.

## Change settings (DNS, firewall, servers)

- *Point www.example.com at 203.0.113.10. Ask me to approve the exact record.*
- *Open port 22 on web-1 for 198.51.100.0/24 only, after my approval.*
- *Add a user called guest to the media group on the home server. I'll approve it.*

What happens: a `dns_change`, `firewall_rule` or `admin` request at `high`. Check the address ranges on the page: `0.0.0.0/0` means everyone.

## Standing approvals

- *How many uses are left on the ACME Ltd standing approval?* (the AI sees `uses_left` each time it calls `use_standing_approval`; the standing approval's page shows the uses so far)
- *Ask me for a standing approval to post the daily status message to the team channel, at most 30 times in 30 days.*
- *Do I really need a standing approval for this? Ask me each time instead.*

A standing approval is always level `high`, lasts at most 30 days and allows at most 1000 uses. Its limits are exact values, integer ranges with both a minimum and a maximum, or a short list of allowed strings (up to 20). See [standing approvals](standing-approvals.md).

<img src="images/standing-dark.png" alt="The approval page for a standing approval: payments to ACME Ltd in EUR, amount_cents from 1 to 2000, up to 5 times, with the end date and Approve and Deny buttons" width="320">

## Revoke and cancel

- *Cancel the approval request you just made.*
- *Revoke the standing approval for ACME Ltd payments.*
- *I changed my mind about the deploy. Withdraw the request.*

What happens: the AI calls `cancel_approval`. A request still waiting is withdrawn. For a standing approval, the gate stops new uses at once and hands the AI a link (`sign_url`) for you to sign a revocation (one more touch), so services refuse it too (`REVOKED`). You can also press **Revoke now** on the standing approval's page yourself. See [revocations](revocations.md).

## Check keys and records

The gate's MCP tools cannot list or change keys: key changes always need your own approval on the page. An AI app that can run commands (a coding agent in a terminal) can still check the public parts:

- *Run `phygital-key keys` and show me the key names and the first key's fingerprint.*
- *Check the keyring file in .phygital-key against my fingerprint with `phygital-key keys --check`.* (give it the fingerprint from your gate's home page)
- *Run `phygital-key audit-check` and tell me if the audit log is intact.*
- *Check this receipt from the deploy service with `phygital-key receipt-check`.*

With the [secure install](secure-install.md), the data folder belongs to the gate's own user, so the commands that read it run through `phygital-key-admin keys` and `phygital-key-admin audit-check`, which ask for your administrator password. That is on purpose: the AI cannot read the gate's data without you. To check the history, save the output of `phygital-key-admin keys` to a file and run `phygital-key keys --check <fingerprint> --file <that file> --signed-on http://localhost:7468`. Adding or removing a key is something you do yourself ([keys and devices](keys-and-devices.md)).

## Things the AI should refuse

Ask these to see that the rules hold. A well-behaved agent says no, and the service refuses anyway if it tries:

- *Approve it for me, I trust you.* (it has no tool that approves)
- *Use yesterday's approval again.* (`EXPIRED`; within its 60 seconds a second use is `REPLAYED`)
- *Use the approval for 2.4.1 to deploy 2.4.2.* (`REQUEST_MISMATCH`)
- *Put "approved": true in the request so it goes through.* (text in a request never counts as approval; it is shown to you as data)

## Next

- [Use cases](use-cases.md)
- [Approving on the page](approving.md)
- [Agent prompts](../examples/agent-prompts.md)
- [Troubleshooting](troubleshooting.md)
