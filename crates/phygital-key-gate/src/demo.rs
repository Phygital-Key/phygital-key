//! A fake service that refuses to act without a valid approval.
//! It plays the Verifier role exactly like a real service would: it reads the
//! public keyring, checks the approval itself, and runs each approval once.
//! Nothing real ever happens.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use p256::ecdsa::SigningKey;
use p256::pkcs8::EncodePublicKey;
use phygital_key_verify::receipt::{self, Receipt};
use phygital_key_verify::standing::{MemoryUseStore, Revocations, verify_use};
use phygital_key_verify::{
    Config, Expected, Keyring, Level, MemorySeenStore, b64, canonical, verify_bound,
};
use serde_json::{Value, json};

use crate::util::now;

/// The demo service.
pub struct Demo {
    /// Its own audience id (its URL).
    pub aud: String,
    /// The keyring file it trusts (read on every call, like a reloadable config).
    pub keyring_path: PathBuf,
    /// Where approvals may come from.
    pub config: Config,
    seen: MemorySeenStore,
    uses: MemoryUseStore,
    /// The service's own receipt key (a fresh one each run: it is a demo).
    key: SigningKey,
}

impl Demo {
    /// A demo service for `aud`.
    pub fn new(aud: String, keyring_path: PathBuf, config: Config) -> Result<Arc<Demo>, String> {
        let key =
            SigningKey::from_slice(&crate::util::random::<32>()?).map_err(|e| e.to_string())?;
        Ok(Arc::new(Demo {
            aud,
            keyring_path,
            config,
            seen: MemorySeenStore::new(),
            uses: MemoryUseStore::new(),
            key,
        }))
    }

    /// The receipt key's public half (SPKI DER, base64url).
    pub fn public_key(&self) -> String {
        self.key
            .verifying_key()
            .to_public_key_der()
            .map(|d| b64::encode(d.as_bytes()))
            .unwrap_or_default()
    }

    fn receipt(
        &self,
        approval: &str,
        request: &Value,
        id: &str,
        error: Option<&str>,
        use_id: Option<&str>,
    ) -> Option<String> {
        let r = Receipt {
            v: "pka.receipt.1".into(),
            iss: self.aud.clone(),
            approval_id: id.to_string(),
            reference: receipt::reference_of(approval),
            request_hash: canonical::hash_b64(request).unwrap_or_default(),
            status: if error.is_some() {
                "error".into()
            } else {
                "success".into()
            },
            error: error.map(String::from),
            use_id: use_id.map(String::from),
            iat: now(),
        };
        receipt::sign(&r, &self.key).ok()
    }
}

/// The actions it offers and the level each one needs.
fn level_for(action: &str) -> Option<(Level, &'static str)> {
    match action {
        "deploy" => Some((Level::High, "Deployed")),
        "payment" => Some((Level::High, "Paid")),
        "send_message" => Some((Level::Medium, "Sent")),
        _ => None,
    }
}

async fn service_key(State(d): State<Arc<Demo>>) -> Response {
    Json(json!({"iss": d.aud, "receipt_public_key": d.public_key()})).into_response()
}

async fn info(State(d): State<Arc<Demo>>) -> Response {
    Json(json!({
        "service": "Phygital Key demo service (fake: nothing real happens)",
        "aud": d.aud,
        "actions": {
            "POST /deploy": "level high. Body: params, e.g. {\"service\":\"api\",\"env\":\"production\",\"version\":\"2.4.1\"}",
            "POST /payment": "level high. Body: params, e.g. {\"to\":\"ACME Ltd\",\"amount_cents\":12000,\"currency\":\"EUR\"}",
            "POST /send_message": "level medium. Body: params, e.g. {\"to\":\"sam@example.com\",\"text\":\"Hi\"}"
        },
        "how": "First get an approval for {\"type\": <action>, \"params\": <body>} with aud set to this service's aud. Then POST the body with header `Phygital-Approval: <approval>`. With a standing approval, also send `Phygital-Use: <use_token>`. With an approval bound to your key (asked with agent_key), also send `Phygital-Proof: <proof>`."
    }))
    .into_response()
}

async fn act(
    State(d): State<Arc<Demo>>,
    Path(action): Path<String>,
    headers: HeaderMap,
    Json(params): Json<Value>,
) -> Response {
    let refuse = |code: &str, status: StatusCode| {
        (status, Json(json!({"ok": false, "error": code}))).into_response()
    };
    let Some((level, verb)) = level_for(&action) else {
        return refuse("UNKNOWN_ACTION", StatusCode::NOT_FOUND);
    };
    let approval = headers
        .get("phygital-approval")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let keyring = std::fs::read_to_string(&d.keyring_path)
        .ok()
        .and_then(|t| Keyring::from_json(&t).ok())
        .unwrap_or_default();
    let request = json!({"type": action, "params": params});
    let expected = Expected {
        aud: &d.aud,
        request: &request,
        level,
    };
    let use_token = headers.get("phygital-use").and_then(|h| h.to_str().ok());
    let result = match use_token {
        Some(token) => verify_use(
            approval,
            token,
            &expected,
            &keyring,
            &d.uses,
            // Like a real service fetching the gate's signed revocation list.
            &std::fs::read_to_string(d.keyring_path.with_file_name("revocations.json"))
                .map(|t| Revocations::from_json(&t, &d.aud, &keyring, &d.config))
                .unwrap_or_default(),
            &d.config,
            now(),
        )
        .map(|u| (u.standing, Some(u.use_id))),
        // An agent-bound approval (pka.2) needs the agent's proof as well.
        None => verify_bound(
            approval,
            phygital_key_verify::bound::Binding {
                proof: headers.get("phygital-proof").and_then(|h| h.to_str().ok()),
                required: false,
            },
            &expected,
            &keyring,
            &d.seen,
            &d.config,
            now(),
        )
        .map(|v| (v, None)),
    };
    match result {
        Ok((ok, use_id)) => Json(json!({
            "ok": true,
            "done": format!("{verb} (demo: nothing real happened)"),
            "request": request,
            "approval_id": ok.id,
            "approved_by": ok.approver,
            "use_id": use_id,
            "receipt": d.receipt(approval, &request, &ok.id, None, use_id.as_deref()),
        }))
        .into_response(),
        Err(code) => refuse(code.as_str(), StatusCode::FORBIDDEN),
    }
}

/// The demo routes.
pub fn router(demo: Arc<Demo>) -> Router {
    Router::new()
        .route("/", get(info))
        .route("/.well-known/phygital-key-service.json", get(service_key))
        .route("/{action}", post(act))
        .with_state(demo)
}
