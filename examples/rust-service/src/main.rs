//! A database worker that runs a job only with a person's valid approval.
//!
//! It reads one job per line on stdin, as JSON:
//!   {"approval": "<approval string>", "request": {"type": "delete_rows", "params": {...}}}
//! and prints one JSON result per line. Nothing real is deleted: it is an example.
//!
//! In a real service the request comes from your own job queue or database,
//! never from the approval. Here the job line plays that part.
//!
//! Env: KEYRING (required), AUD (default "worker:db-maintenance"),
//!      GATE_ORIGIN (default "http://localhost:7468").

use std::io::{BufRead, Write};

use phygital_key_verify::{Config, Keyring, Level, Verifier};
use serde_json::{Value, json};

/// The worker decides the level for each job type, never the caller.
fn level_for(job_type: &str) -> Option<Level> {
    match job_type {
        "delete_rows" | "drop_table" => Some(Level::High),
        "vacuum" => Some(Level::Medium),
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keyring_path = std::env::var("KEYRING").map_err(|_| "set KEYRING to the keyring file")?;
    let aud = std::env::var("AUD").unwrap_or_else(|_| "worker:db-maintenance".into());
    let origin = std::env::var("GATE_ORIGIN").unwrap_or_else(|_| "http://localhost:7468".into());
    // The relying party id is the host name of the approval page origin.
    let host = origin
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split([':', '/']).next())
        .ok_or("GATE_ORIGIN must look like https://approve.example.com")?
        .to_string();

    // Keys are read once at start: restart the worker after the keyring changes.
    let keyring = Keyring::from_json(&std::fs::read_to_string(&keyring_path)?)?;
    let config = Config { origins: vec![origin.clone()], rp_ids: vec![host] };
    // In-memory seen store: each approval works once while this process runs.
    // With several workers, use Verifier::with_store and a shared, durable store.
    let verifier = Verifier::new(aud.clone(), keyring, config);
    eprintln!("worker ready: aud {aud}, approvals from {origin}; one JSON job per line");

    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let result = handle(&verifier, &line);
        let mut out = stdout.lock();
        writeln!(out, "{result}")?;
        out.flush()?;
    }
    Ok(())
}

fn handle(verifier: &Verifier, line: &str) -> Value {
    let Ok(job) = serde_json::from_str::<Value>(line) else {
        return json!({"ok": false, "error": "BAD_JOB"});
    };
    let approval = job.get("approval").and_then(Value::as_str).unwrap_or("");
    let request = job.get("request").cloned().unwrap_or(Value::Null);
    let job_type = request.get("type").and_then(Value::as_str).unwrap_or("");
    let Some(level) = level_for(job_type) else {
        return json!({"ok": false, "error": "UNKNOWN_JOB"});
    };
    match verifier.verify(approval, &request, level) {
        Ok(ok) => {
            // ...a real worker runs the job here, then logs id, request_hash, approver, result.
            json!({
                "ok": true,
                "done": format!("{job_type} (example: nothing real happened)"),
                "approval_id": ok.id,
                "request_hash": ok.request_hash,
                "approved_by": ok.approver,
            })
        }
        Err(code) => json!({"ok": false, "error": code.as_str()}),
    }
}
