//! Google's Wycheproof ECDSA P-256 / SHA-256 cases, run through the
//! exact calls the verifier makes: DER signatures as in WebAuthn
//! assertions, raw r||s signatures as in receipts and use tokens.
// serde_json::Value indexing returns Null for missing keys; it never panics.
#![allow(clippy::indexing_slicing)]

use p256::ecdsa::Signature;
use p256::ecdsa::signature::Verifier as _;
use phygital_key_verify::webauthn::parse_public_key;
use serde_json::Value;

const DER: &str = include_str!("../../../test-vectors/wycheproof/ecdsa_secp256r1_sha256_test.json");
const RAW: &str =
    include_str!("../../../test-vectors/wycheproof/ecdsa_secp256r1_sha256_p1363_test.json");

fn hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| s.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
        .collect()
}

fn b64url(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Runs one file. `parse` turns signature bytes into a signature the way the verifier does.
fn run(file: &str, parse: fn(&[u8]) -> Option<Signature>) -> Result<(), String> {
    let doc: Value = serde_json::from_str(file).map_err(|e| e.to_string())?;
    let mut failures = Vec::new();
    let mut count = 0;
    for group in doc["testGroups"].as_array().ok_or("no groups")? {
        let der = hex(group["publicKeyDer"].as_str().ok_or("no key")?).ok_or("bad hex")?;
        // The key goes in the way a keyring holds it: base64url SPKI.
        let key = parse_public_key(&b64url(&der)).ok_or("key refused")?;
        for t in group["tests"].as_array().ok_or("no tests")? {
            count += 1;
            let id = t["tcId"].as_i64().unwrap_or(0);
            let msg = hex(t["msg"].as_str().unwrap_or("")).ok_or("bad msg")?;
            let sig = hex(t["sig"].as_str().unwrap_or("")).ok_or("bad sig")?;
            let ok = parse(&sig).is_some_and(|s| key.verify(&msg, &s).is_ok());
            let want = t["result"].as_str().unwrap_or("");
            let pass = match want {
                "valid" => ok,
                "invalid" => !ok,
                _ => true,
            };
            if !pass {
                failures.push(format!(
                    "tcId {id} ({want}, got {}): {}",
                    if ok { "accepted" } else { "refused" },
                    t["comment"].as_str().unwrap_or("")
                ));
            }
        }
    }
    let total = doc["numberOfTests"].as_i64().unwrap_or(-1);
    assert_eq!(count, total, "ran every case");
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} failures:\n{}",
            failures.len(),
            failures.join("\n")
        ))
    }
}

#[test]
fn wycheproof_der_signatures() -> Result<(), String> {
    run(DER, |b| Signature::from_der(b).ok())
}

#[test]
fn wycheproof_raw_signatures() -> Result<(), String> {
    run(RAW, |b| Signature::from_slice(b).ok())
}

/// Authenticators do not normalise S, so a high-S signature must verify too.
#[test]
fn high_s_signatures_verify() -> Result<(), String> {
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer as _;
    let key = SigningKey::from_slice(&[7u8; 32]).map_err(|e| e.to_string())?;
    let mut high = 0;
    for i in 0u8..32 {
        let msg = [i; 16];
        let low: Signature = key.sign(&msg);
        let (r, s) = low.split_scalars();
        let flipped = Signature::from_scalars(r, -*s).map_err(|e| e.to_string())?;
        for sig in [low, flipped] {
            assert!(key.verifying_key().verify(&msg, &sig).is_ok());
            high += usize::from(sig.normalize_s() != sig);
        }
    }
    assert_eq!(high, 32, "half the signatures were high-S");
    Ok(())
}
