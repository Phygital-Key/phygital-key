//! Random standing approvals and use tokens: never panic, never verify.
#![no_main]
use libfuzzer_sys::fuzz_target;
use phygital_key_verify::standing::{MemoryUseStore, Revocations, verify_use};
use phygital_key_verify::{Config, Expected, Keyring, Level};
use serde_json::{Value, json};

const STANDING: &str = include_str!("../../test-vectors/standing.json");

fuzz_target!(|data: &[u8]| {
    let Ok(doc) = serde_json::from_str::<Value>(STANDING) else { return };
    let Some(input) = doc.get("vectors").and_then(|v| v.get(0)).and_then(|v| v.get("input")) else { return };
    let keyring: Keyring = serde_json::from_value(input.get("keyring").cloned().unwrap_or(Value::Null)).unwrap_or_default();
    let config: Config = serde_json::from_value(input.get("config").cloned().unwrap_or(Value::Null))
        .unwrap_or(Config { origins: vec![], rp_ids: vec![] });
    let real_standing = input.get("standing").and_then(Value::as_str).unwrap_or("");
    let real_use = input.get("use").and_then(Value::as_str).unwrap_or("");
    let now = input.get("now").and_then(Value::as_i64).unwrap_or(0);
    let request = json!({"type": "payment", "params": {"to": "ACME Ltd", "currency": "EUR", "amount_cents": 450, "memo": "coffee"}});
    let expected = Expected { aud: "https://deploy.example.com", request: &request, level: Level::High };
    let Ok(text) = std::str::from_utf8(data) else { return };
    // Random use token with the real standing approval, and the reverse.
    if text != real_use {
        assert!(verify_use(real_standing, text, &expected, &keyring, &MemoryUseStore::new(), &Revocations::none(), &config, now).is_err());
    }
    if text != real_standing {
        assert!(verify_use(text, real_use, &expected, &keyring, &MemoryUseStore::new(), &Revocations::none(), &config, now).is_err());
    }
});
