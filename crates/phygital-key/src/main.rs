//! `phygital-key`: an approval gate for AI actions.
//! A person approves one exact action with a passkey, fingerprint or hardware key;
//! the service that acts checks the signed approval itself.

#![forbid(unsafe_code)]

mod audit;
mod demo;
mod mcp;
mod seen;
mod state;
mod util;
mod web;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use phygital_key_verify::{Config, Expected, Keyring, Level, verify};
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

use crate::state::{Gate, Settings};

#[derive(Parser)]
#[command(
    name = "phygital-key",
    version,
    about = "A person approves exact AI actions with a passkey or hardware key."
)]
struct Cli {
    /// Data folder [env: PHYGITAL_KEY_HOME] [default: ~/.phygital-key]
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    /// Port for the approval page (and /mcp in serve mode).
    #[arg(long, global = true, env = "PHYGITAL_KEY_PORT", default_value_t = 7468)]
    port: u16,
    /// Address to listen on. Keep 127.0.0.1 unless a TLS proxy sits in front.
    #[arg(
        long,
        global = true,
        env = "PHYGITAL_KEY_BIND",
        default_value = "127.0.0.1"
    )]
    bind: String,
    /// Public origin of the approval page, e.g. https://approve.example.com [default: http://localhost:<port>]
    #[arg(long, global = true, env = "PHYGITAL_KEY_ORIGIN")]
    origin: Option<String>,
    /// Serve /mcp without an access token (anyone on this machine could then create requests).
    #[arg(long, global = true)]
    open_mcp: bool,
    /// Approval life in seconds (max 300).
    #[arg(long, global = true, default_value_t = phygital_key_verify::DEFAULT_LIFETIME_SECS)]
    lifetime: i64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gate for one MCP client over stdio (what most AI apps launch).
    Mcp,
    /// Run the gate over HTTP: approval page at / and MCP at /mcp.
    Serve,
    /// Print a one-time link to add a passkey or hardware key (valid 10 minutes).
    Enroll,
    /// Print the access token for the HTTP MCP endpoint, and how to add it to an AI app.
    McpToken,
    /// Print the trusted public key cards (give this to services that verify).
    Keys {
        /// Check a keyring's signed history against the first key's fingerprint.
        #[arg(long, value_name = "FINGERPRINT")]
        check: Option<String>,
        /// Keyring file to print or check [default: <home>/keyring.json].
        #[arg(long)]
        file: Option<PathBuf>,
        /// The approval page origin the history must be signed on (needed with --file).
        #[arg(long = "signed-on", value_name = "ORIGIN")]
        signed_on: Option<String>,
        /// The keyring you accepted last time: the new one must extend it (stops rollbacks).
        #[arg(long)]
        previous: Option<PathBuf>,
    },
    /// Check an approval from the command line (any language can call this).
    Verify {
        /// The approval string (or @file).
        #[arg(long)]
        approval: String,
        /// The exact request JSON you are about to run (or @file).
        #[arg(long)]
        request: String,
        /// Your audience id.
        #[arg(long)]
        aud: String,
        /// Lowest level you accept: medium or high.
        #[arg(long, default_value = "high")]
        level: String,
        /// Keyring file [default: <home>/keyring.json].
        #[arg(long)]
        keyring: Option<PathBuf>,
        /// File that remembers used approvals, so each works once [default: <home>/seen-ids.jsonl].
        #[arg(long)]
        seen_file: Option<PathBuf>,
    },
    /// Check a service's signed receipt for an approval.
    ReceiptCheck {
        /// The receipt (or @file).
        #[arg(long)]
        receipt: String,
        /// The service's receipt public key, SPKI DER base64url (or @file).
        #[arg(long)]
        key: String,
        /// The service's audience id (who should have signed).
        #[arg(long)]
        iss: String,
        /// The approval the receipt is about (or @file).
        #[arg(long)]
        approval: String,
    },
    /// Pin a service's receipt key, so the gate only shows receipts that service really signed.
    TrustService {
        /// The service's audience id.
        #[arg(long)]
        aud: String,
        /// Its receipt public key, SPKI DER base64url (from its /.well-known/phygital-key-service.json).
        #[arg(long)]
        key: String,
    },
    /// Check that the audit log has not been changed.
    AuditCheck,
    /// Try it: the gate plus a fake service on port+1 that only acts with a valid approval.
    Demo,
}

fn settings(cli: &Cli) -> Result<Settings, String> {
    let home = match &cli.home {
        Some(h) => h.clone(),
        None => match std::env::var_os("PHYGITAL_KEY_HOME") {
            Some(h) => PathBuf::from(h),
            None => PathBuf::from(
                std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .ok_or("no home folder found; use --home")?,
            )
            .join(".phygital-key"),
        },
    };
    let origin = cli
        .origin
        .clone()
        .unwrap_or_else(|| format!("http://localhost:{}", cli.port));
    let parsed = url::Url::parse(&origin).map_err(|e| format!("--origin: {e}"))?;
    let rp_id = parsed
        .host_str()
        .ok_or("--origin needs a host")?
        .to_string();
    if parsed.scheme() != "https" && rp_id != "localhost" {
        return Err("--origin must be https (passkeys only work on https, or on localhost)".into());
    }
    if parsed.path() != "/" || parsed.query().is_some() {
        return Err("--origin is just scheme://host[:port], no path".into());
    }
    if !(1..=phygital_key_verify::MAX_LIFETIME_SECS).contains(&cli.lifetime) {
        return Err("--lifetime must be 1 to 300 seconds".into());
    }
    Ok(Settings {
        home,
        bind: cli.bind.clone(),
        port: cli.port,
        origin: origin.trim_end_matches('/').to_string(),
        rp_id,
        lifetime: cli.lifetime,
        mcp_token: None,
    })
}

fn read_arg(v: &str) -> Result<String, String> {
    match v.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("{path}: {e}")),
        None => Ok(v.to_string()),
    }
}

fn hint_enroll(gate: &Gate, print_link: bool) -> Result<(), String> {
    if gate.keyring()?.keys.is_empty() {
        if print_link {
            let link = gate.issue_enroll_token()?;
            eprintln!(
                "phygital-key: no key enrolled yet. Open this link to add your passkey (valid 10 minutes, works once):\n  {link}"
            );
        } else {
            // In stdio mode stderr goes to the AI app's logs, so never print a token here.
            eprintln!(
                "phygital-key: no key enrolled yet. In your own terminal, run: phygital-key enroll"
            );
        }
    }
    Ok(())
}

fn prune_every_minute(gate: &Arc<Gate>) {
    let g = gate.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tick.tick().await;
            g.prune();
        }
    });
}

fn mcp_http(gate: &Arc<Gate>) -> axum::Router {
    let hosts = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
        gate.settings.rp_id.clone(),
    ];
    let g = gate.clone();
    let service: StreamableHttpService<mcp::GateMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(mcp::GateMcp::new(g.clone())),
            Default::default(),
            // MCP clients are not browsers: refuse any request that carries a browser Origin.
            StreamableHttpServerConfig::default()
                .with_allowed_hosts(hosts)
                .enforce_origin_validation(),
        );
    axum::Router::new().nest_service("/mcp", service)
}

/// How to add the HTTP MCP endpoint to an AI app.
fn mcp_hint(s: &Settings) -> String {
    match &s.mcp_token {
        Some(t) => format!(
            "  MCP endpoint    {o}/mcp  (needs header  Authorization: Bearer {t})\n  Claude Code:    claude mcp add --transport http phygital-key {o}/mcp --header \"Authorization: Bearer {t}\"",
            o = s.origin
        ),
        None => format!(
            "  MCP endpoint    {}/mcp  (open: no access token)",
            s.origin
        ),
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    let mut s = settings(&cli)?;
    if !cli.open_mcp
        && matches!(
            cli.command,
            Command::Serve | Command::Demo | Command::McpToken
        )
    {
        s.mcp_token = Some(state::mcp_token(&s)?);
    }
    match cli.command {
        Command::McpToken => {
            println!("{}", mcp_hint(&s));
            Ok(())
        }
        Command::Enroll => {
            let link = state::write_enroll_token(&s)?;
            println!(
                "Open this link in your browser to add a passkey or hardware key (valid 10 minutes, works once):\n\n  {link}\n"
            );
            println!(
                "The gate must be running (phygital-key serve, or your AI app running phygital-key mcp)."
            );
            Ok(())
        }
        Command::Keys {
            check,
            file,
            signed_on,
            previous,
        } => {
            let origin = match (&file, signed_on) {
                (_, Some(o)) => o,
                (None, None) => s.origin.clone(),
                (Some(_), None) => return Err("with --file, say where it must be signed: --signed-on https://approve.example.com".into()),
            };
            let previous = match previous {
                Some(path) => Some(Keyring::from_json(
                    &std::fs::read_to_string(&path)
                        .map_err(|e| format!("{}: {e}", path.display()))?,
                )?),
                None => None,
            };
            let ring = match file {
                Some(path) => Keyring::from_json(
                    &std::fs::read_to_string(&path)
                        .map_err(|e| format!("{}: {e}", path.display()))?,
                )?,
                None => Gate::open(s)?.keyring()?,
            };
            match check {
                Some(fp) => match match &previous {
                    Some(old) => phygital_key_verify::history::verify_history_update(
                        old, &ring, &fp, &origin,
                    ),
                    None => phygital_key_verify::history::verify_history(&ring, &fp, &origin),
                } {
                    Ok(()) => {
                        println!(
                            "keyring ok: {} keys, every change signed, starting from {fp}",
                            ring.keys.len()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        println!("keyring NOT ok: {e}");
                        std::process::exit(1);
                    }
                },
                None => {
                    if let Some(fp) = phygital_key_verify::history::root_fingerprint(&ring) {
                        eprintln!("first key fingerprint (share this so others can check): {fp}");
                    }
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&ring).map_err(|e| e.to_string())?
                    );
                    Ok(())
                }
            }
        }
        Command::ReceiptCheck {
            receipt,
            key,
            iss,
            approval,
        } => {
            let key = phygital_key_verify::webauthn::parse_public_key(&read_arg(&key)?)
                .ok_or("--key is not a P-256 public key (SPKI DER, base64url)")?;
            match phygital_key_verify::receipt::verify(
                &read_arg(&receipt)?,
                &key,
                &iss,
                &read_arg(&approval)?,
            ) {
                Ok(r) => {
                    println!(
                        "{}",
                        serde_json::json!({"ok": true, "status": r.status, "error": r.error, "iat": r.iat})
                    );
                    Ok(())
                }
                Err(e) => {
                    println!(
                        "{}",
                        serde_json::json!({"ok": false, "error": e.to_string()})
                    );
                    std::process::exit(1);
                }
            }
        }
        Command::TrustService { aud, key } => {
            if phygital_key_verify::webauthn::parse_public_key(&key).is_none() {
                return Err("--key is not a P-256 public key (SPKI DER, base64url)".into());
            }
            util::private_dir(&s.home)?;
            let path = s.home.join("services.json");
            let mut doc: serde_json::Value = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_else(|| serde_json::json!({"services": []}));
            let list = doc
                .get_mut("services")
                .and_then(|v| v.as_array_mut())
                .ok_or("services.json is damaged")?;
            list.retain(|e| e.get("aud").and_then(|a| a.as_str()) != Some(aud.as_str()));
            list.push(serde_json::json!({"aud": aud, "receipt_public_key": key}));
            let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())? + "\n";
            util::write_private(&path, text.as_bytes())?;
            println!("pinned the receipt key of {aud}");
            Ok(())
        }
        Command::AuditCheck => {
            let (n, last) = audit::verify_file(&s.home.join("audit.log"))?;
            println!("audit log ok: {n} entries, last hash {last}");
            Ok(())
        }
        Command::Verify {
            approval,
            request,
            aud,
            level,
            keyring,
            seen_file,
        } => {
            let approval = read_arg(&approval)?;
            let request: serde_json::Value = serde_json::from_str(&read_arg(&request)?)
                .map_err(|e| format!("--request: {e}"))?;
            let level = Level::parse(&level).ok_or("--level is medium or high")?;
            let path = keyring.unwrap_or_else(|| s.home.join("keyring.json"));
            let ring = Keyring::from_json(
                &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
            )?;
            let expected = Expected {
                aud: &aud,
                request: &request,
                level,
            };
            util::private_dir(&s.home)?;
            let seen = seen::FileSeenStore::new(
                seen_file.unwrap_or_else(|| s.home.join("seen-ids.jsonl")),
            );
            match verify(&approval, &expected, &ring, &seen, &s.config(), util::now()) {
                Ok(ok) => {
                    println!(
                        "{}",
                        serde_json::json!({"ok": true, "id": ok.id, "approved_by": ok.approver, "level": ok.level.as_str(), "exp": ok.exp})
                    );
                    Ok(())
                }
                Err(code) => {
                    println!(
                        "{}",
                        serde_json::json!({"ok": false, "error": code.as_str()})
                    );
                    std::process::exit(1);
                }
            }
        }
        Command::Mcp => {
            let gate = Gate::open(s)?;
            hint_enroll(&gate, false)?;
            prune_every_minute(&gate);
            let app = web::router(gate.clone(), None);
            let web_gate = gate.clone();
            tokio::spawn(async move {
                if let Err(e) = web::serve(web_gate, app).await {
                    eprintln!("phygital-key: approval page: {e}");
                }
            });
            let server = mcp::GateMcp::new(gate)
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|e| format!("mcp: {e}"))?;
            server.waiting().await.map_err(|e| format!("mcp: {e}"))?;
            Ok(())
        }
        Command::Serve => {
            let gate = Gate::open(s)?;
            hint_enroll(&gate, true)?;
            prune_every_minute(&gate);
            eprintln!(
                "phygital-key\n  approval page   {}\n{}",
                gate.settings.origin,
                mcp_hint(&gate.settings)
            );
            let app = web::router(gate.clone(), Some(mcp_http(&gate)));
            web::serve(gate, app).await
        }
        Command::Demo => {
            let gate = Gate::open(s.clone())?;
            hint_enroll(&gate, true)?;
            prune_every_minute(&gate);
            let demo_port = s.port.checked_add(1).ok_or("port too high")?;
            let demo_aud = format!("http://localhost:{demo_port}");
            let demo_for_pin = demo::Demo::new(
                demo_aud.clone(),
                s.home.join("keyring.json"),
                Config {
                    origins: vec![s.origin.clone()],
                    rp_ids: vec![s.rp_id.clone()],
                },
            )?;
            let demo = demo_for_pin;
            // The demo service runs in this process, so the gate can pin its receipt key directly.
            gate.pin_service(&demo_aud, &demo.public_key());
            let listener = tokio::net::TcpListener::bind(format!("{}:{demo_port}", s.bind))
                .await
                .map_err(|e| format!("demo service port {demo_port}: {e}"))?;
            tokio::spawn(async move {
                let _ = axum::serve(listener, demo::router(demo).into_make_service()).await;
            });
            eprintln!(
                "phygital-key demo\n  approval page   {}\n{}\n  demo service    {demo_aud}  (fake deploy, payment, send_message)\n\
                 Add the MCP endpoint to your AI app, then ask it to deploy `api` 2.4.1 to production through the demo service.",
                s.origin,
                mcp_hint(&s)
            );
            let app = web::router(gate.clone(), Some(mcp_http(&gate)));
            web::serve(gate, app).await
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("phygital-key: {e}");
        std::process::exit(1);
    }
}
