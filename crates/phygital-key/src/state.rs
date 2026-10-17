//! The gate's state: pending requests, the keyring, enrollment, the audit log.
//! No LLM, no network. The gate can never create a yes by itself: only the
//! person's key can sign, and the gate checks that signature before storing it.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use phygital_key_verify::webauthn::{self, AssertionCheck};
use phygital_key_verify::{
    Assertion, Config, Expected, KeyCard, Keyring, Level, MAX_REQUEST_BYTES, VERSION, b64,
    canonical, check, request_shape_ok,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::Notify;

use crate::audit::Audit;
use crate::util::{self, now};

/// How long a request waits for the person.
pub const PENDING_SECS: i64 = 600;
/// How long an enrollment link works.
pub const ENROLL_SECS: i64 = 600;
/// Most pending requests at once.
const MAX_PENDING: usize = 50;
/// Most new requests per minute.
const MAX_PER_MINUTE: usize = 30;
/// Audience the gate uses for its own requests (adding a key).
pub const GATE_AUD: &str = "phygital-key:keyring";

/// Where and how the gate runs.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Data folder (keyring, audit log, enrollment token).
    pub home: PathBuf,
    /// Address to listen on.
    pub bind: String,
    /// Port to listen on.
    pub port: u16,
    /// Public origin of the approval page, e.g. `http://localhost:7468`.
    pub origin: String,
    /// WebAuthn relying party id (the origin's host).
    pub rp_id: String,
    /// Approval life in seconds.
    pub lifetime: i64,
}

impl Settings {
    /// The verifier config for approvals made on this page.
    pub fn config(&self) -> Config {
        Config {
            origins: vec![self.origin.clone()],
            rp_ids: vec![self.rp_id.clone()],
        }
    }
    fn keyring_path(&self) -> PathBuf {
        self.home.join("keyring.json")
    }
    fn token_path(&self) -> PathBuf {
        self.home.join("enroll-token.json")
    }
    fn audit_path(&self) -> PathBuf {
        self.home.join("audit.log")
    }
    /// Link to one request's approval page.
    pub fn approve_url(&self, id: &str) -> String {
        format!("{}/r/{id}", self.origin)
    }
}

/// What a request is for.
#[derive(Debug, Clone)]
enum Kind {
    /// An outside party will act on the yes.
    Action,
    /// Adding a new key to the keyring, approved by an existing key.
    AddKey(KeyCard),
}

/// Where a request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Waiting for the person.
    Pending,
    /// Signed. The approval can be fetched.
    Approved,
    /// The person said no.
    Denied,
    /// The requester withdrew it.
    Cancelled,
    /// Nobody decided in time, or the approval ran out.
    Expired,
}

#[derive(Debug, Clone)]
struct Request {
    id: String,
    request: Value,
    request_jcs: String,
    request_hash: String,
    aud: String,
    level: Level,
    kind: Kind,
    created: i64,
    deadline: i64,
    status: Status,
    claims: Option<Value>,
    approval: Option<String>,
    approved_by: Option<String>,
    exp: Option<i64>,
}

impl Request {
    fn current_status(&self, t: i64) -> Status {
        match self.status {
            Status::Pending if t >= self.deadline => Status::Expired,
            Status::Approved if self.exp.is_some_and(|e| t >= e) => Status::Expired,
            s => s,
        }
    }
}

/// What the approval page and tools see about a request.
#[derive(Debug, Clone, Serialize)]
pub struct View {
    /// Request id.
    pub id: String,
    /// Where it stands.
    pub status: Status,
    /// Who will act on the yes.
    pub aud: String,
    /// Required level.
    pub level: Level,
    /// The canonical request text. The page shows this, never agent prose.
    pub request_jcs: String,
    /// `request_hash`.
    pub request_hash: String,
    /// The approval page link.
    pub approve_url: String,
    /// When it was asked.
    pub created: i64,
    /// Decide before this time.
    pub deadline: i64,
    /// Approval expiry, once approved.
    pub exp: Option<i64>,
    /// Name on the key that approved.
    pub approved_by: Option<String>,
    /// True when this request adds a new key.
    pub adds_key: bool,
}

/// What the page needs to sign.
#[derive(Debug, Clone, Serialize)]
pub struct Started {
    /// Canonical claims. The page hashes these exact bytes into the challenge.
    pub claims_jcs: String,
    /// Relying party id.
    pub rp_id: String,
    /// Credential ids that may sign.
    pub allow: Vec<String>,
}

/// Options for creating a passkey.
#[derive(Debug, Clone, Serialize)]
pub struct EnrollOptions {
    /// Registration challenge, base64url.
    pub challenge: String,
    /// Relying party id.
    pub rp_id: String,
    /// Random user handle, base64url.
    pub user_id: String,
    /// Display name for the key.
    pub name: String,
    /// Credentials already enrolled (so the authenticator does not make a duplicate).
    pub exclude: Vec<String>,
}

/// Result of finishing an enrollment.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Enrolled {
    /// First key: added right away.
    Added {
        /// The key card.
        card: KeyCard,
    },
    /// Not the first key: an existing key must approve it.
    NeedsApproval {
        /// Where to approve.
        approve_url: String,
    },
}

struct EnrollSession {
    challenge: [u8; 32],
    user_id: [u8; 16],
    name: String,
    expires: i64,
}

struct Inner {
    requests: HashMap<String, Request>,
    recent: VecDeque<i64>,
    enroll: Option<EnrollSession>,
    keyring: Keyring,
    audit: Audit,
}

/// Shared gate state.
pub struct Gate {
    /// Settings.
    pub settings: Settings,
    inner: Mutex<Inner>,
    changed: Notify,
}

fn sanitize_name(name: &str) -> String {
    let clean: String = name.chars().filter(|c| !c.is_control()).take(64).collect();
    let clean = clean.trim().to_string();
    if clean.is_empty() {
        "Key".into()
    } else {
        clean
    }
}

impl Gate {
    /// Open the data folder and load the keyring and audit log.
    pub fn open(settings: Settings) -> Result<Arc<Gate>, String> {
        util::private_dir(&settings.home)?;
        let keyring = match std::fs::read_to_string(settings.keyring_path()) {
            Ok(t) => Keyring::from_json(&t)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Keyring::default(),
            Err(e) => return Err(format!("keyring: {e}")),
        };
        let audit = Audit::open(&settings.audit_path())?;
        Ok(Arc::new(Gate {
            settings,
            inner: Mutex::new(Inner {
                requests: HashMap::new(),
                recent: VecDeque::new(),
                enroll: None,
                keyring,
                audit,
            }),
            changed: Notify::new(),
        }))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, String> {
        self.inner.lock().map_err(|_| "internal error".to_string())
    }

    fn save_keyring(&self, ring: &Keyring) -> Result<(), String> {
        let text = serde_json::to_string_pretty(ring).map_err(|e| e.to_string())? + "\n";
        util::write_private(&self.settings.keyring_path(), text.as_bytes())
    }

    /// The trusted keys (public key cards only).
    pub fn keyring(&self) -> Result<Keyring, String> {
        Ok(self.lock()?.keyring.clone())
    }

    /// Ask the person to approve `request` for `aud` at `level`.
    pub fn create(&self, request: Value, aud: &str, level: Level) -> Result<View, String> {
        self.create_kind(request, aud, level, Kind::Action)
    }

    fn create_kind(
        &self,
        request: Value,
        aud: &str,
        level: Level,
        kind: Kind,
    ) -> Result<View, String> {
        if !request_shape_ok(&request) {
            return Err(
                "request must be {\"type\": \"<lowercase name>\", \"params\": {...}}".into(),
            );
        }
        let request = canonical::normalize(&request).map_err(|_| {
            "request has no canonical form: use strings or integers, never floats".to_string()
        })?;
        let request_jcs = canonical::canonical_string(&request)
            .map_err(|_| "request: no canonical form".to_string())?;
        if request_jcs.len() > MAX_REQUEST_BYTES {
            return Err(format!("request is over {MAX_REQUEST_BYTES} bytes"));
        }
        if aud.is_empty() || aud.len() > 256 || aud.chars().any(char::is_control) {
            return Err(
                "aud must be 1 to 256 printable characters naming who will act on the yes".into(),
            );
        }
        let request_hash =
            canonical::hash_b64(&request).map_err(|_| "request: no canonical form".to_string())?;
        let t = now();
        let id = util::new_id()?;
        let mut inner = self.lock()?;
        if inner.keyring.keys.is_empty() {
            return Err("No key is enrolled yet. Ask the person to run `phygital-key enroll` and add their passkey first.".into());
        }
        inner.requests.retain(|_, r| t < r.deadline + 3600);
        while inner.recent.front().is_some_and(|&x| x <= t - 60) {
            inner.recent.pop_front();
        }
        if inner.recent.len() >= MAX_PER_MINUTE {
            return Err("Too many requests this minute. Try again shortly.".into());
        }
        if inner
            .requests
            .values()
            .filter(|r| r.current_status(t) == Status::Pending)
            .count()
            >= MAX_PENDING
        {
            return Err("Too many requests are waiting. Wait for the person to decide.".into());
        }
        inner.recent.push_back(t);
        let r = Request {
            id: id.clone(),
            request,
            request_jcs,
            request_hash: request_hash.clone(),
            aud: aud.to_string(),
            level,
            kind,
            created: t,
            deadline: t + PENDING_SECS,
            status: Status::Pending,
            claims: None,
            approval: None,
            approved_by: None,
            exp: None,
        };
        inner.audit.append(
            "requested",
            json!({"id": id, "aud": aud, "level": level.as_str(), "request_hash": request_hash}),
            t,
        )?;
        let view = self.view_of(&r, t);
        inner.requests.insert(id, r);
        Ok(view)
    }

    fn view_of(&self, r: &Request, t: i64) -> View {
        View {
            id: r.id.clone(),
            status: r.current_status(t),
            aud: r.aud.clone(),
            level: r.level,
            request_jcs: r.request_jcs.clone(),
            request_hash: r.request_hash.clone(),
            approve_url: self.settings.approve_url(&r.id),
            created: r.created,
            deadline: r.deadline,
            exp: r.exp,
            approved_by: r.approved_by.clone(),
            adds_key: matches!(r.kind, Kind::AddKey(_)),
        }
    }

    /// See a request.
    pub fn view(&self, id: &str) -> Result<View, String> {
        let inner = self.lock()?;
        let r = inner.requests.get(id).ok_or("unknown request")?;
        Ok(self.view_of(r, now()))
    }

    /// The approval, if approved and still valid.
    pub fn approval(&self, id: &str) -> Result<(View, Option<String>), String> {
        let inner = self.lock()?;
        let r = inner.requests.get(id).ok_or("unknown request")?;
        let v = self.view_of(r, now());
        let a = if v.status == Status::Approved {
            r.approval.clone()
        } else {
            None
        };
        Ok((v, a))
    }

    /// Wait up to `secs` for a request to leave `pending`.
    pub async fn wait(&self, id: &str, secs: u64) -> Result<(View, Option<String>), String> {
        let until = tokio::time::Instant::now() + Duration::from_secs(secs.min(120));
        loop {
            let notified = self.changed.notified();
            let (v, a) = self.approval(id)?;
            if v.status != Status::Pending || tokio::time::Instant::now() >= until {
                return Ok((v, a));
            }
            let _ = tokio::time::timeout_at(until, notified).await;
        }
    }

    /// The person pressed Approve: fix the claims (and the clock) now.
    pub fn start(&self, id: &str) -> Result<Started, String> {
        let t = now();
        let mut inner = self.lock()?;
        let allow: Vec<String> = inner
            .keyring
            .keys
            .iter()
            .map(|k| k.credential_id.clone())
            .collect();
        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        if r.current_status(t) != Status::Pending {
            return Err("This request is no longer waiting.".into());
        }
        let claims = json!({
            "v": VERSION,
            "id": r.id,
            "aud": r.aud,
            "request": r.request,
            "request_hash": r.request_hash,
            "level": r.level.as_str(),
            "iat": t,
            "exp": t + self.settings.lifetime,
        });
        let claims_jcs = canonical::canonical_string(&claims)
            .map_err(|_| "claims: no canonical form".to_string())?;
        r.claims = Some(claims);
        Ok(Started {
            claims_jcs,
            rp_id: self.settings.rp_id.clone(),
            allow,
        })
    }

    /// The page sends the signature. The gate checks it fully before keeping it.
    pub fn finish(&self, id: &str, assertion: Assertion) -> Result<View, String> {
        let t = now();
        let mut inner = self.lock()?;
        let keyring = inner.keyring.clone();
        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        if r.current_status(t) != Status::Pending {
            return Err("This request is no longer waiting.".into());
        }
        let claims = r.claims.clone().ok_or("Press Approve first.")?;
        let obj = json!({"claims": claims, "signatures": [assertion]});
        let approval = b64::encode(
            canonical::canonical_bytes(&obj)
                .map_err(|_| "approval: no canonical form".to_string())?,
        );
        let expected = Expected {
            aud: &r.aud,
            request: &r.request,
            level: r.level,
        };
        let ok =
            check(&approval, &expected, &keyring, &self.settings.config(), t).map_err(|code| {
                format!("The signature was not accepted ({code}). Nothing was approved.")
            })?;
        r.status = Status::Approved;
        r.approval = Some(approval);
        r.approved_by = Some(ok.approver.clone());
        r.exp = Some(ok.exp);
        let kind = r.kind.clone();
        let entry = json!({"id": ok.id, "aud": r.aud, "level": ok.level.as_str(), "request_hash": ok.request_hash, "credential_id": ok.credential_id});
        let view = self.view_of(r, t);
        inner.audit.append("approved", entry, t)?;
        if let Kind::AddKey(card) = kind
            && inner.keyring.find(&card.credential_id).is_none()
        {
            inner.keyring.keys.push(card.clone());
            let ring = inner.keyring.clone();
            self.save_keyring(&ring)?;
            inner.audit.append("key_added", json!({"credential_id": card.credential_id, "name": card.name, "approved_with": ok.credential_id}), t)?;
        }
        drop(inner);
        self.changed.notify_waiters();
        Ok(view)
    }

    /// The person said no.
    pub fn deny(&self, id: &str) -> Result<View, String> {
        self.close(id, Status::Denied, "denied")
    }

    /// The requester withdrew.
    pub fn cancel(&self, id: &str) -> Result<View, String> {
        self.close(id, Status::Cancelled, "cancelled")
    }

    fn close(&self, id: &str, status: Status, event: &str) -> Result<View, String> {
        let t = now();
        let mut inner = self.lock()?;
        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        if r.current_status(t) != Status::Pending {
            return Err("This request is no longer waiting.".into());
        }
        r.status = status;
        let view = self.view_of(r, t);
        let entry = json!({"id": r.id, "aud": r.aud, "request_hash": r.request_hash});
        inner.audit.append(event, entry, t)?;
        drop(inner);
        self.changed.notify_waiters();
        Ok(view)
    }

    /// Write a new one-time enrollment token. Only the person at the
    /// computer sees it (printed by the CLI). Returns the enrollment link.
    pub fn issue_enroll_token(&self) -> Result<String, String> {
        write_enroll_token(&self.settings)
    }

    fn token_ok(&self, token: &str) -> bool {
        let Ok(text) = std::fs::read_to_string(self.settings.token_path()) else {
            return false;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            return false;
        };
        let (Some(saved), Some(expires)) = (
            v.get("token").and_then(Value::as_str),
            v.get("expires").and_then(Value::as_i64),
        ) else {
            return false;
        };
        now() < expires
            && saved.len() == token.len()
            && bool::from(saved.as_bytes().ct_eq(token.as_bytes()))
    }

    /// Step 1 of adding a key.
    pub fn enroll_begin(&self, token: &str, name: &str) -> Result<EnrollOptions, String> {
        if !self.token_ok(token) {
            return Err("This enrollment link is not valid or has expired. Run `phygital-key enroll` for a new one.".into());
        }
        let session = EnrollSession {
            challenge: util::random()?,
            user_id: util::random()?,
            name: sanitize_name(name),
            expires: now() + ENROLL_SECS,
        };
        let mut inner = self.lock()?;
        let opts = EnrollOptions {
            challenge: b64::encode(session.challenge),
            rp_id: self.settings.rp_id.clone(),
            user_id: b64::encode(session.user_id),
            name: session.name.clone(),
            exclude: inner
                .keyring
                .keys
                .iter()
                .map(|k| k.credential_id.clone())
                .collect(),
        };
        inner.enroll = Some(session);
        Ok(opts)
    }

    /// Step 2 of adding a key. The first key is added directly; any later key
    /// must be approved (level high) by a key that is already trusted.
    pub fn enroll_finish(
        &self,
        token: &str,
        credential_id: &str,
        public_key: &str,
        client_data_json: &str,
    ) -> Result<Enrolled, String> {
        if !self.token_ok(token) {
            return Err("This enrollment link is not valid or has expired.".into());
        }
        let t = now();
        let mut inner = self.lock()?;
        let session = inner
            .enroll
            .take()
            .ok_or("Start again: no enrollment in progress.")?;
        if t >= session.expires {
            return Err("Enrollment took too long. Start again.".into());
        }
        let cd = b64::decode(client_data_json).ok_or("bad client data")?;
        let origins = vec![self.settings.origin.clone()];
        let want = AssertionCheck {
            kind: "webauthn.create",
            challenge: &session.challenge,
            origins: &origins,
            rp_id: &self.settings.rp_id,
        };
        if !webauthn::check_client_data(&cd, &want) {
            return Err("The passkey was not created for this page.".into());
        }
        if webauthn::parse_public_key(public_key).is_none() {
            return Err("Only P-256 (ES256) keys are supported.".into());
        }
        if b64::decode(credential_id).is_none_or(|c| c.is_empty() || c.len() > 1023) {
            return Err("bad credential id".into());
        }
        if inner.keyring.find(credential_id).is_some() {
            return Err("This key is already enrolled.".into());
        }
        let _ = session.user_id;
        let card = KeyCard {
            credential_id: credential_id.to_string(),
            public_key: public_key.to_string(),
            rp_id: self.settings.rp_id.clone(),
            name: session.name.clone(),
            created: util::date(t),
        };
        let _ = std::fs::remove_file(self.settings.token_path());
        if inner.keyring.keys.is_empty() {
            inner.keyring.keys.push(card.clone());
            let ring = inner.keyring.clone();
            self.save_keyring(&ring)?;
            inner.audit.append("key_added", json!({"credential_id": card.credential_id, "name": card.name, "approved_with": "first key"}), t)?;
            return Ok(Enrolled::Added { card });
        }
        drop(inner);
        let pk_hash = b64::encode(Sha256::digest(b64::decode(public_key).unwrap_or_default()));
        let request = json!({"type": "add_key", "params": {"name": card.name, "credential_id": card.credential_id, "public_key_sha256": pk_hash}});
        let view = self.create_kind(request, GATE_AUD, Level::High, Kind::AddKey(card))?;
        Ok(Enrolled::NeedsApproval {
            approve_url: view.approve_url,
        })
    }
}

/// Write a fresh enrollment token into the data folder and return the link.
/// The token goes in the link's #fragment, which browsers never send to servers.
pub fn write_enroll_token(settings: &Settings) -> Result<String, String> {
    util::private_dir(&settings.home)?;
    let token = b64::encode(util::random::<32>()?);
    let doc = json!({"token": token, "expires": now() + ENROLL_SECS});
    util::write_private(&settings.token_path(), doc.to_string().as_bytes())?;
    Ok(format!("{}/enroll#token={token}", settings.origin))
}
