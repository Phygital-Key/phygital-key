//! The shared commands of every Phygital Key product (`serve`, `enroll`, `keys`,
//! `verify`, `install-service`, ...). The product names itself and adds its agent
//! protocol through [`crate::Adapter`].

use std::path::PathBuf;
use std::sync::Arc;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use phygital_key_verify::{Config, Expected, Keyring, Level, verify};

use crate::product::{self, Adapter, Product, product};
use crate::state::{self, Gate, Settings};
use crate::{audit, demo, seen, update, util, web};

#[derive(Parser)]
struct Cli {
    /// Data folder [default: ~/.<program name>]
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    /// Port for the approval page (and the agent endpoint in serve mode).
    #[arg(long, global = true, default_value_t = 7468)]
    port: u16,
    /// Address to listen on. Keep 127.0.0.1 unless a TLS proxy sits in front.
    #[arg(long, global = true, default_value = "127.0.0.1")]
    bind: String,
    /// Public origin of the approval page, e.g. https://approve.example.com [default: http://localhost:<port>]
    #[arg(long, global = true)]
    origin: Option<String>,
    /// Serve the agent endpoint without an access token (anyone on this machine could then create requests).
    #[arg(long, global = true)]
    no_token: bool,
    /// Approval life in seconds (max 300).
    #[arg(long, global = true, default_value_t = phygital_key_verify::DEFAULT_LIFETIME_SECS)]
    lifetime: i64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gate for one AI app over stdio (what most MCP apps launch).
    Stdio,
    /// Run the gate over HTTP: the approval page at / and the agent endpoint.
    Serve,
    /// Print a one-time link to add a passkey or hardware key (valid 10 minutes).
    Enroll,
    /// Print the access token for the agent endpoint, and how to connect an AI app or agent.
    Token,
    /// Run the gate as its own locked-down system user (macOS, Linux). Needs sudo.
    InstallService {
        /// Remove the service (keeps keys and log unless --purge).
        #[arg(long)]
        uninstall: bool,
        /// Install this binary instead of downloading the official release.
        /// It and every folder above it must be owned by root only.
        #[arg(long)]
        binary: Option<PathBuf>,
        /// Release to install, e.g. v0.4.0 (default: this program's version). Use it to update.
        #[arg(long = "release", value_name = "VERSION")]
        release: Option<String>,
        /// With --uninstall: also delete the service user, keys and log.
        #[arg(long)]
        purge: bool,
        /// Update automatically every day to the latest signed release (the default).
        #[arg(long, conflicts_with = "no_auto_update")]
        auto_update: bool,
        /// Turn automatic updates off (kept on later installs until --auto-update).
        #[arg(long)]
        no_auto_update: bool,
        /// Used by the daily update job: install the latest signed release if it is newer.
        #[arg(long, hide = true)]
        auto_update_run: bool,
        /// With --auto-update-run: update even while standing approvals are active.
        #[arg(long, hide = true)]
        now: bool,
    },
    /// Update this program to the latest signed release (for installs in your own folders).
    Update {
        /// Only say whether a newer release exists.
        #[arg(long)]
        check: bool,
    },
    /// Write the product's files made from the base templates (installers, install test, release scripts).
    #[command(hide = true)]
    ProductFiles {
        /// Folder to write them to.
        #[arg(long, default_value = ".")]
        out: PathBuf,
    },
    /// Check a signed release list (used by the secure installer).
    #[command(hide = true)]
    ReleaseCheck {
        /// release.txt
        #[arg(long)]
        file: PathBuf,
        /// release.txt.sig
        #[arg(long)]
        sig: PathBuf,
    },
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
        /// Its receipt public key, SPKI DER base64url (or @file), from its /.well-known/phygital-key-service.json.
        #[arg(long)]
        key: String,
    },
    /// Check that the audit log has not been changed.
    AuditCheck,
    /// Try it: the gate plus a fake service on port+1 that only acts with a valid approval.
    Demo,
}

fn settings(cli: &Cli, agent_path: &'static str) -> Result<Settings, String> {
    let home = match &cli.home {
        Some(h) => h.clone(),
        None => match std::env::var_os(product().env("HOME")) {
            Some(h) => PathBuf::from(h),
            None => PathBuf::from(
                std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .ok_or("no home folder found; use --home")?,
            )
            .join(format!(".{}", product().bin)),
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
        access_token: None,
        agent_path,
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
                "{}: no key enrolled yet. Open this link to add your passkey (valid 10 minutes, works once):\n  {link}",
                product().bin
            );
        } else {
            // In stdio mode stderr goes to the AI app's logs, so never print a token here.
            let b = product().bin;
            eprintln!(
                "{b}: no key enrolled yet. In your own terminal, run: {b} enroll (or {b}-admin enroll for the secure install)"
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

/// The approval page plus the adapter's routes, for serve and demo.
fn app(gate: &Arc<Gate>, adapter: &Arc<dyn Adapter>) -> axum::Router {
    let mut extra = adapter.router(gate.clone());
    if let Some(public) = adapter.public_router(gate.clone()) {
        extra = extra.merge(public);
    }
    web::router(gate.clone(), Some(extra))
}

async fn run_cli(cli: Cli, adapter: Arc<dyn Adapter>) -> Result<(), String> {
    let mut s = settings(&cli, adapter.path())?;
    if !cli.no_token && matches!(cli.command, Command::Serve | Command::Demo | Command::Token) {
        s.access_token = Some(state::access_token(&s)?);
    }
    match cli.command {
        Command::InstallService {
            uninstall,
            purge,
            binary,
            release,
            auto_update,
            no_auto_update,
            auto_update_run,
            now,
        } => {
            let script = product::install_service_script(product());
            let mut args = vec!["-s".to_string(), "--".to_string()];
            for (on, flag) in [
                (uninstall, "--uninstall"),
                (purge, "--purge"),
                (auto_update, "--auto-update"),
                (no_auto_update, "--no-auto-update"),
                (auto_update_run, "--auto-update-run"),
                (now, "--now"),
            ] {
                if on {
                    args.push(flag.into());
                }
            }
            // Runs as root: start from an empty environment, so nothing of the
            // caller's (HOME with a ~/.curlrc, PATH with a planted curl, BASH_ENV,
            // OPENSSL_CONF...) can steer what root runs, downloads or writes.
            let root_home = if cfg!(target_os = "macos") {
                "/var/root"
            } else {
                "/root"
            };
            let mut child = std::process::Command::new("/bin/bash")
                .env_clear()
                .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .env("HOME", root_home)
                .env("LANG", "C")
                .args(&args)
                // The script downloads and checks the official release itself: this
                // program file is yours, so it is never what gets installed as root.
                .env(
                    "PKA_VERSION",
                    release.unwrap_or_else(|| format!("v{}", product().version)),
                )
                .env(
                    "PKA_BINARY",
                    binary.map(|b| b.display().to_string()).unwrap_or_default(),
                )
                .env("PKA_PORT", s.port.to_string())
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("cannot run bash: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin
                    .write_all(script.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let status = child.wait().map_err(|e| e.to_string())?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
            Ok(())
        }
        Command::ProductFiles { out } => {
            for (name, text) in product::product_files(product(), adapter.path()) {
                let path = out.join(name);
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                }
                std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                        .map_err(|e| e.to_string())?;
                }
                println!("wrote {}", path.display());
            }
            Ok(())
        }
        Command::Update { check } => update::update(check),
        Command::ReleaseCheck { file, sig } => update::release_check(&file, &sig),
        Command::Token => {
            println!("{}", adapter.connect_hint(&s));
            Ok(())
        }
        Command::Enroll => {
            let link = state::write_enroll_token(&s)?;
            println!(
                "Open this link in your browser to add a passkey or hardware key (valid 10 minutes, works once):\n\n  {link}\n"
            );
            println!(
                "The gate must be running ({} serve, or your AI app running it).",
                product().bin
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
            let key = read_arg(&key)?;
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
            let log = s.home.join("audit.log");
            if !log.exists() {
                let ring = s.home.join("keyring.json");
                let enrolled = if ring.exists() {
                    // A keyring that cannot be read is not "nothing happened".
                    let text = std::fs::read_to_string(&ring)
                        .map_err(|e| format!("{}: {e}", ring.display()))?;
                    let k = Keyring::from_json(&text)
                        .map_err(|e| format!("{}: damaged keyring ({e})", ring.display()))?;
                    !k.keys.is_empty()
                } else {
                    false
                };
                if enrolled {
                    return Err(format!(
                        "no audit log at {}, but a key is enrolled: the log was deleted or moved",
                        log.display()
                    ));
                }
                println!("no audit log yet: nothing has happened on this gate");
                return Ok(());
            }
            let (n, last) = audit::verify_file(&log)?;
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
        Command::Stdio => {
            let gate = Gate::open(s)?;
            let Some(protocol) = adapter.stdio(gate.clone()) else {
                return Err(format!(
                    "{} has no stdio mode: run `{} serve`",
                    product().name,
                    product().bin
                ));
            };
            hint_enroll(&gate, false)?;
            prune_every_minute(&gate);
            // The approval page and the agent side live and die together. If the page
            // cannot run (its address is taken, perhaps by something pretending to
            // be the gate), the agent side must not keep handing out links to it.
            let page = web::serve(gate.clone(), web::router(gate.clone(), None));
            tokio::select! {
                r = page => r.map_err(|e| format!("approval page: {e}")),
                r = protocol => r,
            }
        }
        Command::Serve => {
            let gate = Gate::open(s)?;
            // serve often runs as a service whose output goes to a log file:
            // never print the enrollment link or the access token there.
            hint_enroll(&gate, false)?;
            prune_every_minute(&gate);
            let endpoint = if gate.settings.access_token.is_some() {
                format!(
                    "  agent endpoint  {}{}  (access token: run  {} token)",
                    gate.settings.origin,
                    adapter.path(),
                    product().bin
                )
            } else {
                adapter.connect_hint(&gate.settings)
            };
            eprintln!(
                "{}\n  approval page   {}\n{endpoint}",
                product().bin,
                gate.settings.origin
            );
            let app = app(&gate, &adapter);
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
                "{} demo\n  approval page   {}\n{}\n  demo service    {demo_aud}  (fake deploy, payment, send_message)\n{}",
                product().bin,
                s.origin,
                adapter.connect_hint(&s),
                adapter.demo_hint()
            );
            let app = app(&gate, &adapter);
            web::serve(gate, app).await
        }
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Run the product: parse the command line and do it. Never returns.
pub fn run(p: &'static Product, adapter: impl Adapter) -> ! {
    product::set(p);
    let adapter: Arc<dyn Adapter> = Arc::new(adapter);
    let mut cmd = Cli::command()
        .name(p.bin)
        .bin_name(p.bin)
        .version(p.version)
        .about(p.about)
        .mut_arg("port", |a| {
            a.default_value(leak(p.port.to_string()))
                .env(leak(p.env("PORT")))
        })
        .mut_arg("bind", |a| a.env(leak(p.env("BIND"))))
        .mut_arg("origin", |a| a.env(leak(p.env("ORIGIN"))))
        .mut_arg("home", |a| {
            a.help(leak(format!(
                "Data folder [env: {}] [default: ~/.{}]",
                p.env("HOME"),
                p.bin
            )))
        });
    if !adapter.has_stdio() {
        cmd = cmd.mut_subcommand("stdio", |c| c.hide(true));
    }
    let cli = match Cli::from_arg_matches(&cmd.get_matches()) {
        Ok(c) => c,
        Err(e) => e.exit(),
    };
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("{}: {e}", p.bin);
            std::process::exit(1);
        }
    };
    // Exit right away when done: a stdio reader runs on a blocking thread that
    // would otherwise keep the process alive while its input pipe stays open.
    match rt.block_on(run_cli(cli, adapter)) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("{}: {e}", p.bin);
            std::process::exit(1);
        }
    }
}
