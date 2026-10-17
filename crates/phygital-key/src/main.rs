//! `phygital-key`: an approval gate for AI actions.
//! A person approves one exact action with a passkey, fingerprint or hardware key;
//! the service that acts checks the signed approval itself.

#![forbid(unsafe_code)]

mod audit;
mod demo;
mod mcp;
mod state;
mod util;
mod web;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use phygital_key_verify::{Config, Expected, Keyring, Level, MemorySeenStore, verify};
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
    /// Print the trusted public key cards (give this to services that verify).
    Keys,
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

async fn run(cli: Cli) -> Result<(), String> {
    let s = settings(&cli)?;
    match cli.command {
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
        Command::Keys => {
            let gate = Gate::open(s)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&gate.keyring()?).map_err(|e| e.to_string())?
            );
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
            eprintln!(
                "note: the command line cannot remember used approvals. Record the id yourself to stop replays."
            );
            match verify(
                &approval,
                &expected,
                &ring,
                &MemorySeenStore::new(),
                &s.config(),
                util::now(),
            ) {
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
                "phygital-key: approval page {}  |  MCP endpoint {}/mcp",
                gate.settings.origin, gate.settings.origin
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
            let demo = demo::Demo::new(
                demo_aud.clone(),
                s.home.join("keyring.json"),
                Config {
                    origins: vec![s.origin.clone()],
                    rp_ids: vec![s.rp_id.clone()],
                },
            );
            let listener = tokio::net::TcpListener::bind(format!("{}:{demo_port}", s.bind))
                .await
                .map_err(|e| format!("demo service port {demo_port}: {e}"))?;
            tokio::spawn(async move {
                let _ = axum::serve(listener, demo::router(demo).into_make_service()).await;
            });
            eprintln!(
                "phygital-key demo\n  approval page   {}\n  MCP endpoint    {}/mcp\n  demo service    {demo_aud}  (fake deploy, payment, send_message)\n\
                 Add the MCP endpoint to your AI app, then ask it to deploy `api` 2.4.1 to production through the demo service.",
                s.origin, s.origin
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
