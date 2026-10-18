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
        let binding = phygital_key_verify::bound::Binding {
            proof: input["proof"].as_str(),
            required: input["require_binding"].as_bool().unwrap_or(false),
        };
        let got = phygital_key_verify::verify_bound(
            input["approval"].as_str().unwrap_or(""),
            binding,
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
        if matches!(
            code,
            phygital_key_verify::ErrorCode::Denied
                | phygital_key_verify::ErrorCode::NotAllowed
                | phygital_key_verify::ErrorCode::UsedUp
                | phygital_key_verify::ErrorCode::Revoked
        ) {
            continue; // gates, or standing approvals (covered in standing.json)
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

const KEYRINGS: &str = include_str!("../../../test-vectors/keyrings.json");

#[test]
fn every_keyring_vector_passes() -> Result<(), String> {
    let doc: Value = serde_json::from_str(KEYRINGS).map_err(|e| e.to_string())?;
    let mut failures = Vec::new();
    for v in doc["vectors"].as_array().ok_or("no vectors")? {
        let ring: Keyring =
            serde_json::from_value(v["keyring"].clone()).map_err(|e| e.to_string())?;
        let origin = v["origin"]
            .as_str()
            .unwrap_or("https://approve.example.com");
        let got = match v.get("accepted").filter(|a| !a.is_null()) {
            Some(acc) => {
                let accepted: Keyring =
                    serde_json::from_value(acc.clone()).map_err(|e| e.to_string())?;
                phygital_key_verify::history::verify_history_update(
                    &accepted,
                    &ring,
                    v["root_fingerprint"].as_str().unwrap_or(""),
                    origin,
                )
            }
            None => phygital_key_verify::history::verify_history(
                &ring,
                v["root_fingerprint"].as_str().unwrap_or(""),
                origin,
            ),
        };
        let want = v["expected"]["error"].as_str();
        let pass = match &got {
            Ok(()) => want.is_none(),
            Err(e) => want == Some(e.code()),
        };
        if !pass {
            failures.push(format!(
                "{}: want {:?}, got {:?}",
                v["name"],
                want.unwrap_or("ok"),
                got
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

const STANDING: &str = include_str!("../../../test-vectors/standing.json");

#[test]
fn every_standing_vector_passes() -> Result<(), String> {
    use phygital_key_verify::standing::{MemoryUseStore, Revocations, UseStore, verify_use};
    let doc: Value = serde_json::from_str(STANDING).map_err(|e| e.to_string())?;
    let mut failures = Vec::new();
    for v in doc["vectors"].as_array().ok_or("no vectors")? {
        let input = &v["input"];
        let keyring: Keyring =
            serde_json::from_value(input["keyring"].clone()).map_err(|e| e.to_string())?;
        let config: Config =
            serde_json::from_value(input["config"].clone()).map_err(|e| e.to_string())?;
        let level = Level::parse(input["level"].as_str().unwrap_or("")).ok_or("bad level")?;
        let now = input["now"].as_i64().ok_or("bad now")?;
        let standing = input["standing"].as_str().unwrap_or("");
        // The standing id is fixed by the generator: [40; 16].
        let sid = phygital_key_verify::b64::encode([40u8; 16]);
        let uses = MemoryUseStore::new();
        for id in input["prior_uses"].as_array().into_iter().flatten() {
            let _ = uses.record_use(&sid, id.as_str().unwrap_or(""), 3, i64::MAX, now);
        }
        let expected = Expected {
            aud: input["aud"].as_str().unwrap_or(""),
            request: &input["request"],
            level,
        };
        let list: Vec<String> =
            serde_json::from_value(input["revocations"].clone()).unwrap_or_default();
        let revoked = Revocations::from_approvals(&list, expected.aud, &keyring, &config);
        let got = verify_use(
            standing,
            input["use"].as_str().unwrap_or(""),
            &expected,
            &keyring,
            &uses,
            &revoked,
            &config,
            now,
        );
        let want = v["expected"]["error"].as_str();
        let pass = match &got {
            Ok(_) => want.is_none(),
            Err(code) => want == Some(code.as_str()),
        };
        if !pass {
            failures.push(format!(
                "{}: want {:?}, got {:?}",
                v["name"],
                want.unwrap_or("ok"),
                got.map(|_| "ok")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}
