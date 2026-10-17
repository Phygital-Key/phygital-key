//! Property tests: random input never panics and never verifies;
//! any change to a valid approval is refused; canonical form is stable.

use phygital_key_verify::{Config, Expected, Keyring, Level, MemorySeenStore, b64, canonical, verify};
use proptest::prelude::*;
use serde_json::{Map, Value, json};

const VECTORS: &str = include_str!("../../../test-vectors/vectors.json");

struct Fixture {
    approval: String,
    request: Value,
    aud: String,
    keyring: Keyring,
    config: Config,
    now: i64,
}

fn fixture() -> Fixture {
    let doc: Value = serde_json::from_str(VECTORS).unwrap_or_default();
    let input = doc["vectors"][0]["input"].clone();
    Fixture {
        approval: input["approval"].as_str().unwrap_or_default().to_string(),
        request: input["request"].clone(),
        aud: input["aud"].as_str().unwrap_or_default().to_string(),
        keyring: serde_json::from_value(input["keyring"].clone()).unwrap_or_default(),
        config: serde_json::from_value(input["config"].clone())
            .unwrap_or(Config { origins: vec![], rp_ids: vec![] }),
        now: input["now"].as_i64().unwrap_or_default(),
    }
}

fn run(f: &Fixture, approval: &str, request: &Value) -> bool {
    let expected = Expected { aud: &f.aud, request, level: Level::Medium };
    verify(approval, &expected, &f.keyring, &MemorySeenStore::new(), &f.config, f.now).is_ok()
}

fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-1_000_000i64..1_000_000).prop_map(Value::from),
        "\\PC{0,12}".prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map("\\PC{0,8}", inner, 0..6)
                .prop_map(|m| Value::Object(m.into_iter().collect::<Map<String, Value>>())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn random_text_never_verifies(s in "\\PC{0,400}") {
        let f = fixture();
        prop_assert!(!run(&f, &s, &f.request));
    }

    #[test]
    fn random_bytes_never_verify(bytes in prop::collection::vec(any::<u8>(), 0..2000)) {
        let f = fixture();
        prop_assert!(!run(&f, &b64::encode(&bytes), &f.request));
    }

    #[test]
    fn changing_any_decoded_byte_is_refused(pos in any::<prop::sample::Index>(), bit in 0u8..8) {
        let f = fixture();
        let mut raw = b64::decode(&f.approval).unwrap_or_default();
        let i = pos.index(raw.len());
        if let Some(b) = raw.get_mut(i) { *b ^= 1 << bit; }
        let changed = b64::encode(&raw);
        // A flip inside insignificant JSON whitespace cannot happen: the encoding has none.
        prop_assert!(!run(&f, &changed, &f.request));
    }

    #[test]
    fn any_other_request_is_refused(params in prop::collection::btree_map("[a-z]{1,6}", "[a-z0-9]{0,6}", 0..5)) {
        let f = fixture();
        let other = json!({"type": "deploy", "params": params});
        prop_assume!(canonical::canonical_bytes(&other) != canonical::canonical_bytes(&f.request));
        prop_assert!(!run(&f, &f.approval, &other));
    }

    #[test]
    fn canonical_form_is_idempotent(v in json_value()) {
        if let Ok(s) = canonical::canonical_string(&v) {
            let back: Value = serde_json::from_str(&s).unwrap_or(Value::Null);
            prop_assert_eq!(canonical::canonical_string(&back), Ok(s));
        }
    }

    #[test]
    fn verify_never_panics_on_random_requests(v in json_value()) {
        let f = fixture();
        let _ = run(&f, &f.approval, &v);
    }
}
