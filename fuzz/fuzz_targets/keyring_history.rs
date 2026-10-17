//! Random keyrings never panic the history check, and never pass for a random root.
#![no_main]
use libfuzzer_sys::fuzz_target;
use phygital_key_verify::{Keyring, history};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(ring) = serde_json::from_str::<Keyring>(text) {
            assert!(history::verify_history(&ring, "not-a-real-fingerprint").is_err());
        }
    }
});
