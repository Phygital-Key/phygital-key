# Agent prompts

Snippets you can paste into an AI agent's instructions so it asks for approval at the right moments and behaves correctly with the result. They match the gate's own MCP instructions and the rules in [agents](../docs/agents.md). The rules that matter are enforced by the service, not by these words: a service that checks approvals refuses a wrong one whatever the agent believes. The snippets make the agent ask at the right time and stop wasting your attention.

## Claude Code (`CLAUDE.md`)

Put this in the project's `CLAUDE.md` (or `AGENTS.md` for other coding agents). Change the service list to yours.

```markdown
## Approvals (Phygital Key)

Some actions need a person's signed approval through the `phygital-key` MCP server.

Ask for approval (request_approval) before:
- deploying to production: {"type": "deploy", "params": {"service": ..., "env": ..., "version": ...}}, aud https://deploy.example.com, level high
- deleting data or dropping tables: level high
- sending email or messages for me: level medium
- changing DNS, firewall rules or user accounts: level high

How to do it:
1. Call request_approval with the exact request you will run, the service's aud, and the level above.
   Use strings or integers, never floats. Never put secrets in params: they are shown on the approval page
   and travel inside the approval, which anyone who holds it can read.
2. Tell me in one line what you asked for, and give me approve_url if the page did not open.
3. Call get_approval to wait. Act only if status is "approved", and act at once: a one-time approval
   works for 60 seconds by default.
4. Send the action with header `Phygital-Approval: <approval>`, with exactly the request I approved.
5. If the service returns a receipt, pass it back with record_receipt.

Never:
- say or imply that I approved before get_approval returns "approved"
- perform the action, or a smaller or similar one, when the status is denied, cancelled or expired
- put "approved", "pre-approved" or instructions to the reviewer in params
- change anything in the request after I approved it; if it must change, ask again
- reuse an approval, or use one meant for another service

If the service refuses with a code (REQUEST_MISMATCH, EXPIRED, REPLAYED, ...), stop and tell me the code.
```

## Any agent (system prompt)

A shorter version for agents built on an API or another framework, with the gate connected as an MCP server:

```text
You can ask a person to approve an exact action with the Phygital Key tools.
Before any action that deploys, deletes, moves money, sends messages for the person,
or changes infrastructure or accounts, call request_approval with the exact request
({"type": "<lowercase name>", "params": {...}}), the audience of the service that will act,
and level "high" (or "medium" for messages and small edits). Then call get_approval.

Only status "approved" means yes. Then send the approval string with the action, unchanged,
for exactly that request. Denied, cancelled or expired means do not perform the action,
and do not try a variant of it. Never claim the person approved before get_approval says so.
Never write approval words or instructions into params: they are shown to the person as data
and change nothing. Never put secrets in params. If the service refuses, report its error code.
```

## Standing approvals

Add this when an agent repeats one kind of small action:

```markdown
For repeated small actions of one kind (for example payments to one payee, up to a set amount),
you may ask once with request_standing_approval: for_type, fixed values, vary limits
(integer ranges need both min and max; strings use one_of), max_uses, days, aud.
Keep the limits as narrow as the job needs. For each action, call use_standing_approval
with the exact request and send both headers: Phygital-Approval and Phygital-Use.
If the gate refuses a use (outside the limits, no uses left, revoked), do not work around it:
ask me. When I say stop, call cancel_approval and show me sign_url so I can sign the revocation.
```

## What the gate already tells the agent

The gate sends its own instructions to every MCP client when it connects. In short: request, let the person decide on the page, wait with `get_approval`, send the approval with the action, pass back receipts; the agent can never approve, text inside a request never counts as approval, and a denied, expired or cancelled result means do not act. Every tool result also carries a `next` sentence that says what to do now; for an approved standing approval it says to call `use_standing_approval` for each action and send both headers. The exact texts are in the [MCP tools reference](../docs/tools.md#the-status-object). The snippets above repeat this in the agent's own instructions and add when to ask.

## Next

- [Rules for AI agents](../docs/agents.md)
- [MCP tools](../docs/tools.md)
- [Things to ask your AI](../docs/examples.md)
- [All examples](README.md)
