//! Random bytes as the agent's proof for a real bound approval (pka.2), and the
//! real proof with the bytes XORed into it: never panics, never verifies.
#![no_main]
use libfuzzer_sys::fuzz_target;
use phygital_key_verify::bound::Binding;
use phygital_key_verify::{Config, Expected, Keyring, Level, MemorySeenStore, b64, verify_bound};
use serde_json::Value;

const VECTORS: &str = include_str!("../../test-vectors/vectors.json");

fuzz_target!(|data: &[u8]| {
    let Ok(doc) = serde_json::from_str::<Value>(VECTORS) else { return };
    let Some(input) = doc.get("vectors").and_then(Value::as_array).and_then(|a| a.iter().find(|v| v.get("name").and_then(Value::as_str) == Some("valid_bound"))).and_then(|v| v.get("input")) else { return };
    let get = |k: &str| input.get(k).cloned().unwrap_or(Value::Null);
    let keyring: Keyring = serde_json::from_value(get("keyring")).unwrap_or_default();
    let config: Config = serde_json::from_value(get("config")).unwrap_or(Config { origins: vec![], rp_ids: vec![] });
    let request = get("request");
    let aud = get("aud");
    let expected = Expected { aud: aud.as_str().unwrap_or(""), request: &request, level: Level::Medium };
    let now = get("now").as_i64().unwrap_or(0);
    let approval = get("approval").as_str().unwrap_or("").to_string();
    let real = get("proof").as_str().unwrap_or("").to_string();
    let run = |proof: &str| verify_bound(&approval, Binding { proof: Some(proof), required: false }, &expected, &keyring, &MemorySeenStore::new(), &config, now);

    // 1. The bytes themselves as a proof.
    if let Ok(text) = std::str::from_utf8(data) {
        if text != real {
            assert!(run(text).is_err());
        }
    }
    // 2. The real proof's signature or payload with the fuzz bytes XORed in.
    let parts: Vec<&str> = real.split('.').collect();
    let [h, p, s] = parts.as_slice() else { return };
    for (which, part) in [(1usize, *p), (2usize, *s)] {
        let mut raw = b64::decode(part).unwrap_or_default();
        let len = raw.len().max(1);
        for (i, b) in data.iter().enumerate() {
            if let Some(r) = raw.get_mut(i % len) { *r ^= b; }
        }
        let changed = b64::encode(&raw);
        if changed == part { continue; }
        let mutated = if which == 1 { format!("{h}.{changed}.{s}") } else { format!("{h}.{p}.{changed}") };
        assert!(run(&mutated).is_err());
    }
});
