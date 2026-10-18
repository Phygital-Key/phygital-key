# Run the gate on your own server

Run the gate on a server you control when you want to approve from any device (open approval links on your phone), share one gate between several machines, or keep the gate away from the computer the AI runs on. This page covers the `serve` options, the TLS proxy in front, the access token, the firewall, running it as its own user, and backups.

There is no hosted Phygital Key service. Every gate is one that someone runs themselves.

**On this page:** [what you need](#what-you-need) · [why https](#why-https) · [start the gate](#start-the-gate) · [TLS proxy](#the-tls-proxy) · [add your key](#add-your-key) · [connect AI apps](#connect-ai-apps) · [what becomes public](#what-becomes-public) · [firewall](#firewall) · [own user](#run-it-as-its-own-user) · [backups](#back-up-the-data-folder)

## What you need

- A server (Linux, macOS or Windows) with `phygital-key` installed ([getting started](getting-started.md#before-you-start)).
- A domain name for the approval page, for example `approve.example.com`.
- A TLS proxy in front (Caddy, nginx, a load balancer, any of them) holding a certificate for that name.

## Why https

Passkeys work only on pages served over `https` or from `localhost`. A browser refuses to create or use a passkey anywhere else, so the gate refuses to start with any other address:

```text
phygital-key: --origin must be https (passkeys only work on https, or on localhost)
```

Passkeys also bind each key to the page's host name (the relying party id, here `approve.example.com`). A copy of your approval page on any other domain cannot get a usable signature, and a service checks the origin and host of every approval it accepts. TLS also keeps requests private on the way to your browser: approvals themselves are [signed, not encrypted](privacy.md#signed-not-encrypted).

## Start the gate

```bash
phygital-key serve --origin https://approve.example.com
```

```text
phygital-key: no key enrolled yet. In your own terminal, run: phygital-key enroll (or phygital-key-admin enroll for the secure install)
phygital-key
  approval page   https://approve.example.com
  MCP endpoint    https://approve.example.com/mcp  (access token: run  phygital-key mcp-token)
```

| Option | Default | On a server |
|---|---|---|
| `--origin` | `http://localhost:<port>` | Your public address, `https://approve.example.com`. Only scheme, host and an optional port: no path (`--origin is just scheme://host[:port], no path`) |
| `--bind` | `127.0.0.1` | Keep it when the proxy runs on the same server. On loopback the gate holds both `127.0.0.1` and `[::1]` |
| `--port` | `7468` | The port the proxy forwards to |
| `--home` | `~/.phygital-key` | The data folder (`PHYGITAL_KEY_HOME`) |
| `--lifetime` | `60` | Approval life in seconds, 1 to 300 |

Every option also has an environment variable: `PHYGITAL_KEY_ORIGIN`, `PHYGITAL_KEY_BIND`, `PHYGITAL_KEY_PORT`, `PHYGITAL_KEY_HOME`.

> [!IMPORTANT]
> Run `enroll`, `mcp-token` and `keys` with the **same** `--home` and `--origin` as `serve`. The enrollment link and the MCP line are built from `--origin`: without it, they point at `http://localhost:7468`. Setting `PHYGITAL_KEY_ORIGIN` and `PHYGITAL_KEY_HOME` once in the gate's environment avoids the mistake.

```bash
phygital-key --origin https://approve.example.com enroll
phygital-key --origin https://approve.example.com mcp-token
```

## Run it in a container

From v0.3.0 each release also ships a container image, `ghcr.io/phygital-key/phygital-key` (Linux, amd64 and arm64). The release workflow builds it from the same Linux binaries the signed release contains, after the signing job, and publishes a build provenance record for it. It runs as an unprivileged user (uid 65532) on a distroless base, with the data folder at `/data`.

```bash
docker run -d --name phygital-key --restart unless-stopped \
  -p 127.0.0.1:7468:7468 \
  -e PHYGITAL_KEY_ORIGIN=https://approve.example.com \
  -v phygital-key:/data \
  ghcr.io/phygital-key/phygital-key:latest
docker exec -it phygital-key phygital-key enroll
docker exec -it phygital-key phygital-key mcp-token
```

| Point | Why |
|---|---|
| `-p 127.0.0.1:7468:7468` | Publish the port on loopback only, and let the TLS proxy below reach it. Inside the container the gate listens on all addresses so Docker can forward to it |
| `-e PHYGITAL_KEY_ORIGIN=...` | The public `https` address. `docker exec` uses the same environment, so the enrollment link and the MCP line come out right |
| `-v phygital-key:/data` | Keyring, audit log, MCP token and revocations live in this volume. Back it up like the data folder below |
| `:latest` or `:v0.3.0` | Pin a version tag for a server you update on purpose |

Update by pulling the new image and starting a new container on the same volume: `docker pull ghcr.io/phygital-key/phygital-key:latest`, then `docker rm -f phygital-key` and the `docker run` line again. The secure install's automatic updates do not apply to containers, and `phygital-key update` cannot replace the program inside the image.

Check where an image came from: `gh attestation verify oci://ghcr.io/phygital-key/phygital-key:v0.3.0 --repo Phygital-Key/phygital-key-mcp`.

> [!WARNING]
> On your own computer, a container does not keep the gate out of the AI's reach: if your user can run `docker`, so can an agent acting as you, and Docker access is as strong as administrator access. On a desktop, use the [secure install](secure-install.md). The container is for servers where AI agents do not run.

## The TLS proxy

The proxy ends TLS and forwards plain HTTP to the gate on the same machine. The gate checks two headers on everything it receives, so the proxy must pass them through as the browser sent them:

| Header | The gate accepts | If wrong |
|---|---|---|
| `Host` | Exactly `approve.example.com` (the host of `--origin`, with its port only if the origin has one), or `localhost:7468`, `127.0.0.1:7468`, `[::1]:7468` | `421` `{"error":"unknown host"}`. `approve.example.com:443` is refused too |
| `Origin` (on page POSTs) | Exactly `https://approve.example.com` | `403` `{"error":"wrong origin"}` |

These two checks stop DNS rebinding and cross-site requests, so do not turn them off by rewriting headers.

The shape of the setup, in two common proxies (examples, not tested by the project: check them against your proxy's documentation):

```text
# Caddy (Caddyfile). Caddy gets the certificate and keeps the Host header by default.
approve.example.com {
    reverse_proxy 127.0.0.1:7468
}
```

```text
# nginx, inside the server block for approve.example.com with your TLS certificate
location / {
    proxy_pass http://127.0.0.1:7468;
    proxy_set_header Host $host;
    proxy_buffering off;   # MCP over HTTP streams its answers
}
```

Check it from outside:

```bash
curl -s https://approve.example.com/api/status
```

```json
{"history_ok":false,"keys":[],"origin":"https://approve.example.com","root_fingerprint":null,"rp_id":"approve.example.com"}
```

`"keys":[]` is expected until you enroll.

## Add your key

Run `enroll` on the server (with the same `--home` and `--origin`), for example over SSH, and open the `https://approve.example.com/enroll#token=...` link on the device that holds your key: a phone works directly now. Then follow [your first key](keys-and-devices.md#your-first-key).

A keyring belongs to one address. Keys made on `localhost` do not work on `approve.example.com`, so a gate that moves from your laptop to a server starts a new keyring with a new fingerprint.

## Connect AI apps

AI apps connect to `https://approve.example.com/mcp` with the header `Authorization: Bearer <token>` ([HTTP setup](connect-your-ai.md#http-the-app-connects-to-a-url)). Anyone with the token can create requests, never approve them. Keep it in the apps' settings only, and [rotate it](connect-your-ai.md#rotate-the-token) if it leaks. `--open-mcp` (no token) is a bad idea on a public address: anyone on the internet could flood you with requests.

The gate rate-limits requests: 30 new per minute, 50 waiting at once, 5000 held in memory. Over the limit, the AI gets `Too many requests right now. Try again shortly.` or `Too many requests are waiting. Wait for the person to decide.` The end-to-end tests check that the 31st request in a minute is refused and that the gate still answers afterwards.

## What becomes public

On a public origin, `/.well-known/phygital-key.json` (your key cards, key names and signed key history) and `/.well-known/phygital-key-revoked.json` (signed revocations) are readable by any website (`Access-Control-Allow-Origin: *`), so services can fetch them. Name your keys with that in mind. On `localhost` these files carry no such header, so other websites cannot read them. Requests, approvals and the audit log are never published.

## Firewall

- [ ] Open only the proxy's port (443) to the outside.
- [ ] Keep the gate's port (7468) closed to everything but the proxy. With the default `--bind 127.0.0.1` it is not reachable from outside at all.
- [ ] If the proxy is on another machine and you must `--bind` to a network address, the link between them is plain HTTP: keep it on a private network and allow only the proxy's address.

## Run it as its own user

The [secure install](secure-install.md) sets up the gate for the machine it runs on (`http://localhost:<port>`); it ignores `--origin`, so it cannot serve a public https address yet.

On a server the main separation is the machine itself: the AI runs elsewhere and can reach the gate only through MCP. Still, run the gate under its own user, with the program owned by root and the data folder closed to everyone else, as the secure install does. A systemd unit modelled on the secure install's (an example: the project does not install or test this one):

```text
[Unit]
Description=Phygital Key approval gate
After=network.target

[Service]
User=phygitalkey
Group=phygitalkey
ExecStart=/opt/phygital-key/bin/phygital-key --home /var/lib/phygital-key --origin https://approve.example.com serve
Restart=on-failure
UMask=0077
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ReadWritePaths=/var/lib/phygital-key

[Install]
WantedBy=multi-user.target
```

`serve` never prints the enrollment link or the MCP token, so its log is safe to keep. Run admin commands as the gate user, for example `sudo -u phygitalkey /opt/phygital-key/bin/phygital-key --home /var/lib/phygital-key --origin https://approve.example.com enroll`.

This setup does not update itself (the secure install's daily job is not part of it). Check for a newer signed release and install it, then restart the gate:

```bash
sudo /opt/phygital-key/bin/phygital-key update --check
sudo /opt/phygital-key/bin/phygital-key update
sudo systemctl restart phygital-key
```

`phygital-key update` checks the signed release list and the SHA-256 the same way the installers do ([updates](updates.md#phygital-key-update-install-script-and-cargo)). It treats a program under `/usr/lib/phygital-key/` or `/Library/PhygitalKey/` as the secure install and refuses, which is why this example uses `/opt/phygital-key/`. A restart drops waiting requests and ends standing approvals.

## Back up the data folder

| File | Back it up? | Why |
|---|---|---|
| `keyring.json` | Yes | Your trusted keys and signed history. Without it you start a new keyring with a new fingerprint. It holds only public keys |
| `revocations.json` | Yes | Signed revocations of standing approvals. Lose it, and services that have not fetched them yet never will |
| `audit.log` | Yes | The record of every request, approval, denial, key change and receipt. Also keep its last hash somewhere else (`phygital-key audit-check` prints it): the chain catches edits only if you do |
| `services.json` | Yes, if you use it | Pinned receipt keys (`phygital-key trust-service`) |
| `mcp-token` | Optional | The MCP access token. A secret: if you lose it, a new one is made and apps need the new line |
| `enroll-token.json` | No | A one-time enrollment token, deleted once used |
| `seen-ids.jsonl` | Only on machines that run `phygital-key verify` | Approvals already used by the command-line verifier |

Waiting requests and standing approvals live only in memory and are not in any file. A restart or a restore ends them.

Restore the **newest** copy of `keyring.json`. A service that keeps the copy it accepted (`--previous`) refuses an older one as a rollback.

## Next

- [Keys and devices](keys-and-devices.md)
- [Connect your AI app](connect-your-ai.md)
- [Privacy](privacy.md)
- [Troubleshooting](troubleshooting.md#own-server)
