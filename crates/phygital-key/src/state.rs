//! The gate's state: pending requests, the keyring, enrollment, the audit log.
//! No LLM, no network. The gate can never create a yes by itself: only the
//! person's key can sign, and the gate checks that signature before storing it.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use p256::ecdsa::SigningKey;
use p256::pkcs8::EncodePublicKey;
use phygital_key_verify::history;
use phygital_key_verify::receipt;
use phygital_key_verify::standing::{self, StandingParams, Use};
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
/// Most requests held in memory at once (waiting, decided, or active standing).
const MAX_STORED: usize = 5000;

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
    /// Bearer token required on `/mcp` in HTTP mode (`None`: open, `--open-mcp`).
    pub mcp_token: Option<String>,
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
    /// The audience of this gate's own key changes.
    pub fn keyring_aud(&self) -> String {
        history::keyring_aud(&self.origin)
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
    /// Adding a key. The first key approves itself; later keys need a trusted key.
    AddKey(KeyCard),
    /// Removing a key, approved by a trusted key.
    RemoveKey(KeyCard),
    /// A standing approval: limited uses, signed one by one with a gate-held key.
    Standing(StandingParams),
    /// The person's signed revocation of a standing approval, for services to see.
    Revoke(String),
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
    first_key: bool,
    created: i64,
    deadline: i64,
    status: Status,
    claims: Option<Value>,
    approval: Option<String>,
    approved_by: Option<String>,
    exp: Option<i64>,
    receipt: Option<String>,
    /// Standing approvals: when the permission ends, and uses so far.
    until: Option<i64>,
    uses: u32,
    usable: bool,
    /// Standing approvals: use tokens handed out, by use id.
    issued: HashMap<String, String>,
    /// Standing approvals: the person's revocation is signed and published.
    revocation_published: bool,
}

impl Request {
    /// Forget it after this time: an hour after it ended, or after a standing approval's end.
    fn keep_until(&self) -> i64 {
        // Only an approved standing approval is kept until its end; anything never
        // approved is forgotten an hour after its deadline, like every other request.
        let standing_end = if self.approval.is_some() {
            self.until.map_or(0, |u| u + 3600)
        } else {
            0
        };
        (self.deadline + 3600).max(standing_end)
    }

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
    /// True when this is the very first key, approving itself.
    pub first_key: bool,
    /// True when this request removes a key.
    pub removes_key: bool,
    /// True when this is the signed revocation of a standing approval.
    pub revokes: bool,
    /// Standing approval details, for standing requests.
    pub standing: Option<StandingView>,
    /// For a revoked standing approval: whether the person's revocation is signed.
    pub revocation_signed: Option<bool>,
    /// What the service reported after acting (`success` or `error`): set only
    /// from a receipt signed by the service's pinned key.
    pub service_reported: Option<String>,
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

/// Result of creating a key: it is trusted only once it is approved.
/// The first key approves itself; any later key needs a key already trusted.
#[derive(Debug, Clone, Serialize)]
pub struct Enrolled {
    /// Where to approve.
    pub approve_url: String,
    /// True for the very first key.
    pub first_key: bool,
}

struct EnrollSession {
    token_hash: [u8; 32],
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
    /// Use keys of standing approvals, by request id. Memory only: a restart
    /// or a revoke throws them away, and no more uses can be signed.
    use_keys: HashMap<String, SigningKey>,
}

/// Standing approval details shown on the page and to the agent.
#[derive(Debug, Clone, Serialize)]
pub struct StandingView {
    /// Action type allowed.
    pub for_type: String,
    /// Uses allowed.
    pub max_uses: u32,
    /// Uses signed so far.
    pub uses: u32,
    /// When it ends.
    pub until: i64,
    /// False once revoked or after a restart (the use key is gone).
    pub usable: bool,
}

/// One signed use of a standing approval.
#[derive(Debug, Clone, Serialize)]
pub struct SignedUse {
    /// The person's standing approval (send as `Phygital-Approval`).
    pub approval: String,
    /// The use token for this one request (send as `Phygital-Use`).
    pub use_token: String,
    /// Uses left after this one.
    pub uses_left: u32,
}

/// Shared gate state.
pub struct Gate {
    /// Settings.
    pub settings: Settings,
    inner: Mutex<Inner>,
    changed: Notify,
    /// Receipt keys pinned in memory (the demo pins its own service).
    pinned: Mutex<HashMap<String, String>>,
}

/// Audiences are visible ASCII only: no spaces, no look-alikes, no invisible characters.
pub fn aud_ok(aud: &str) -> bool {
    !aud.is_empty() && aud.len() <= 256 && aud.bytes().all(|b| (0x21..=0x7e).contains(&b))
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
                use_keys: HashMap::new(),
            }),
            changed: Notify::new(),
            pinned: Mutex::new(HashMap::new()),
        }))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, String> {
        self.inner.lock().map_err(|_| "internal error".to_string())
    }

    fn services_path(&self) -> PathBuf {
        self.settings.home.join("services.json")
    }

    /// Pin a service's receipt key for this run only.
    pub fn pin_service(&self, aud: &str, public_key: &str) {
        if let Ok(mut p) = self.pinned.lock() {
            p.insert(aud.to_string(), public_key.to_string());
        }
    }

    /// The pinned receipt key of a service: in memory, or in `services.json`.
    fn service_key(&self, aud: &str) -> Option<p256::ecdsa::VerifyingKey> {
        let from_memory = self.pinned.lock().ok().and_then(|p| p.get(aud).cloned());
        let spki = from_memory.or_else(|| {
            let text = std::fs::read_to_string(self.services_path()).ok()?;
            let doc: Value = serde_json::from_str(&text).ok()?;
            doc.get("services")?.as_array()?.iter().find_map(|s| {
                (s.get("aud")?.as_str()? == aud)
                    .then(|| s.get("receipt_public_key")?.as_str().map(String::from))?
            })
        })?;
        webauthn::parse_public_key(&spki)
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
        self.create_full(request, aud, level, kind, None)
    }

    /// Create a request. `standing` is the end time and use key of a standing
    /// approval, set in the same step so the page never sees it half made.
    fn create_full(
        &self,
        request: Value,
        aud: &str,
        level: Level,
        kind: Kind,
        standing: Option<(i64, SigningKey)>,
    ) -> Result<View, String> {
        if !request_shape_ok(&request) {
            return Err(
                "request must be {\"type\": \"<lowercase name>\", \"params\": {...}}".into(),
            );
        }
        canonical::validate(&request).map_err(|e| match e {
            canonical::CanonError::NotNfc => {
                "request text must be Unicode NFC (it is never rewritten, so look-alikes cannot sneak in)".to_string()
            }
            _ => "request has no canonical form: use strings or integers, never floats".to_string(),
        })?;
        let request_jcs = canonical::canonical_string(&request)
            .map_err(|_| "request: no canonical form".to_string())?;
        if request_jcs.len() > MAX_REQUEST_BYTES {
            return Err(format!("request is over {MAX_REQUEST_BYTES} bytes"));
        }
        // Approval objects nest 3 levels above params; keep the whole object within the limit.
        if phygital_key_verify::strict::depth(request_jcs.as_bytes())
            > phygital_key_verify::strict::MAX_DEPTH - 3
        {
            return Err("request is nested too deeply".into());
        }
        if !aud_ok(aud) {
            return Err(
                "aud must be 1 to 256 visible ASCII characters naming who will act on the yes (a URL, domain or id)".into(),
            );
        }
        // A standing request can only be made through create_standing, so the person
        // never signs one that looks like a normal request, with a key the agent chose.
        if request.get("type").and_then(Value::as_str) == Some(phygital_key_verify::STANDING_TYPE)
            && !matches!(kind, Kind::Standing(_))
        {
            return Err("\"standing\" is reserved: use request_standing_approval".into());
        }
        if request.get("type").and_then(Value::as_str) == Some(phygital_key_verify::REVOKE_TYPE)
            && !matches!(kind, Kind::Revoke(_))
        {
            return Err(
                "\"revoke\" is reserved: revoke a standing approval on its approval page".into(),
            );
        }
        if aud == self.settings.keyring_aud() && matches!(kind, Kind::Action | Kind::Standing(_)) {
            return Err("this audience is reserved for the gate itself".into());
        }
        let request_hash =
            canonical::hash_b64(&request).map_err(|_| "request: no canonical form".to_string())?;
        let t = now();
        let id = util::new_id()?;
        let mut inner = self.lock()?;
        let first_key = inner.keyring.keys.is_empty();
        if first_key && matches!(kind, Kind::Action | Kind::Standing(_)) {
            return Err("No key is enrolled yet. Ask the person to run `phygital-key enroll` and add their passkey first.".into());
        }
        Self::forget_old(&mut inner, t);
        while inner.recent.front().is_some_and(|&x| x <= t - 60) {
            inner.recent.pop_front();
        }
        // A revocation is always let through: an agent filling the limits must
        // never be able to stop the person from revoking.
        let is_revoke = matches!(kind, Kind::Revoke(_));
        if !is_revoke {
            if inner.recent.len() >= MAX_PER_MINUTE || inner.requests.len() >= MAX_STORED {
                return Err("Too many requests right now. Try again shortly.".into());
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
        }
        let r = Request {
            id: id.clone(),
            request,
            request_jcs,
            request_hash: request_hash.clone(),
            aud: aud.to_string(),
            level,
            kind,
            first_key,
            created: t,
            deadline: t + PENDING_SECS,
            status: Status::Pending,
            claims: None,
            approval: None,
            approved_by: None,
            exp: None,
            receipt: None,
            until: standing.as_ref().map(|(days, _)| t + days * 86_400),
            uses: 0,
            usable: false,
            issued: HashMap::new(),
            revocation_published: false,
        };
        if let Some((_, key)) = standing {
            inner.use_keys.insert(id.clone(), key);
        }
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
            first_key: matches!(r.kind, Kind::AddKey(_)) && r.first_key,
            removes_key: matches!(r.kind, Kind::RemoveKey(_)),
            revokes: matches!(r.kind, Kind::Revoke(_)),
            revocation_signed: (matches!(r.kind, Kind::Standing(_))
                && r.approval.is_some()
                && !r.usable)
                .then_some(r.revocation_published),
            standing: match &r.kind {
                Kind::Standing(p) => Some(StandingView {
                    for_type: p.for_type.clone(),
                    max_uses: p.max_uses,
                    uses: r.uses,
                    until: r.until.unwrap_or(0),
                    usable: r.usable && r.current_status(t) == Status::Approved,
                }),
                _ => None,
            },
            service_reported: r
                .receipt
                .as_deref()
                .and_then(receipt::peek)
                .map(|x| x.status),
        }
    }

    fn forget_old(inner: &mut Inner, t: i64) {
        inner.requests.retain(|_, r| t < r.keep_until());
        let Inner {
            requests, use_keys, ..
        } = inner;
        use_keys.retain(|id, _| requests.contains_key(id));
    }

    /// What a restart would end: requests still waiting for the person, and
    /// standing approvals that can still be used (their use keys live only in
    /// memory). The daily update waits while either is above zero.
    pub fn busy(&self) -> (usize, usize) {
        let t = now();
        let Ok(inner) = self.inner.lock() else {
            return (0, 0);
        };
        let waiting = inner
            .requests
            .values()
            .filter(|r| r.current_status(t) == Status::Pending)
            .count();
        let standing = inner
            .requests
            .values()
            .filter(|r| {
                r.usable
                    && r.current_status(t) == Status::Approved
                    && r.until.is_some_and(|u| t < u)
                    && inner.use_keys.contains_key(&r.id)
            })
            .count();
        (waiting, standing)
    }

    /// Forget requests that ended more than an hour ago. Called on a timer.
    pub fn prune(&self) {
        let t = now();
        if let Ok(mut inner) = self.inner.lock() {
            Self::forget_old(&mut inner, t);
            if inner.enroll.as_ref().is_some_and(|e| t >= e.expires) {
                inner.enroll = None;
            }
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
        let mut allow: Vec<String> = inner
            .keyring
            .keys
            .iter()
            .map(|k| k.credential_id.clone())
            .collect();
        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        if r.current_status(t) != Status::Pending {
            return Err("This request is no longer waiting.".into());
        }
        if r.until
            .is_some_and(|u| u <= t + 60 || u > t + standing::MAX_STANDING_SECS)
        {
            return Err("This standing approval would end too soon or too late. Ask again.".into());
        }
        // The very first key signs its own addition: only it may sign.
        if let Kind::AddKey(card) = &r.kind
            && r.first_key
        {
            allow = vec![card.credential_id.clone()];
        }
        // Keep claims that are still valid, so nobody can swap them while the
        // person is signing. Only make new ones once the old ones ran out.
        if let Some(old) = r.claims.as_ref()
            && old
                .get("exp")
                .and_then(Value::as_i64)
                .is_some_and(|e| e > t)
        {
            let claims_jcs = canonical::canonical_string(old)
                .map_err(|_| "claims: no canonical form".to_string())?;
            return Ok(Started {
                claims_jcs,
                rp_id: self.settings.rp_id.clone(),
                allow,
            });
        }
        let claims = json!({
            "v": VERSION,
            "id": r.id,
            "aud": r.aud,
            "request": r.request,
            "request_hash": r.request_hash,
            "level": r.level.as_str(),
            "iat": t,
            "exp": match r.until {
                Some(until) => until,
                None => t + self.settings.lifetime,
            },
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
        let current = inner.keyring.clone();
        let r = inner.requests.get(id).ok_or("unknown request")?;
        if r.current_status(t) != Status::Pending {
            return Err("This request is no longer waiting.".into());
        }
        let (kind, first_key) = (r.kind.clone(), r.first_key);
        let (aud, request, level) = (r.aud.clone(), r.request.clone(), r.level);
        let claims = r.claims.clone().ok_or("Press Approve first.")?;

        // Which keys may sign, and is the change still possible?
        let signers = match &kind {
            Kind::AddKey(card) if first_key => {
                if !current.keys.is_empty() {
                    return Err("A first key was already added. Start again.".into());
                }
                Keyring::of(vec![card.clone()])
            }
            Kind::AddKey(card) if current.find(&card.credential_id).is_some() => {
                return Err("This key is already trusted.".into());
            }
            Kind::RemoveKey(card) if current.find(&card.credential_id).is_none() => {
                return Err("This key was already removed.".into());
            }
            Kind::RemoveKey(_) if current.keys.len() <= 1 => {
                return Err("You cannot remove your only key.".into());
            }
            _ => current.clone(),
        };
        let obj = json!({"claims": claims, "signatures": [assertion]});
        let approval = b64::encode(
            canonical::canonical_bytes(&obj)
                .map_err(|_| "approval: no canonical form".to_string())?,
        );
        let expected = Expected {
            aud: &aud,
            request: &request,
            level,
        };
        let config = self.settings.config();
        let checked = match &kind {
            Kind::Standing(_) => {
                standing::check_standing(&approval, &expected, &signers, &config, t)
            }
            _ => check(&approval, &expected, &signers, &config, t),
        };
        let ok = checked.map_err(|code| {
            format!("The signature was not accepted ({code}). Nothing was approved.")
        })?;

        // Key changes: build the new keyring, save it, and only then switch to it.
        // Keyrings made before history existed (v0.1.0) stay unsigned.
        let signed = first_key || !current.history.is_empty();
        let mut next = current.clone();
        let key_event = match &kind {
            Kind::AddKey(card) => {
                next.keys.push(card.clone());
                Some(("key_added", card.clone()))
            }
            Kind::RemoveKey(card) => {
                next.keys.retain(|k| k.credential_id != card.credential_id);
                Some(("key_removed", card.clone()))
            }
            _ => None,
        };
        let revoked_standing = match &kind {
            Kind::Revoke(sid) => Some(sid.clone()),
            _ => None,
        };
        if revoked_standing.is_some() {
            let mut list = self.revocations()?;
            list.push(approval.clone());
            let doc = json!({"revocations": list});
            let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())? + "\n";
            util::write_private(
                &self.settings.home.join("revocations.json"),
                text.as_bytes(),
            )?;
        }
        if key_event.is_some() {
            if signed {
                next.origin = Some(self.settings.origin.clone());
                next.history.push(approval.clone());
            }
            self.save_keyring(&next)?;
            inner.keyring = next;
        }

        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        r.status = Status::Approved;
        r.approval = Some(approval);
        r.approved_by = Some(ok.approver.clone());
        r.exp = Some(ok.exp);
        r.usable = matches!(kind, Kind::Standing(_));
        let view = self.view_of(r, t);
        let entry = json!({"id": ok.id, "aud": aud, "level": ok.level.as_str(), "request_hash": ok.request_hash, "credential_id": ok.credential_id});
        inner.audit.append("approved", entry, t)?;
        if let Some(sid) = revoked_standing {
            if let Some(st) = inner.requests.get_mut(&sid) {
                st.revocation_published = true;
            }
            inner.audit.append(
                "revocation_published",
                json!({"standing_id": sid, "approved_with": ok.credential_id}),
                t,
            )?;
        }
        if let Some((event, card)) = key_event {
            inner.audit.append(event, json!({"credential_id": card.credential_id, "name": card.name, "approved_with": ok.credential_id}), t)?;
        }
        drop(inner);
        self.changed.notify_waiters();
        Ok(view)
    }

    /// Keep the service's receipt for an approved request, if it is about it.
    pub fn record_receipt(&self, id: &str, jws: &str) -> Result<View, String> {
        let t = now();
        let mut inner = self.lock()?;
        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        let approval = r
            .approval
            .clone()
            .ok_or("This request was never approved.")?;
        // Only a receipt signed by the service's own pinned key counts. Without a
        // pinned key the gate cannot tell a real receipt from one the agent made up.
        let key = self.service_key(&r.aud).ok_or(
            "No receipt key is pinned for this service, so the receipt cannot be checked. \
             Pin it with `phygital-key trust-service`.",
        )?;
        let rec = if matches!(r.kind, Kind::Standing(_)) {
            // One receipt per use, for a use token this gate handed out.
            let use_id = receipt::peek(jws)
                .and_then(|x| x.use_id)
                .ok_or("This receipt names no use.")?;
            let token = r
                .issued
                .get(&use_id)
                .cloned()
                .ok_or("This use was not handed out by this gate, or already has a receipt.")?;
            let rec = receipt::verify_use_receipt(jws, &key, &r.aud, &approval, &token)
                .map_err(|e| format!("The receipt was not accepted ({e})."))?;
            r.issued.remove(&use_id);
            rec
        } else {
            if r.receipt.is_some() {
                return Err("A receipt was already recorded.".into());
            }
            receipt::verify(jws, &key, &r.aud, &approval)
                .map_err(|e| format!("The receipt was not accepted ({e})."))?
        };
        r.receipt = Some(jws.to_string());
        let view = self.view_of(r, t);
        let entry = json!({"id": rec.approval_id, "use_id": rec.use_id, "aud": rec.iss, "status": rec.status, "reference": rec.reference, "verified": true});
        inner.audit.append("receipt", entry, t)?;
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
        let standing_approved = {
            let inner = self.lock()?;
            let r = inner.requests.get(id).ok_or("unknown request")?;
            matches!(r.kind, Kind::Standing(_)) && r.current_status(now()) == Status::Approved
        };
        if standing_approved {
            return self.revoke(id).map(|(v, _)| v);
        }
        self.close(id, Status::Cancelled, "cancelled")
    }

    /// Revoke an approved standing approval. Two steps:
    /// 1. right away, the use key is thrown away, so no new uses can be signed;
    /// 2. a revocation request is created: when the person signs it, it is
    ///    published at `/.well-known/phygital-key-revoked.json` for services.
    ///
    /// Returns the closed standing view and the link to sign the revocation.
    pub fn revoke(&self, id: &str) -> Result<(View, String), String> {
        let active = {
            let inner = self.lock()?;
            let r = inner.requests.get(id).ok_or("unknown request")?;
            if !matches!(r.kind, Kind::Standing(_)) || r.approval.is_none() {
                return Err("Only an approved standing approval can be revoked.".into());
            }
            r.current_status(now()) == Status::Approved
        };
        // Step 1, at once and for good: no new uses can be signed.
        let view = if active {
            self.close(id, Status::Cancelled, "cancelled")?
        } else {
            self.view(id)?
        };
        // Step 2: the person's signed revocation for the service. Can be retried.
        let url = self.sign_revocation(id)?;
        Ok((view, url))
    }

    /// The link to sign the revocation of a revoked standing approval. Reuses a
    /// waiting one, or makes a new one (after a deny or a timeout).
    pub fn sign_revocation(&self, id: &str) -> Result<String, String> {
        let t = now();
        let (aud, sid) = {
            let mut inner = self.lock()?;
            let r = inner.requests.get(id).ok_or("unknown request")?;
            if !matches!(r.kind, Kind::Standing(_)) || r.approval.is_none() || r.usable {
                return Err("Revoke the standing approval first.".into());
            }
            let (aud, sid) = (r.aud.clone(), r.id.clone());
            // At most ONE revocation request per standing approval. After a deny or
            // a timeout the same request is opened again, never a new one, so
            // nobody can use revocations to flood the gate.
            let existing = inner
                .requests
                .values_mut()
                .find(|x| matches!(&x.kind, Kind::Revoke(s) if s == &sid));
            if let Some(x) = existing {
                if x.approval.is_some() {
                    return Err("The revocation is already signed and published.".into());
                }
                if x.current_status(t) != Status::Pending {
                    x.status = Status::Pending;
                    x.deadline = t + PENDING_SECS;
                    x.claims = None;
                }
                let url = self.settings.approve_url(&x.id);
                drop(inner);
                self.changed.notify_waiters();
                return Ok(url);
            }
            (aud, sid)
        };
        let sign = self.create_kind(
            standing::revoke_request(&sid),
            &aud,
            Level::Medium,
            Kind::Revoke(sid),
        )?;
        Ok(sign.approve_url)
    }

    /// Signed revocations made on this gate (approval strings).
    pub fn revocations(&self) -> Result<Vec<String>, String> {
        match std::fs::read_to_string(self.settings.home.join("revocations.json")) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(format!("revocations.json: {e}")),
            // A damaged file is never quietly replaced: earlier revocations would be lost.
            Ok(t) => serde_json::from_str::<Value>(&t)
                .ok()
                .and_then(|v| v.get("revocations").cloned())
                .and_then(|v| serde_json::from_value(v).ok())
                .ok_or_else(|| "revocations.json is damaged; fix or restore it".to_string()),
        }
    }

    fn close(&self, id: &str, status: Status, event: &str) -> Result<View, String> {
        let t = now();
        let mut inner = self.lock()?;
        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        let revoking = status == Status::Cancelled
            && matches!(r.kind, Kind::Standing(_))
            && r.current_status(t) == Status::Approved;
        if r.current_status(t) != Status::Pending && !revoking {
            return Err("This request is no longer waiting.".into());
        }
        r.status = status;
        r.usable = false;
        let view = self.view_of(r, t);
        let entry = json!({"id": r.id, "aud": r.aud, "request_hash": r.request_hash});
        let rid = r.id.clone();
        // Revoking a standing approval throws its use key away: no more uses can be signed.
        inner.use_keys.remove(&rid);
        inner
            .audit
            .append(if revoking { "revoked" } else { event }, entry, t)?;
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

    fn session_ok(inner: &Inner, token: &str, t: i64) -> bool {
        let h: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        inner
            .enroll
            .as_ref()
            .is_some_and(|e| t < e.expires && bool::from(e.token_hash.ct_eq(&h)))
    }

    /// Step 1 of adding a key. The token file is used up here; the same link
    /// can retry (for example after closing the key prompt) until it expires.
    pub fn enroll_begin(&self, token: &str, name: &str) -> Result<EnrollOptions, String> {
        let t = now();
        let mut inner = self.lock()?;
        let expires = if Self::session_ok(&inner, token, t) {
            inner.enroll.as_ref().map_or(t, |e| e.expires)
        } else if self.token_ok(token) {
            std::fs::remove_file(self.settings.token_path())
                .map_err(|e| format!("could not use up the enrollment token: {e}"))?;
            t + ENROLL_SECS
        } else {
            return Err("This enrollment link is not valid or has expired. Run `phygital-key enroll` for a new one.".into());
        };
        let session = EnrollSession {
            token_hash: Sha256::digest(token.as_bytes()).into(),
            challenge: util::random()?,
            user_id: util::random()?,
            name: sanitize_name(name),
            expires,
        };
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
    ///
    /// With attestation "none" the gate cannot prove the key lives in real
    /// hardware, so the one-time link is what protects the first key.
    pub fn enroll_finish(
        &self,
        token: &str,
        credential_id: &str,
        public_key: &str,
        client_data_json: &str,
    ) -> Result<Enrolled, String> {
        let t = now();
        let mut inner = self.lock()?;
        if !Self::session_ok(&inner, token, t) {
            return Err("This enrollment link is not valid or has expired. Run `phygital-key enroll` for a new one.".into());
        }
        let Some(session) = inner.enroll.as_ref() else {
            return Err("Start again: no enrollment in progress.".into());
        };
        let (challenge, name) = (session.challenge, session.name.clone());
        let cd = b64::decode(client_data_json).ok_or("bad client data")?;
        let origins = vec![self.settings.origin.clone()];
        let want = AssertionCheck {
            kind: "webauthn.create",
            challenge: &challenge,
            origins: &origins,
            rp_id: &self.settings.rp_id,
        };
        if !webauthn::check_client_data(&cd, &want) {
            return Err(
                "The browser's response did not match this page. Start again from the link.".into(),
            );
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
        let card = KeyCard {
            credential_id: credential_id.to_string(),
            public_key: public_key.to_string(),
            rp_id: self.settings.rp_id.clone(),
            name,
            created: util::date(t),
        };
        inner.enroll = None;
        drop(inner);
        let aud = self.settings.keyring_aud();
        let view = self.create_kind(
            history::add_key_request(&card),
            &aud,
            Level::High,
            Kind::AddKey(card),
        )?;
        Ok(Enrolled {
            approve_url: view.approve_url,
            first_key: view.first_key,
        })
    }

    /// Ask the person for a standing approval: `params` without `use_key`
    /// (the gate makes a fresh one), valid for `days` (1 to 30).
    pub fn create_standing(
        &self,
        for_type: &str,
        fixed: serde_json::Map<String, Value>,
        vary: std::collections::BTreeMap<String, standing::Limit>,
        max_uses: u32,
        days: i64,
        aud: &str,
    ) -> Result<View, String> {
        if !(1..=30).contains(&days) {
            return Err("days must be 1 to 30".into());
        }
        let key = SigningKey::from_slice(&util::random::<32>()?).map_err(|e| e.to_string())?;
        let der = key
            .verifying_key()
            .to_public_key_der()
            .map_err(|e| e.to_string())?;
        let params = StandingParams {
            for_type: for_type.to_string(),
            fixed,
            vary,
            max_uses,
            use_key: b64::encode(der.as_bytes()),
        };
        let request = params.to_request();
        StandingParams::from_request(&request).map_err(|_| {
            "Not a valid standing approval: for_type is a lowercase action name, max_uses 1 to 1000, \
             fixed and vary fields must not overlap, vary limits are {\"min\": n, \"max\": n} or {\"one_of\": [up to 20 strings]}."
                .to_string()
        })?;
        self.create_full(
            request,
            aud,
            Level::High,
            Kind::Standing(params),
            Some((days, key)),
        )
    }

    /// Sign one use of an approved standing approval for this exact request.
    pub fn use_standing(&self, id: &str, request: &Value) -> Result<SignedUse, String> {
        let t = now();
        let mut inner = self.lock()?;
        let key = inner.use_keys.get(id).cloned().ok_or(
            "This standing approval cannot be used: unknown, revoked, or the gate restarted.",
        )?;
        let r = inner.requests.get_mut(id).ok_or("unknown request")?;
        let Kind::Standing(params) = &r.kind else {
            return Err("This is not a standing approval.".into());
        };
        if r.current_status(t) != Status::Approved || !r.usable {
            return Err("This standing approval is not active.".into());
        }
        if !params.allows(request) {
            return Err("This request is outside the limits the person approved. Ask for a one-time approval instead.".into());
        }
        if r.uses >= params.max_uses {
            return Err("No uses left on this standing approval.".into());
        }
        let exp = (t + standing::MAX_USE_SECS).min(r.exp.unwrap_or(t));
        let u = Use {
            v: "pka.use.1".into(),
            standing_id: r.id.clone(),
            id: util::new_id()?,
            aud: r.aud.clone(),
            request: request.clone(),
            request_hash: canonical::hash_b64(request)
                .map_err(|_| "request: no canonical form".to_string())?,
            iat: t,
            exp,
        };
        let use_token = standing::sign_use(&u, &key).map_err(|e| e.to_string())?;
        r.uses += 1;
        r.issued.insert(u.id.clone(), use_token.clone());
        let out = SignedUse {
            approval: r.approval.clone().unwrap_or_default(),
            use_token,
            uses_left: params.max_uses - r.uses,
        };
        let entry =
            json!({"id": r.id, "use_id": u.id, "aud": r.aud, "request_hash": u.request_hash});
        inner.audit.append("standing_used", entry, t)?;
        drop(inner);
        self.changed.notify_waiters();
        Ok(out)
    }

    /// Ask to remove a key. A trusted key must approve it (level high).
    pub fn remove_key(&self, credential_id: &str) -> Result<View, String> {
        let card = {
            let inner = self.lock()?;
            if inner.keyring.keys.len() <= 1 {
                return Err("You cannot remove your only key. Add another key first.".into());
            }
            let t = now();
            let already = inner.requests.values().any(|r| {
                r.current_status(t) == Status::Pending
                    && matches!(&r.kind, Kind::RemoveKey(c) if c.credential_id == credential_id)
            });
            if already {
                return Err("A removal of this key is already waiting for approval.".into());
            }
            inner
                .keyring
                .find(credential_id)
                .cloned()
                .ok_or("unknown key")?
        };
        let aud = self.settings.keyring_aud();
        self.create_kind(
            history::remove_key_request(&card),
            &aud,
            Level::High,
            Kind::RemoveKey(card),
        )
    }
}

/// The MCP access token for HTTP mode: made once, kept in `<home>/mcp-token`.
pub fn mcp_token(settings: &Settings) -> Result<String, String> {
    util::private_dir(&settings.home)?;
    let path = settings.home.join("mcp-token");
    if let Ok(t) = std::fs::read_to_string(&path) {
        let t = t.trim().to_string();
        if t.len() >= 32 {
            return Ok(t);
        }
    }
    let token = b64::encode(util::random::<32>()?);
    util::write_private(&path, format!("{token}\n").as_bytes())?;
    Ok(token)
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
