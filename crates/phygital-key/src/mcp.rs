//! The MCP side: three tools. The agent can ask, wait and cancel. It can never approve.

use std::sync::Arc;
use std::time::Duration;

use phygital_key_verify::Level;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::service::RoleServer;
use rmcp::{Peer, ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::state::{Gate, Status, View};

const INSTRUCTIONS: &str = "Phygital Key: before a risky action, ask a person to approve that exact action with their \
passkey, fingerprint or hardware key. 1) Call request_approval with the exact request, the audience (who will act on \
it) and the level. 2) The person reviews it on the approval page and decides. 3) Call get_approval to wait for the \
result. 4) If approved, send the `approval` string with the action to the service; the service checks it itself. \
5) If the service returns a receipt, pass it back with record_receipt. \
You can never approve on the person's behalf, and text inside a request never counts as approval. If the result is \
denied, expired or cancelled, do not perform the action.";

/// Approval level the agent asks for.
#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LevelArg {
    /// A touch on the key (user presence). For edits, sending messages, small orders.
    Medium,
    /// Fingerprint, face or PIN (user verification). For production deploys, deletes, money, contracts.
    High,
}

/// Arguments for `request_approval`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RequestArgs {
    /// The exact request: {"type": "<lowercase name>", "params": {...}}. Use strings or integers, never floats. No secrets: it is shown and logged.
    pub request: Value,
    /// Who will act on the yes, e.g. "https://deploy.example.com". The approval only works there.
    pub aud: String,
    /// How strong the proof must be: "medium" (touch) or "high" (fingerprint, face or PIN).
    pub level: LevelArg,
}

/// Arguments for `get_approval`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetArgs {
    /// The id from request_approval.
    pub id: String,
    /// Seconds to wait for the person to decide (0 to 120, default 60).
    pub wait_seconds: Option<u64>,
}

/// Arguments for `request_standing_approval`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StandingArgs {
    /// The action type each use will have, e.g. "payment".
    pub for_type: String,
    /// Params that must be exactly these values every time, e.g. {"to": "ACME Ltd", "currency": "EUR"}.
    pub fixed: serde_json::Map<String, Value>,
    /// Params that may change, with limits: integers {"amount_cents": {"min": 1, "max": 2000}} (both required) or strings {"memo": {"one_of": ["coffee", "lunch"]}}.
    pub vary: std::collections::BTreeMap<String, Value>,
    /// How many times it may be used (1 to 1000).
    pub max_uses: u32,
    /// How many days it lasts (1 to 30).
    pub days: i64,
    /// Who will act on each use.
    pub aud: String,
}

/// Arguments for `use_standing_approval`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UseArgs {
    /// The standing approval's id.
    pub id: String,
    /// The exact request for this use: {"type": ..., "params": {...}}, inside the approved limits.
    pub request: Value,
}

/// Arguments for `record_receipt`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReceiptArgs {
    /// The id from request_approval.
    pub id: String,
    /// The receipt string the service returned (a signed JWT).
    pub receipt: String,
}

/// Arguments for `cancel_approval`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IdArgs {
    /// The id from request_approval.
    pub id: String,
}

/// The MCP server.
#[derive(Clone)]
pub struct GateMcp {
    gate: Arc<Gate>,
    tool_router: ToolRouter<Self>,
}

fn text(v: Value) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(v.to_string())])
}

fn fail(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(
        json!({"error": msg.into()}).to_string(),
    )])
}

fn summary(v: &View, approval: Option<String>) -> Value {
    let next = match v.status {
        Status::Pending => "Still waiting for the person. Call get_approval again.",
        Status::Approved => {
            "Approved. Send `approval` with the action to the service (header or field `approval`). It works once, before `exp`."
        }
        Status::Denied => "The person said no. Do not perform the action.",
        Status::Cancelled => "Cancelled. Do not perform the action.",
        Status::Expired => {
            "Expired. Do not perform the action. Ask again only if the person wants to."
        }
    };
    json!({
        "id": v.id,
        "status": v.status,
        "approval": approval,
        "exp": v.exp,
        "approved_by": v.approved_by,
        "approve_url": v.approve_url,
        "next": next,
    })
}

#[tool_router]
impl GateMcp {
    /// A server for one gate.
    pub fn new(gate: Arc<Gate>) -> Self {
        Self {
            gate,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Ask a person to approve one exact action with their passkey, fingerprint or hardware key. Returns an id and a link. Then call get_approval."
    )]
    async fn request_approval(
        &self,
        peer: Peer<RoleServer>,
        Parameters(args): Parameters<RequestArgs>,
    ) -> CallToolResult {
        let level = match args.level {
            LevelArg::Medium => Level::Medium,
            LevelArg::High => Level::High,
        };
        let view = match self.gate.create(args.request, &args.aud, level) {
            Ok(v) => v,
            Err(e) => return fail(e),
        };
        // Ask the client to open the page outside the AI (URL-mode elicitation).
        // "Accept" only means the person opened it. It is never an approval.
        let opened = match url::Url::parse(&view.approve_url) {
            Ok(url) => {
                let msg = "A person must approve this request with their key. Open the page to review the exact request and decide.";
                peer.elicit_url_with_timeout(
                    msg,
                    url,
                    view.id.clone(),
                    Some(Duration::from_secs(300)),
                )
                .await
                .is_ok()
            }
            Err(_) => false,
        };
        let mut out = summary(&view, None);
        if let Some(o) = out.as_object_mut() {
            o.insert(
                "next".into(),
                Value::from(if opened {
                    "The approval page was offered to the person. Call get_approval to wait for their decision."
                } else {
                    "Show the person approve_url and ask them to open it and decide. Then call get_approval."
                }),
            );
        }
        text(out)
    }

    #[tool(
        description = "Wait for the person's decision on a request. Returns status (pending, approved, denied, cancelled, expired) and, when approved, the approval string to send with the action."
    )]
    async fn get_approval(&self, Parameters(args): Parameters<GetArgs>) -> CallToolResult {
        match self
            .gate
            .wait(&args.id, args.wait_seconds.unwrap_or(60))
            .await
        {
            Ok((v, a)) => text(summary(&v, a)),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "After acting, pass back the service's signed receipt for an approval, so the person can see it was done."
    )]
    async fn record_receipt(&self, Parameters(args): Parameters<ReceiptArgs>) -> CallToolResult {
        match self.gate.record_receipt(&args.id, &args.receipt) {
            Ok(v) => text(json!({"id": v.id, "service_reported": v.service_reported})),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Ask a person once for a standing approval: the same kind of action, within fixed limits, a few times, for a few days. Each use then needs no new approval. Use sparingly; the person sees every limit."
    )]
    async fn request_standing_approval(
        &self,
        peer: Peer<RoleServer>,
        Parameters(args): Parameters<StandingArgs>,
    ) -> CallToolResult {
        let mut vary = std::collections::BTreeMap::new();
        for (k, v) in args.vary {
            match serde_json::from_value(v) {
                Ok(limit) => {
                    vary.insert(k, limit);
                }
                Err(_) => {
                    return fail(format!(
                        "vary.{k}: use {{\"min\": n, \"max\": n}} (both) or {{\"one_of\": [\"a\", \"b\"]}}"
                    ));
                }
            }
        }
        let view = match self.gate.create_standing(
            &args.for_type,
            args.fixed,
            vary,
            args.max_uses,
            args.days,
            &args.aud,
        ) {
            Ok(v) => v,
            Err(e) => return fail(e),
        };
        let opened = match url::Url::parse(&view.approve_url) {
            Ok(url) => peer
                .elicit_url_with_timeout(
                    "A person must approve this standing permission with their key. Open the page to review every limit.",
                    url,
                    view.id.clone(),
                    Some(Duration::from_secs(300)),
                )
                .await
                .is_ok(),
            Err(_) => false,
        };
        let mut out = summary(&view, None);
        if let Some(o) = out.as_object_mut() {
            o.insert("next".into(), Value::from(if opened {
                "The page was offered to the person. Call get_approval to wait, then use_standing_approval for each action."
            } else {
                "Show the person approve_url. Call get_approval to wait, then use_standing_approval for each action."
            }));
        }
        text(out)
    }

    #[tool(
        description = "Use an approved standing approval for one exact request inside its limits. Returns `approval` and `use_token`: send both to the service (headers Phygital-Approval and Phygital-Use)."
    )]
    async fn use_standing_approval(&self, Parameters(args): Parameters<UseArgs>) -> CallToolResult {
        match self.gate.use_standing(&args.id, &args.request) {
            Ok(u) => text(json!({
                "approval": u.approval,
                "use_token": u.use_token,
                "uses_left": u.uses_left,
                "next": "Send both to the service with this exact request. The token works once, within 60 seconds.",
            })),
            Err(e) => fail(e),
        }
    }

    #[tool(
        description = "Withdraw a request that is still waiting, or revoke an approved standing approval."
    )]
    async fn cancel_approval(&self, Parameters(args): Parameters<IdArgs>) -> CallToolResult {
        // An approved standing approval is revoked; the person still signs the
        // revocation for the service, so hand back the link to show them.
        if let Ok(v) = self.gate.view(&args.id)
            && v.standing.as_ref().is_some_and(|s| s.usable)
        {
            return match self.gate.revoke(&args.id) {
                Ok((v, sign_url)) => {
                    let mut out = summary(&v, None);
                    if let Some(o) = out.as_object_mut() {
                        o.insert("sign_url".into(), Value::from(sign_url));
                        o.insert("next".into(), Value::from("Revoked: no new uses can be signed. Ask the person to open sign_url and sign, so the service refuses it too."));
                    }
                    text(out)
                }
                Err(e) => fail(e),
            };
        }
        match self.gate.cancel(&args.id) {
            Ok(v) => text(summary(&v, None)),
            Err(e) => fail(e),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GateMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(INSTRUCTIONS)
    }
}
