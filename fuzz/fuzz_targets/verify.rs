//! Random bytes as the approval, and as a mutation of a real one: never panics, never verifies.
#![no_main]
use libfuzzer_sys::fuzz_target;
use phygital_key_verify::{Config, Expected, Keyring, Level, MemorySeenStore, b64, verify};
use serde_json::Value;

const VECTORS: &str = include_str!("../../test-vectors/vectors.json");

fuzz_target!(|data: &[u8]| {
    let Ok(doc) = serde_json::from_str::<Value>(VECTORS) else { return };
    let Some(input) = doc.get("vectors").and_then(|v| v.get(0)).and_then(|v| v.get("input")) else { return };
    let get = |k: &str| input.get(k).cloned().unwrap_or(Value::Null);
    let keyring: Keyring = serde_json::from_value(get("keyring")).unwrap_or_default();
    let config: Config = serde_json::from_value(get("config")).unwrap_or(Config { origins: vec![], rp_ids: vec![] });
    let request = get("request");
    let aud = get("aud");
    let expected = Expected { aud: aud.as_str().unwrap_or(""), request: &request, level: Level::Medium };
    let now = get("now").as_i64().unwrap_or(0);
    let real = get("approval").as_str().unwrap_or("").to_string();

    // 1. The bytes themselves as an approval string.
    if let Ok(text) = std::str::from_utf8(data) {
        if text != real {
            assert!(verify(text, &expected, &keyring, &MemorySeenStore::new(), &config, now).is_err());
        }
    }
    // 2. The real approval with the fuzz bytes XORed into it.
    let mut raw = b64::decode(&real).unwrap_or_default();
    let mut changed = false;
    let len = raw.len().max(1);
    for (i, b) in data.iter().enumerate() {
        if let Some(r) = raw.get_mut(i % len) {
            if *b != 0 { changed = true; }
            *r ^= b;
        }
    }
    if changed && b64::encode(&raw) != real {
        let mutated = b64::encode(&raw);
        assert!(verify(&mutated, &expected, &keyring, &MemorySeenStore::new(), &config, now).is_err());
    }
});
