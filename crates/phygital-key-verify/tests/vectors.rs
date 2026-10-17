//! Runs every shared test vector. Test vectors are law.
// serde_json::Value indexing returns Null for missing keys; it never panics.
#![allow(clippy::indexing_slicing)]

use phygital_key_verify::{Config, Expected, Keyring, Level, MemorySeenStore, SeenStore, verify};
use serde_json::Value;

const VECTORS: &str = include_str!("../../../test-vectors/vectors.json");

#[test]
fn every_vector_passes() -> Result<(), String> {
    let doc: Value = serde_json::from_str(VECTORS).map_err(|e| e.to_string())?;
    let vectors = doc["vectors"].as_array().ok_or("no vectors")?;
    assert!(vectors.len() >= 25, "need at least 25 vectors");
    let mut failures = Vec::new();
    for vec in vectors {
        let name = vec["name"].as_str().unwrap_or("?");
        let input = &vec["input"];
        let keyring: Keyring =
            serde_json::from_value(input["keyring"].clone()).map_err(|e| e.to_string())?;
        let config: Config =
            serde_json::from_value(input["config"].clone()).map_err(|e| e.to_string())?;
        let level = Level::parse(input["level"].as_str().unwrap_or("")).ok_or("bad level")?;
        let now = input["now"].as_i64().ok_or("bad now")?;
        let seen = MemorySeenStore::new();
        for id in input["seen_ids"].as_array().into_iter().flatten() {
            seen.insert_if_new(id.as_str().unwrap_or(""), i64::MAX, now);
        }
        let expected = Expected {
            aud: input["aud"].as_str().unwrap_or(""),
            request: &input["request"],
            level,
        };
        let got = verify(
            input["approval"].as_str().unwrap_or(""),
            &expected,
            &keyring,
            &seen,
            &config,
            now,
        );
        let want_ok = vec["expected"]["ok"].as_bool().unwrap_or(false);
        let want_err = vec["expected"]["error"].as_str();
        let pass = match &got {
            Ok(_) => want_ok,
            Err(code) => !want_ok && Some(code.as_str()) == want_err,
        };
        if !pass {
            failures.push(format!(
                "{name}: want {:?}, got {:?}",
                want_err.unwrap_or("ok"),
                got.map(|_| "ok")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

#[test]
fn every_refusal_code_is_covered() -> Result<(), String> {
    let doc: Value = serde_json::from_str(VECTORS).map_err(|e| e.to_string())?;
    let codes: Vec<&str> = doc["vectors"]
        .as_array()
        .ok_or("no vectors")?
        .iter()
        .filter_map(|v| v["expected"]["error"].as_str())
        .collect();
    for code in phygital_key_verify::ErrorCode::ALL {
        if code == phygital_key_verify::ErrorCode::Denied {
            continue; // returned by gates, never by verify
        }
        assert!(codes.contains(&code.as_str()), "no vector for {code}");
    }
    Ok(())
}

#[test]
fn a_valid_approval_works_once() -> Result<(), String> {
    let doc: Value = serde_json::from_str(VECTORS).map_err(|e| e.to_string())?;
    let vec = doc["vectors"]
        .as_array()
        .ok_or("no vectors")?
        .first()
        .ok_or("empty")?
        .clone();
    let input = &vec["input"];
    let keyring: Keyring =
        serde_json::from_value(input["keyring"].clone()).map_err(|e| e.to_string())?;
    let config: Config =
        serde_json::from_value(input["config"].clone()).map_err(|e| e.to_string())?;
    let seen = MemorySeenStore::new();
    let expected = Expected {
        aud: input["aud"].as_str().unwrap_or(""),
        request: &input["request"],
        level: Level::Medium,
    };
    let now = input["now"].as_i64().unwrap_or(0);
    let approval = input["approval"].as_str().unwrap_or("");
    assert!(verify(approval, &expected, &keyring, &seen, &config, now).is_ok());
    assert_eq!(
        verify(approval, &expected, &keyring, &seen, &config, now).err(),
        Some(phygital_key_verify::ErrorCode::Replayed)
    );
    Ok(())
}
