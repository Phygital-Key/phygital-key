//! The approval page and its small JSON API.
//!
//! Plain HTML and JS, no framework, no third-party scripts, strict CSP.
//! Every POST must come from the page's own origin; every request must use
//! one of our host names (blocks DNS rebinding).

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use phygital_key_verify::Assertion;
use serde::Deserialize;
use serde_json::json;
use subtle::ConstantTimeEq;

use crate::state::Gate;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const APPROVE_HTML: &str = include_str!("../assets/approve.html");
const ENROLL_HTML: &str = include_str!("../assets/enroll.html");
const APP_JS: &str = include_str!("../assets/app.js");
const STYLE_CSS: &str = include_str!("../assets/style.css");
const FONTS_CSS: &str = include_str!("../assets/fonts.css");

/// Files built into the program (fonts and logo), served from the gate itself:
/// the pages never load anything from another site.
static FILES: &[(&str, &str, &[u8])] = &[
    (
        "fonts/anybody-latin-standard-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/anybody-latin-standard-normal.woff2"),
    ),
    (
        "fonts/anybody-latin-ext-standard-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/anybody-latin-ext-standard-normal.woff2"),
    ),
    (
        "fonts/hanken-grotesk-latin-wght-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/hanken-grotesk-latin-wght-normal.woff2"),
    ),
    (
        "fonts/hanken-grotesk-latin-ext-wght-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/hanken-grotesk-latin-ext-wght-normal.woff2"),
    ),
    (
        "fonts/jetbrains-mono-latin-wght-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/jetbrains-mono-latin-wght-normal.woff2"),
    ),
    (
        "fonts/jetbrains-mono-latin-ext-wght-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/jetbrains-mono-latin-ext-wght-normal.woff2"),
    ),
    (
        "fonts/jetbrains-mono-cyrillic-wght-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/jetbrains-mono-cyrillic-wght-normal.woff2"),
    ),
    (
        "fonts/jetbrains-mono-greek-wght-normal.woff2",
        "font/woff2",
        include_bytes!("../assets/fonts/jetbrains-mono-greek-wght-normal.woff2"),
    ),
    (
        "brand/phygital-key-mark.svg",
        "image/svg+xml",
        include_bytes!("../assets/brand/phygital-key-mark.svg"),
    ),
    (
        "brand/phygital-key-wired-cyan.svg",
        "image/svg+xml",
        include_bytes!("../assets/brand/phygital-key-wired-cyan.svg"),
    ),
    (
        "brand/phygital-key-wired-ink.svg",
        "image/svg+xml",
        include_bytes!("../assets/brand/phygital-key-wired-ink.svg"),
    ),
];

async fn asset_file(Path(rest): Path<String>) -> Response {
    match FILES.iter().find(|(name, _, _)| *name == rest) {
        Some((_, kind, bytes)) => (
            [
                (header::CONTENT_TYPE, *kind),
                (header::CACHE_CONTROL, "public, max-age=86400, immutable"),
            ],
            *bytes,
        )
            .into_response(),
        None => err(StatusCode::NOT_FOUND, "not found"),
    }
}

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; connect-src 'self'; img-src 'self' data:; \
                   base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

type Shared = Arc<Gate>;

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({"error": msg.into()}))).into_response()
}

fn ok<T: serde::Serialize>(v: T) -> Response {
    Json(v).into_response()
}

fn page(body: &'static str, kind: &'static str) -> Response {
    ([(header::CONTENT_TYPE, kind)], body).into_response()
}

fn allowed_hosts(gate: &Gate) -> Vec<String> {
    let s = &gate.settings;
    let origin_host = s
        .origin
        .split("://")
        .nth(1)
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    vec![
        origin_host,
        format!("localhost:{}", s.port),
        format!("127.0.0.1:{}", s.port),
        format!("[::1]:{}", s.port),
    ]
}

/// Host check, origin check on writes, and security headers on everything.
async fn guard(State(gate): State<Shared>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !allowed_hosts(&gate).contains(&host) {
        return err(StatusCode::MISDIRECTED_REQUEST, "unknown host");
    }
    let is_mcp = req.uri().path().starts_with("/mcp");
    if is_mcp && let Some(want) = &gate.settings.mcp_token {
        let got = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.split_once(' '))
            .and_then(|(scheme, t)| scheme.eq_ignore_ascii_case("bearer").then_some(t.trim()))
            .unwrap_or_default();
        if got.len() != want.len() || !bool::from(got.as_bytes().ct_eq(want.as_bytes())) {
            return err(
                StatusCode::UNAUTHORIZED,
                "missing or wrong MCP access token (see `phygital-key mcp-token`)",
            );
        }
    }
    if req.method() != Method::GET && !is_mcp {
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default();
        if origin != gate.settings.origin {
            return err(StatusCode::FORBIDDEN, "wrong origin");
        }
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    if !is_mcp {
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CSP),
        );
        if !h.contains_key(header::CACHE_CONTROL) {
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    res
}

async fn status(State(gate): State<Shared>) -> Response {
    match gate.keyring() {
        Ok(ring) => {
            let root = phygital_key_verify::history::root_fingerprint(&ring);
            let signed = root.as_deref().is_some_and(|fp| {
                phygital_key_verify::history::verify_history(&ring, fp, &gate.settings.origin)
                    .is_ok()
            });
            let busy = gate.busy();
            ok(json!({
                "keys": ring.keys.iter().map(|k| json!({"name": k.name, "credential_id": k.credential_id, "created": k.created, "fingerprint": k.fingerprint()})).collect::<Vec<_>>(),
                "rp_id": gate.settings.rp_id,
                "origin": gate.settings.origin,
                "root_fingerprint": root,
                "history_ok": signed,
                "waiting": busy.0,
                "standing_active": busy.1,
            }))
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn well_known(State(gate): State<Shared>) -> Response {
    match gate.keyring() {
        // Public deployments share their key cards with any site. On localhost,
        // no CORS header, so web pages cannot read local key names.
        Ok(ring) if gate.settings.rp_id != "localhost" => {
            ([(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")], Json(ring)).into_response()
        }
        Ok(ring) => Json(ring).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn view(State(gate): State<Shared>, Path(id): Path<String>) -> Response {
    match gate.view(&id) {
        Ok(v) => ok(v),
        Err(e) => err(StatusCode::NOT_FOUND, e),
    }
}

async fn start(State(gate): State<Shared>, Path(id): Path<String>) -> Response {
    match gate.start(&id) {
        Ok(s) => ok(s),
        Err(e) => err(StatusCode::CONFLICT, e),
    }
}

async fn finish(
    State(gate): State<Shared>,
    Path(id): Path<String>,
    Json(a): Json<Assertion>,
) -> Response {
    match gate.finish(&id, a) {
        Ok(v) => ok(v),
        Err(e) => err(StatusCode::BAD_REQUEST, e),
    }
}

async fn deny(State(gate): State<Shared>, Path(id): Path<String>) -> Response {
    match gate.deny(&id) {
        Ok(v) => ok(v),
        Err(e) => err(StatusCode::CONFLICT, e),
    }
}

async fn revoke(State(gate): State<Shared>, Path(id): Path<String>) -> Response {
    match gate.revoke(&id) {
        Ok((v, sign_url)) => ok(json!({"view": v, "sign_url": sign_url})),
        Err(e) => err(StatusCode::CONFLICT, e),
    }
}

async fn sign_revocation(State(gate): State<Shared>, Path(id): Path<String>) -> Response {
    match gate.sign_revocation(&id) {
        Ok(url) => ok(json!({"sign_url": url})),
        Err(e) => err(StatusCode::CONFLICT, e),
    }
}

async fn revoked_list(State(gate): State<Shared>) -> Response {
    let list = match gate.revocations() {
        Ok(l) => l,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let doc = json!({"revocations": list});
    if gate.settings.rp_id != "localhost" {
        ([(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")], Json(doc)).into_response()
    } else {
        Json(doc).into_response()
    }
}

#[derive(Deserialize)]
struct BeginBody {
    token: String,
    name: String,
}

async fn enroll_begin(State(gate): State<Shared>, Json(b): Json<BeginBody>) -> Response {
    match gate.enroll_begin(&b.token, &b.name) {
        Ok(o) => ok(o),
        Err(e) => err(StatusCode::FORBIDDEN, e),
    }
}

#[derive(Deserialize)]
struct FinishBody {
    token: String,
    credential_id: String,
    public_key: String,
    client_data_json: String,
}

async fn enroll_finish(State(gate): State<Shared>, Json(b): Json<FinishBody>) -> Response {
    match gate.enroll_finish(
        &b.token,
        &b.credential_id,
        &b.public_key,
        &b.client_data_json,
    ) {
        Ok(r) => ok(r),
        Err(e) => err(StatusCode::BAD_REQUEST, e),
    }
}

#[derive(Deserialize)]
struct RemoveBody {
    credential_id: String,
}

async fn remove_key(State(gate): State<Shared>, Json(b): Json<RemoveBody>) -> Response {
    match gate.remove_key(&b.credential_id) {
        Ok(v) => ok(json!({"approve_url": v.approve_url})),
        Err(e) => err(StatusCode::BAD_REQUEST, e),
    }
}

async fn not_found(_h: HeaderMap) -> Response {
    err(StatusCode::NOT_FOUND, "not found")
}

/// The page routes. `extra` is merged in before the guard (the MCP endpoint in serve mode).
pub fn router(gate: Shared, extra: Option<Router>) -> Router {
    let mut app = Router::new()
        .route(
            "/",
            get(|| async { page(INDEX_HTML, "text/html; charset=utf-8") }),
        )
        .route(
            "/r/{id}",
            get(|| async { page(APPROVE_HTML, "text/html; charset=utf-8") }),
        )
        .route(
            "/enroll",
            get(|| async { page(ENROLL_HTML, "text/html; charset=utf-8") }),
        )
        .route(
            "/assets/app.js",
            get(|| async { page(APP_JS, "text/javascript; charset=utf-8") }),
        )
        .route(
            "/assets/style.css",
            get(|| async { page(STYLE_CSS, "text/css; charset=utf-8") }),
        )
        .route(
            "/assets/fonts.css",
            get(|| async { page(FONTS_CSS, "text/css; charset=utf-8") }),
        )
        .route("/assets/{*rest}", get(asset_file))
        .route("/.well-known/phygital-key.json", get(well_known))
        .route("/.well-known/phygital-key-revoked.json", get(revoked_list))
        .route("/api/status", get(status))
        .route("/api/r/{id}", get(view))
        .route("/api/r/{id}/start", post(start))
        .route("/api/r/{id}/finish", post(finish))
        .route("/api/r/{id}/deny", post(deny))
        .route("/api/r/{id}/revoke", post(revoke))
        .route("/api/r/{id}/sign-revocation", post(sign_revocation))
        .route("/api/enroll/begin", post(enroll_begin))
        .route("/api/enroll/finish", post(enroll_finish))
        .route("/api/keys/remove", post(remove_key))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(gate.clone());
    if let Some(extra) = extra {
        app = app.merge(extra);
    }
    app.layer(middleware::from_fn_with_state(gate, guard))
}

/// Serve `app` on the gate's address until shutdown.
pub async fn serve(gate: Shared, app: Router) -> Result<(), String> {
    let port = gate.settings.port;
    let bind = gate.settings.bind.clone();
    // On loopback, hold BOTH 127.0.0.1 and [::1]. Browsers may try either for
    // "localhost"; if the gate held only one, another program on this machine could
    // open the other one and serve a fake page with the very same passkey origin.
    let addrs: Vec<String> = if matches!(bind.as_str(), "127.0.0.1" | "localhost" | "::1" | "[::1]")
    {
        vec![format!("127.0.0.1:{port}"), format!("[::1]:{port}")]
    } else {
        vec![format!("{bind}:{port}")]
    };
    let mut listeners = Vec::new();
    for addr in &addrs {
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => listeners.push(l),
            // No IPv6 on this machine: nobody else can listen there either.
            Err(e) if addr.starts_with('[') && e.kind() == std::io::ErrorKind::AddrNotAvailable => {
            }
            Err(e) => {
                return Err(format!(
                    "cannot listen on {addr}: {e}. Another program is using this address \
                     (another phygital-key, or something pretending to be one). Refusing to start."
                ));
            }
        }
    }
    // If any listener stops, stop everything (a service manager restarts the gate),
    // so one address is never left free while the other keeps serving.
    let mut tasks = tokio::task::JoinSet::new();
    for l in listeners {
        let app = app.clone();
        tasks.spawn(async move {
            axum::serve(l, app.into_make_service())
                .with_graceful_shutdown(shutdown_signal())
                .await
                .map_err(|e| e.to_string())
        });
    }
    let first = tasks.join_next().await;
    tasks.abort_all();
    match first {
        Some(Ok(r)) => r,
        Some(Err(e)) => Err(e.to_string()),
        None => Ok(()),
    }
}

/// Ctrl-C, or SIGTERM from a service manager.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
