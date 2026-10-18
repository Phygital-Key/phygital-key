//! Exact edges that mutation testing (cargo-mutants) showed no test pinned down:
//! size limits at the limit itself, store memory at the exact second it lets go,
//! revocation lists read from JSON, a keyring swapped into a live verifier, and
//! authenticator data one byte too short.
// serde_json::Value indexing returns Null for missing keys; it never panics.
#![allow(clippy::indexing_slicing)]

use p256::ecdsa::signature::Signer as _;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey as _;
use phygital_key_verify::receipt::{self, Receipt, ReceiptError};
use phygital_key_verify::standing::{MemoryUseStore, Revocations, UseStore};
use phygital_key_verify::webauthn::{AssertionCheck, AssertionError, verify_assertion};
use phygital_key_verify::{
    Config, ErrorCode, KeyCard, Keyring, Level, MemorySeenStore, SeenStore, Verifier, b64,
    canonical,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const STANDING: &str = include_str!("../../../test-vectors/standing.json");
const RP_ID: &str = "approve.example.com";
const ORIGIN: &str = "https://approve.example.com";
const AUD: &str = "https://deploy.example.com";

fn key(seed: u8) -> SigningKey {
    SigningKey::from_slice(&[seed; 32]).unwrap_or_else(|_| unreachable!("valid scalar"))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Authenticator data for RP_ID with the given flags and `extra` bytes after them.
fn auth_data(flags: u8, extra: usize) -> Vec<u8> {
    let mut a = Sha256::digest(RP_ID.as_bytes()).to_vec();
    a.push(flags);
    a.extend(std::iter::repeat_n(0u8, extra));
    a
}

/// One ES256 assertion over `auth` for `challenge`: (client data JSON, DER signature).
fn assert_over(k: &SigningKey, auth: &[u8], challenge: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let cd = json!({
        "type": "webauthn.get",
        "challenge": b64::encode(challenge),
        "origin": ORIGIN,
        "crossOrigin": false
    })
    .to_string()
    .into_bytes();
    let mut signed = auth.to_vec();
    signed.extend_from_slice(&Sha256::digest(&cd));
    let sig: Signature = k.sign(&signed);
    (cd, sig.to_der().as_bytes().to_vec())
}

fn config() -> Config {
    Config {
        origins: vec![ORIGIN.into()],
        rp_ids: vec![RP_ID.into()],
    }
}

fn card(k: &SigningKey) -> KeyCard {
    let der = k
        .verifying_key()
        .to_public_key_der()
        .unwrap_or_else(|_| unreachable!("P-256 key"));
    serde_json::from_value(json!({
        "credential_id": b64::encode([9u8; 16]),
        "public_key": b64::encode(der.as_bytes()),
        "rp_id": RP_ID,
        "name": "edge test key (TEST ONLY)",
        "created": "2026-10-18"
    }))
    .unwrap_or_else(|_| unreachable!("valid card"))
}

/// A medium approval for `request`, made now, valid for 60 s.
fn fresh_approval(k: &SigningKey, request: &Value) -> String {
    let t = now();
    let claims = json!({
        "v": "pka.1",
        "id": b64::encode([3u8; 16]),
        "aud": AUD,
        "request": request,
        "request_hash": canonical::hash_b64(request).unwrap_or_default(),
        "level": "medium",
        "iat": t,
        "exp": t + 60
    });
    let challenge = canonical::challenge(&claims).unwrap_or_default();
    let auth = auth_data(0x01, 4);
    let (cd, der) = assert_over(k, &auth, &challenge);
    let obj = json!({"claims": claims, "signatures": [{
        "credential_id": b64::encode([9u8; 16]),
        "authenticator_data": b64::encode(&auth),
        "client_data_json": b64::encode(&cd),
        "signature": b64::encode(&der)
    }]});
    b64::encode(obj.to_string().as_bytes())
}

#[test]
fn authenticator_data_one_byte_short_is_refused() {
    let k = key(5);
    let challenge = [1u8; 32];
    let check = |auth: &[u8]| {
        let (cd, der) = assert_over(&k, auth, &challenge);
        let origins = [ORIGIN.to_string()];
        let want = AssertionCheck {
            kind: "webauthn.get",
            challenge: &challenge,
            origins: &origins,
            rp_id: RP_ID,
        };
        verify_assertion(k.verifying_key(), auth, &cd, &der, &want)
    };
    // 32-byte RP hash, flags, 4-byte counter: 37 bytes is the shortest valid form.
    assert_eq!(check(&auth_data(0x01, 4)), Ok(0x01));
    assert_eq!(
        check(&auth_data(0x01, 3)),
        Err(AssertionError::BadSignature),
        "36 bytes, correctly signed, is still refused"
    );
}

#[test]
fn set_keyring_changes_what_a_live_verifier_trusts() {
    let k = key(6);
    let request = json!({"type": "deploy", "params": {"service": "api"}});
    let approval = fresh_approval(&k, &request);
    let mut v = Verifier::new(AUD, Keyring::default(), config());
    assert_eq!(
        v.verify(&approval, &request, Level::Medium).map(|_| ()),
        Err(ErrorCode::UntrustedKey)
    );
    v.set_keyring(Keyring::of(vec![card(&k)]));
    assert!(v.verify(&approval, &request, Level::Medium).is_ok());
}

#[test]
fn seen_ids_are_kept_until_30_s_after_expiry_then_let_go() {
    let s = MemorySeenStore::new();
    assert!(s.insert_if_new("a", 100, 50));
    assert!(
        !s.insert_if_new("a", 100, 129),
        "still remembered at exp + 29"
    );
    assert!(s.insert_if_new("a", 100, 130), "let go at exp + 30");
}

#[test]
fn use_counts_are_kept_until_30_s_after_the_end_then_let_go() {
    let s = MemoryUseStore::new();
    assert_eq!(s.record_use("s", "u1", 5, 100, 50), Ok(4));
    assert_eq!(
        s.record_use("s", "u1", 5, 100, 129),
        Err(ErrorCode::Replayed),
        "still remembered at the end + 29"
    );
    assert_eq!(
        s.record_use("s", "u1", 5, 100, 130),
        Ok(4),
        "let go at the end + 30"
    );
}

/// A receipt whose compact form is exactly `len` characters, signed by `k`.
fn receipt_of_length(k: &SigningKey, len: usize) -> Option<String> {
    (0..4096).find_map(|pad| {
        let r = Receipt {
            v: "pka.receipt.1".into(),
            iss: AUD.into(),
            approval_id: b64::encode([3u8; 16]),
            reference: receipt::reference_of("x"),
            request_hash: "h".into(),
            status: "error".into(),
            error: Some("E".repeat(pad)),
            use_id: None,
            iat: 1,
        };
        receipt::sign(&r, k).ok().filter(|j| j.len() == len)
    })
}

#[test]
fn a_receipt_of_exactly_4096_characters_is_read() {
    let k = key(7);
    let Some(jws) = receipt_of_length(&k, 4096) else {
        unreachable!("some padding gives exactly 4096")
    };
    assert!(receipt::peek(&jws).is_some(), "peek reads it");
    // It then fails on the approval it names, never on its size.
    assert_ne!(
        receipt::verify(&jws, k.verifying_key(), AUD, "x").err(),
        Some(ReceiptError::Malformed)
    );
    if let Some(longer) = receipt_of_length(&k, 4097) {
        assert!(receipt::peek(&longer).is_none());
        assert_eq!(
            receipt::verify(&longer, k.verifying_key(), AUD, "x").err(),
            Some(ReceiptError::Malformed)
        );
    }
}

#[test]
fn revocations_read_from_json_count_and_merge() {
    let doc: Value = serde_json::from_str(STANDING).unwrap_or_default();
    let Some(v) = doc["vectors"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["name"] == json!("revoked")))
    else {
        unreachable!("vector `revoked`")
    };
    let input = &v["input"];
    let keyring: Keyring = serde_json::from_value(input["keyring"].clone()).unwrap_or_default();
    let cfg: Config = serde_json::from_value(input["config"].clone()).unwrap_or_else(|_| config());
    let aud = input["aud"].as_str().unwrap_or("");
    let text = json!({"revocations": input["revocations"]}).to_string();
    let list = Revocations::from_json(&text, aud, &keyring, &cfg);
    assert_eq!(list.len(), 1);
    assert!(!list.is_empty());
    let none = Revocations::none();
    assert_eq!(none.len(), 0);
    assert!(none.is_empty());
    let standing_id = list_standing_id(input);
    assert!(list.contains(&standing_id));
    let mut merged = Revocations::none();
    merged.merge(list);
    assert!(
        merged.contains(&standing_id),
        "merge keeps what was accepted"
    );
    assert!(Revocations::from_json("not json", aud, &keyring, &cfg).is_empty());
}

fn list_standing_id(input: &Value) -> String {
    input["standing"]
        .as_str()
        .and_then(b64::decode)
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["claims"]["id"].as_str().map(String::from))
        .unwrap_or_default()
}
