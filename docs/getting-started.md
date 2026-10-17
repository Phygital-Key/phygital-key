# Getting started

Pick the path that fits you.

- [I just want to try it (2 minutes)](#try-it)
- [I want my AI app to ask me before risky actions](#use-it-with-your-ai-app)
- [I run a service and want to check approvals](#check-approvals-in-a-service)

## Try it

1. Install (pick one):

   ```bash
   brew install s17sh70x2jfl031ag/tap/phygital-key
   ```

   or `curl -fsSL https://raw.githubusercontent.com/s17sh70x2jfl031ag/phygital-key-mcp/main/install.sh | sh`, or download from [Releases](https://github.com/s17sh70x2jfl031ag/phygital-key-mcp/releases). On Windows: `scoop bucket add phygital-key https://github.com/s17sh70x2jfl031ag/phygital-key-mcp` then `scoop install phygital-key`.

2. Start the demo:

   ```bash
   phygital-key demo
   ```

   It starts the gate on `http://localhost:7468` and a fake service on `http://localhost:7469` that only acts with a valid approval. Nothing real ever happens.

3. Open the link it prints, name your key, press **Add key**, and use Touch ID, Windows Hello, your phone or a hardware key. Then confirm your key once more.

4. Connect your AI app with the line the demo prints (Claude Code: `claude mcp add --transport http phygital-key http://localhost:7468/mcp --header "Authorization: Bearer ..."`), and ask it:

   > Deploy api 2.4.1 to production through the demo service at http://localhost:7469.

   A page opens. Check the details, then approve or deny.

## Use it with your AI app

**Quick (your own user).** Most AI apps start MCP servers themselves:

```bash
claude mcp add phygital-key -- phygital-key mcp
```

Other apps (Claude Desktop, Cursor, ...) take the same command in their MCP settings: command `phygital-key`, argument `mcp`. Add your key with `phygital-key enroll` in your own terminal.

**Safe (recommended for anything real).** Run the gate as its own locked-down user, so the AI can't change it, even acting as you. From a fresh terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/s17sh70x2jfl031ag/phygital-key-mcp/v0.3.0/crates/phygital-key/assets/install-service.sh | sudo bash
phygital-key-admin enroll      # add your key
phygital-key-admin token       # the line to paste into your AI app
phygital-key-admin status      # check it is the real gate answering
```

Your AI app then connects to `http://localhost:7468/mcp` with that token.

**What the AI can do with the gate:** ask (`request_approval`), wait (`get_approval`), cancel or revoke (`cancel_approval`), report a service's receipt (`record_receipt`), and ask for or use a standing permission (`request_standing_approval`, `use_standing_approval`). It can never approve.

**Tips**

- Add two keys (phone and laptop), so losing one doesn't lock you out.
- Share your key fingerprint (on the gate's home page, `http://localhost:7468`) with services you work with.
- Only approve what you expected to see.

## Check approvals in a service

Your service receives an action plus an `approval` string (for example in a `Phygital-Approval` header). Before acting, check it. See [integrate.md](integrate.md) for Rust, JavaScript and the command line, standing approvals, revocations and receipts.
