//! Strict parsing and the canonical form: never panic; accepted input is stable.
#![no_main]
use libfuzzer_sys::fuzz_target;
use phygital_key_verify::{canonical, strict};

fuzz_target!(|data: &[u8]| {
    if let Some(v) = strict::parse(data) {
        if let Ok(c) = canonical::canonical_string(&v) {
            let back = strict::parse(c.as_bytes()).expect("canonical output parses");
            assert_eq!(canonical::canonical_string(&back).ok(), Some(c));
        }
    }
});
