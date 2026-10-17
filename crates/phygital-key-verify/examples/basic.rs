//! Verify the first shared test vector, the way a service would.
//! Run: cargo run -p phygital-key-verify --example basic

use phygital_key_verify::{Config, Expected, Keyring, Level, MemorySeenStore, verify};
use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc: Value = serde_json::from_str(include_str!("../../../test-vectors/vectors.json"))?;
    let input = doc
        .get("vectors")
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("input"))
        .ok_or("no vector")?;
    let get = |k: &str| input.get(k).cloned().unwrap_or(Value::Null);

    // What a service keeps: its trusted keys, where approvals may come from, used ids.
    let keyring: Keyring = serde_json::from_value(get("keyring"))?;
    let config: Config = serde_json::from_value(get("config"))?;
    let seen = MemorySeenStore::new();

    // What arrives with the action: the approval, and the request it is about to run.
    let approval = get("approval");
    let request = get("request");
    let aud = get("aud");
    let expected = Expected {
        aud: aud.as_str().unwrap_or(""),
        request: &request,
        level: Level::Medium,
    };
    let now = get("now").as_i64().unwrap_or(0); // a real service passes the current time

    match verify(
        approval.as_str().unwrap_or(""),
        &expected,
        &keyring,
        &seen,
        &config,
        now,
    ) {
        Ok(ok) => println!("approved by {} (id {}): safe to act", ok.approver, ok.id),
        Err(code) => println!("refused: {code}"),
    }
    // The same approval a second time:
    match verify(
        approval.as_str().unwrap_or(""),
        &expected,
        &keyring,
        &seen,
        &config,
        now,
    ) {
        Ok(_) => println!("approved again (this must never happen)"),
        Err(code) => println!("second use refused: {code}"),
    }
    Ok(())
}
