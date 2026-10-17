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

#[test]
fn receipt_reference_known_answer() {
    // base64url SHA-256 of "abc".
    assert_eq!(
        phygital_key_verify::receipt::reference_of("abc"),
        "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0"
    );
}

#[test]
fn receipt_peek_and_display() {
    use phygital_key_verify::receipt::{ReceiptError, peek};
    assert_eq!(peek("not.a.receipt"), None);
    assert_eq!(peek(&"a.".repeat(3000)), None);
    let body = phygital_key_verify::b64::encode(
        br#"{"v":"pka.receipt.1","iss":"s","approval_id":"i","reference":"r","request_hash":"h","status":"error","error":"x","iat":1}"#,
    );
    let r = peek(&format!("h.{body}.s"));
    assert_eq!(r.map(|r| r.status), Some("error".to_string()));
    assert_eq!(ReceiptError::Malformed.to_string(), "MALFORMED");
    assert_eq!(ReceiptError::BadSignature.to_string(), "BAD_SIGNATURE");
    assert_eq!(ReceiptError::Mismatch.to_string(), "MISMATCH");
}

#[test]
fn standing_params_rules() {
    use phygital_key_verify::standing::StandingParams;
    let key = alice().public_key;
    let req = |vary: Value, max_uses: u32, for_type: &str, fixed: Value| json!({"type": "standing", "params": {"for_type": for_type, "fixed": fixed, "vary": vary, "max_uses": max_uses, "use_key": key}});
    let ok = |r: &Value| StandingParams::from_request(r).is_ok();
    assert!(ok(&req(
        json!({"a": {"min": 1, "max": 2}}),
        1,
        "payment",
        json!({})
    )));
    assert!(ok(&req(
        json!({"a": {"min": 2, "max": 2}}),
        1000,
        "payment",
        json!({})
    )));
    assert!(
        !ok(&req(
            json!({"a": {"min": 3, "max": 2}}),
            1,
            "payment",
            json!({})
        )),
        "min above max"
    );
    assert!(
        !ok(&req(json!({"a": {"max": 2}}), 1, "payment", json!({}))),
        "min required"
    );
    assert!(
        !ok(&req(
            json!({"a": {"min": 1, "max": 2, "x": 1}}),
            1,
            "payment",
            json!({})
        )),
        "extra field in limit"
    );
    assert!(
        !ok(&req(json!({"a": {"one_of": []}}), 1, "payment", json!({}))),
        "empty list"
    );
    let twenty: Vec<String> = (0..20).map(|i| i.to_string()).collect();
    let twenty_one: Vec<String> = (0..21).map(|i| i.to_string()).collect();
    assert!(ok(&req(
        json!({"a": {"one_of": twenty}}),
        1,
        "payment",
        json!({})
    )));
    assert!(
        !ok(&req(
            json!({"a": {"one_of": twenty_one}}),
            1,
            "payment",
            json!({})
        )),
        "21 strings"
    );
    assert!(!ok(&req(json!({}), 0, "payment", json!({}))), "max_uses 0");
    assert!(
        !ok(&req(json!({}), 1001, "payment", json!({}))),
        "max_uses 1001"
    );
    assert!(
        !ok(&req(json!({}), 1, "standing", json!({}))),
        "standing of standing"
    );
    assert!(!ok(&req(json!({}), 1, "Pay", json!({}))), "bad type name");
    assert!(
        !ok(&req(
            json!({"a": {"min": 1, "max": 2}}),
            1,
            "payment",
            json!({"a": 1})
        )),
        "fixed and vary overlap"
    );
    let mut bad_key = req(json!({}), 1, "payment", json!({}));
    bad_key["params"]["use_key"] = json!("AAAA");
    assert!(!ok(&bad_key), "bad use key");
    assert!(
        !ok(&json!({"type": "payment", "params": {}})),
        "not a standing request"
    );
}
