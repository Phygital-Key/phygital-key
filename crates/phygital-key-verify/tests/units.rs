//! Small exact checks, added where mutation testing showed a change would go unnoticed.
// serde_json::Value indexing returns Null for missing keys; it never panics.
#![allow(clippy::indexing_slicing)]

use phygital_key_verify::history::{self, add_key_request, remove_key_request};
use phygital_key_verify::{
    Config, ErrorCode, KeyCard, Keyring, Level, Verifier, canonical, id_ok, request_shape_ok,
    strict,
};
use serde_json::{Value, json};

const VECTORS: &str = include_str!("../../../test-vectors/vectors.json");
const KEYRINGS: &str = include_str!("../../../test-vectors/keyrings.json");

fn alice() -> KeyCard {
    let doc: Value = serde_json::from_str(VECTORS).unwrap_or_default();
    serde_json::from_value(doc["vectors"][0]["input"]["keyring"]["keys"][0].clone()).unwrap_or_else(
        |_| KeyCard {
            credential_id: String::new(),
            public_key: String::new(),
            rp_id: String::new(),
            name: String::new(),
            created: String::new(),
        },
    )
}

#[test]
fn request_hash_known_answer() {
    // SHA-256 of {"params":{},"type":"x"}, base64url.
    assert_eq!(
        canonical::hash_b64(&json!({"type": "x", "params": {}})),
        Ok("nsm6iVybCMIAEc4KNTIO5aMbJp8YSn6qy09KPSqCiEw".to_string())
    );
}

#[test]
fn huge_integers_are_refused() {
    assert!(canonical::validate(&json!(u64::MAX)).is_err());
    assert!(canonical::validate(&json!(9_007_199_254_740_991_u64)).is_ok());
    assert!(canonical::validate(&json!(-9_007_199_254_740_992_i64)).is_err());
}

#[test]
fn level_names() {
    assert_eq!(Level::Medium.as_str(), "medium");
    assert_eq!(Level::High.as_str(), "high");
    assert_eq!(Level::parse("high"), Some(Level::High));
    assert_eq!(Level::parse("low"), None);
    assert!(Level::High > Level::Medium);
}

#[test]
fn error_codes_print_as_wire_names() {
    assert_eq!(ErrorCode::RequestMismatch.to_string(), "REQUEST_MISMATCH");
    assert_eq!(ErrorCode::ApprovalRequired.to_string(), "APPROVAL_REQUIRED");
    assert_eq!(
        history::HistoryError::BadEntry(1, "x").to_string(),
        "history entry 2: x"
    );
}

#[test]
fn keyring_from_json_reads_and_checks_keys() {
    let card = alice();
    let text = json!({"keys": [card]}).to_string();
    let ring = Keyring::from_json(&text).unwrap_or_default();
    assert_eq!(ring.keys.len(), 1);
    assert!(ring.find(&card.credential_id).is_some());
    let mut bad = card.clone();
    bad.public_key = "AAAA".into();
    assert!(Keyring::from_json(&json!({"keys": [bad]}).to_string()).is_err());
    assert!(Keyring::from_json(r#"{"keys": [], "extra": 1}"#).is_err());
}

#[test]
fn key_card_params_round_trip() {
    let card = alice();
    let params = card.to_params();
    assert_eq!(params["credential_id"], json!(card.credential_id));
    assert_eq!(KeyCard::from_params(&params), Some(card.clone()));
    assert_eq!(card.fingerprint().len(), 43);
    assert_eq!(add_key_request(&card)["type"], json!("add_key"));
    assert_eq!(add_key_request(&card)["params"], params);
    assert_eq!(
        remove_key_request(&card),
        json!({"type": "remove_key", "params": {"credential_id": card.credential_id, "name": card.name}})
    );
}

#[test]
fn request_shapes() {
    let ok = |t: &str| request_shape_ok(&json!({"type": t, "params": {}}));
    assert!(ok("deploy"));
    assert!(ok("send_email.v2-beta"));
    assert!(!ok(""));
    assert!(!ok("Deploy"));
    assert!(!ok("de ploy"));
    assert!(!ok("dé"));
    assert!(!ok("d/x"));
    assert!(ok(&"a".repeat(64)));
    assert!(!ok(&"a".repeat(65)));
    assert!(!request_shape_ok(&json!({"type": "x", "params": []})));
    assert!(!request_shape_ok(&json!({"type": "x"})));
    assert!(!request_shape_ok(
        &json!({"type": "x", "params": {}, "z": 1})
    ));
    assert!(!request_shape_ok(&json!({"kind": "x", "params": {}})));
    assert!(!request_shape_ok(&json!(["x"])));
}

#[test]
fn ids() {
    assert!(id_ok("BwcHBwcHBwcHBwcHBwcHBw"));
    assert!(!id_ok("!!!!!!!!!!!!!!!!!!!!!!"));
    assert!(!id_ok("BwcHBwcHBwcHBwcHBwcHB"));
    assert!(!id_ok("BwcHBwcHBwcHBwcHBwcHBwc"));
}

#[test]
fn depth_counting() {
    // Escaped quotes do not end a string; brackets after them are still inside it.
    let s = format!(r#"{{"a":"\"{}"}}"#, "[".repeat(40));
    assert!(strict::parse(s.as_bytes()).is_some());
    // Backslash escapes: an escaped backslash ends the escape.
    let t = format!(r#"{{"a":"\\","b":{}{}}}"#, "[".repeat(16), "]".repeat(16));
    assert!(
        strict::parse(t.as_bytes()).is_none(),
        "depth 17 after an escaped backslash"
    );
    // Siblings do not add up: closing brackets lower the depth.
    let siblings = format!("[{}]", vec!["[[]]"; 50].join(","));
    assert!(strict::parse(siblings.as_bytes()).is_some());
    assert_eq!(strict::depth(siblings.as_bytes()), 3);
}

#[test]
fn verifier_set_keyring_takes_effect() {
    let doc: Value = serde_json::from_str(VECTORS).unwrap_or_default();
    let config: Config = serde_json::from_value(doc["vectors"][0]["input"]["config"].clone())
        .unwrap_or(Config {
            origins: vec![],
            rp_ids: vec![],
        });
    let mut v = Verifier::new("https://deploy.example.com", Keyring::default(), config);
    let request = doc["vectors"][0]["input"]["request"].clone();
    let approval = doc["vectors"][0]["input"]["approval"]
        .as_str()
        .unwrap_or("")
        .to_string();
    // The fixed vector time is in the past, so a live Verifier sees it as expired either way;
    // with an empty keyring it must not get as far as checking the key.
    assert!(v.verify(&approval, &request, Level::Medium).is_err());
    v.set_keyring(Keyring::of(vec![alice()]));
    assert_eq!(
        v.verify(&approval, &request, Level::Medium),
        Err(ErrorCode::Expired)
    );
}

#[test]
fn root_fingerprint_reads_the_first_entry() {
    let doc: Value = serde_json::from_str(KEYRINGS).unwrap_or_default();
    let ring: Keyring =
        serde_json::from_value(doc["vectors"][0]["keyring"].clone()).unwrap_or_default();
    assert_eq!(
        history::root_fingerprint(&ring),
        Some(alice().fingerprint())
    );
    assert_eq!(history::root_fingerprint(&Keyring::default()), None);
}
