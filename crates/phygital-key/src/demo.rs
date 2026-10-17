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
use phygital_key_verify::{Config, Expected, Keyring, Level, MemorySeenStore, verify};
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
}

impl Demo {
    /// A demo service for `aud`.
    pub fn new(aud: String, keyring_path: PathBuf, config: Config) -> Arc<Demo> {
        Arc::new(Demo {
            aud,
            keyring_path,
            config,
            seen: MemorySeenStore::new(),
        })
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

async fn info(State(d): State<Arc<Demo>>) -> Response {
    Json(json!({
        "service": "Phygital Key demo service (fake: nothing real happens)",
        "aud": d.aud,
        "actions": {
            "POST /deploy": "level high. Body: params, e.g. {\"service\":\"api\",\"env\":\"production\",\"version\":\"2.4.1\"}",
            "POST /payment": "level high. Body: params, e.g. {\"to\":\"ACME Ltd\",\"amount\":\"120.00\",\"currency\":\"EUR\"}",
            "POST /send_message": "level medium. Body: params, e.g. {\"to\":\"sam@example.com\",\"text\":\"Hi\"}"
        },
        "how": "First get an approval for {\"type\": <action>, \"params\": <body>} with aud set to this service's aud. Then POST the body with header `Phygital-Approval: <approval>`."
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
    match verify(approval, &expected, &keyring, &d.seen, &d.config, now()) {
        Ok(ok) => Json(json!({
            "ok": true,
            "done": format!("{verb} (demo: nothing real happened)"),
            "request": request,
            "approval_id": ok.id,
            "approved_by": ok.approver,
        }))
        .into_response(),
        Err(code) => refuse(code.as_str(), StatusCode::FORBIDDEN),
    }
}

/// The demo routes.
pub fn router(demo: Arc<Demo>) -> Router {
    Router::new()
        .route("/", get(info))
        .route("/{action}", post(act))
        .with_state(demo)
}
