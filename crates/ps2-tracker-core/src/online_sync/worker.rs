//! Sign-in, token refresh and background uploads of the online sync.
//!
//! - Nothing happens while the sync is switched off: no keyring access and no
//!   request to the stats server (an explicit "Sign out" still revokes the
//!   stored sign-in).
//! - The refresh token is read from and written to the keyring only; the
//!   access token stays in memory. Refresh tokens are single use, so every
//!   refresh writes the new one back before the access token is used.
//! - Uploads run one after the other in the background, with their own HTTP
//!   client; Census requests and the UI never wait for them.
//! - Only sessions of the user's own characters that the server lists for the
//!   account are uploaded; squad sessions never.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::sync::Notify;

use super::api::{Account, ApiError, ErrorClass, SyncApi, Token, classify};
use super::loopback::Loopback;
use super::persist::QueueCounts;
use super::queue::{Attempt, apply};
use super::secret::{SecretError, SecretStore};
use super::{LOGIN_TIMEOUT_SECS, REDIRECT_PORTS, SyncSettings, authorize_url, pkce, privacy_url};
use crate::census::SharedNames;
use crate::session::SessionConfig;
use crate::sessionfile::{MAX_FILE_BYTES, SessionFile};
use crate::store::{Store, StoreError};

/// Entries uploaded per pass at most (the server allows 60 per hour).
const PASS_LIMIT: u32 = 20;
/// Longest sleep of the background loop between two passes.
const IDLE_SECS: i64 = 60;
/// An access token this close to its end is refreshed first.
const EXPIRY_MARGIN_SECS: i64 = 60;

pub type SecretFactory = Arc<dyn Fn(&str) -> Arc<dyn SecretStore> + Send + Sync>;
pub type BrowserOpener = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;
pub type ChangeHook = Arc<dyn Fn() + Send + Sync>;

/// What the worker needs from the app.
#[derive(Clone)]
pub struct Deps {
    pub store: Arc<Store>,
    pub names: SharedNames,
    /// Keyring entry for an API base (see [`super::keyring_account`]).
    pub secrets: SecretFactory,
    /// Opens a URL in the system browser.
    pub open_browser: BrowserOpener,
    /// Called after every change of the status or the queue.
    pub on_change: ChangeHook,
}

#[derive(Debug, Clone)]
enum Auth {
    /// Not looked into the keyring yet.
    Unknown,
    SignedOut,
    /// A refresh token is in the keyring; no access token yet.
    Stored,
    Active {
        access: Token,
        expires_at: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyringState {
    Unknown,
    Available,
    Unavailable,
}

struct Inner {
    settings: SyncSettings,
    own: HashSet<u64>,
    api: Option<SyncApi>,
    secret: Arc<dyn SecretStore>,
    auth: Auth,
    account: Option<Account>,
    keyring: KeyringState,
    keyring_error: Option<String>,
    login: Option<Arc<Notify>>,
    last_error: Option<String>,
    /// The server answered 503 `sync_disabled`/`unavailable` last time.
    server_unavailable: bool,
    ports: Vec<u16>,
}

/// Everything the settings page shows.
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub api_base: String,
    pub privacy_url: Option<String>,
    /// A Discord client ID is built in or configured.
    pub client_id_available: bool,
    pub keyring: KeyringState,
    pub keyring_error: Option<String>,
    /// A sign-in is stored (keyring) or active.
    pub signed_in: bool,
    pub login_pending: bool,
    pub account: Option<Account>,
    pub last_error: Option<String>,
    pub server_unavailable: bool,
    pub counts: QueueCounts,
}

/// Why no access token is available.
enum NoToken {
    SignedOut,
    Failed(String),
}

pub struct SyncWorker {
    deps: Deps,
    inner: Mutex<Inner>,
    refresh_lock: tokio::sync::Mutex<()>,
    wake: Notify,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, SecretError> + Send + 'static,
) -> Result<T, SecretError> {
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| Err(SecretError::Failed(format!("keyring task: {e}"))))
}

impl SyncWorker {
    pub fn new(deps: Deps, settings: SyncSettings, own: HashSet<u64>) -> Arc<Self> {
        let base = settings.base();
        let secret = (deps.secrets)(&super::keyring_account(&base));
        Arc::new(Self {
            inner: Mutex::new(Inner {
                api: SyncApi::new(&base).ok(),
                settings,
                own,
                secret,
                auth: Auth::Unknown,
                account: None,
                keyring: KeyringState::Unknown,
                keyring_error: None,
                login: None,
                last_error: None,
                server_unavailable: false,
                ports: REDIRECT_PORTS.to_vec(),
            }),
            deps,
            refresh_lock: tokio::sync::Mutex::new(()),
            wake: Notify::new(),
        })
    }

    /// Loopback ports to try (tests use port 0).
    pub fn set_ports(&self, ports: Vec<u16>) {
        lock(&self.inner).ports = ports;
    }

    /// New settings or own characters. A different API address starts over
    /// with that server's keyring entry.
    pub fn configure(&self, settings: SyncSettings, own: HashSet<u64>) {
        {
            let mut i = lock(&self.inner);
            if settings.base() != i.settings.base() {
                let base = settings.base();
                i.api = SyncApi::new(&base).ok();
                i.secret = (self.deps.secrets)(&super::keyring_account(&base));
                i.auth = Auth::Unknown;
                i.account = None;
                i.last_error = None;
                i.server_unavailable = false;
            }
            i.settings = settings;
            i.own = own;
        }
        self.changed();
    }

    fn changed(&self) {
        self.wake.notify_one();
        (self.deps.on_change)();
    }

    pub fn status(&self) -> SyncStatus {
        let counts = self.deps.store.sync_counts().unwrap_or_default();
        let i = lock(&self.inner);
        let base = i.settings.base();
        SyncStatus {
            enabled: i.settings.enabled,
            privacy_url: privacy_url(&base),
            api_base: base,
            client_id_available: i.settings.effective_client_id().is_some(),
            keyring: i.keyring,
            keyring_error: i.keyring_error.clone(),
            signed_in: matches!(i.auth, Auth::Stored | Auth::Active { .. }),
            login_pending: i.login.is_some(),
            account: i.account.clone(),
            last_error: i.last_error.clone(),
            server_unavailable: i.server_unavailable,
            counts,
        }
    }

    /// May a session of this character be queued at all?
    fn may_upload(i: &Inner, character_id: u64) -> Result<(), String> {
        if !i.own.contains(&character_id) {
            return Err("only sessions of your own characters are uploaded".into());
        }
        if let Some(a) = &i.account
            && !a.allows(character_id)
        {
            return Err("this character is not linked to your ELME account".into());
        }
        Ok(())
    }

    /// A session ended: queue it if the sync is on and the character is one
    /// of the user's own (and, once known, linked to the account).
    pub fn session_finished(&self, session_id: i64, character_id: u64, now: i64) -> bool {
        {
            let i = lock(&self.inner);
            if !i.settings.enabled || Self::may_upload(&i, character_id).is_err() {
                return false;
            }
        }
        match self.deps.store.sync_enqueue(session_id, now) {
            Ok(q) => {
                if q {
                    self.changed();
                }
                q
            }
            Err(e) => {
                tracing::warn!(error = %e, "session not queued for online sync");
                false
            }
        }
    }

    /// "Upload now" / "Retry" for one session.
    pub fn upload_now(&self, session_id: i64, now: i64) -> Result<(), String> {
        let cid = self
            .deps
            .store
            .session_character(session_id)
            .map_err(|e| e.to_string())?
            .ok_or("session not found")?;
        {
            let i = lock(&self.inner);
            if !i.settings.enabled {
                return Err("online sync is switched off".into());
            }
            Self::may_upload(&i, cid)?;
        }
        self.deps
            .store
            .sync_enqueue(session_id, now)
            .map_err(|e| e.to_string())?;
        self.changed();
        Ok(())
    }

    /// "Upload existing sessions": queues every ended session of the own
    /// characters linked to the account that was never queued.
    pub fn upload_existing(&self, now: i64) -> Result<usize, String> {
        let ids: Vec<u64> = {
            let i = lock(&self.inner);
            if !i.settings.enabled {
                return Err("online sync is switched off".into());
            }
            let Some(a) = &i.account else {
                return Err("sign in first".into());
            };
            a.characters
                .iter()
                .map(|c| c.character_id)
                .filter(|c| i.own.contains(c))
                .collect()
        };
        let n = self
            .deps
            .store
            .sync_enqueue_missing(&ids, now)
            .map_err(|e| e.to_string())?;
        self.changed();
        Ok(n)
    }

    fn api(&self) -> Result<SyncApi, String> {
        lock(&self.inner)
            .api
            .clone()
            .ok_or_else(|| "online sync: HTTP client not available".into())
    }

    fn secret(&self) -> Arc<dyn SecretStore> {
        lock(&self.inner).secret.clone()
    }

    fn set_keyring(&self, r: &Result<(), SecretError>) {
        let mut i = lock(&self.inner);
        match r {
            Ok(()) => {
                i.keyring = KeyringState::Available;
                i.keyring_error = None;
            }
            Err(SecretError::Unavailable(e)) => {
                i.keyring = KeyringState::Unavailable;
                i.keyring_error = Some(e.clone());
            }
            Err(SecretError::Failed(e)) => {
                i.keyring_error = Some(e.clone());
            }
        }
    }

    /// Signs out locally: keyring entry gone, tokens forgotten.
    async fn forget_sign_in(&self, reason: Option<String>) -> Result<(), String> {
        let secret = self.secret();
        let r = blocking(move || secret.delete()).await;
        {
            let mut i = lock(&self.inner);
            i.auth = Auth::SignedOut;
            i.account = None;
            i.last_error = reason;
        }
        self.changed();
        r.map_err(|e| e.to_string())
    }

    /// Looks into the keyring once (after a start or a change of server).
    async fn restore(&self) {
        if !matches!(lock(&self.inner).auth, Auth::Unknown) {
            return;
        }
        let secret = self.secret();
        let r = blocking(move || secret.get()).await;
        self.set_keyring(&r.as_ref().map(|_| ()).map_err(Clone::clone));
        {
            let mut i = lock(&self.inner);
            i.auth = match r {
                Ok(Some(_)) => Auth::Stored,
                _ => Auth::SignedOut,
            };
        }
        self.changed();
    }

    /// A valid access token, refreshing it with the stored refresh token if
    /// needed (one refresh at a time: refresh tokens are single use).
    async fn access_token(&self, now: i64) -> Result<Token, NoToken> {
        let _guard = self.refresh_lock.lock().await;
        match &lock(&self.inner).auth {
            Auth::Active { access, expires_at } if *expires_at - EXPIRY_MARGIN_SECS > now => {
                return Ok(access.clone());
            }
            Auth::SignedOut | Auth::Unknown => return Err(NoToken::SignedOut),
            _ => {}
        }
        let secret = self.secret();
        let stored = blocking(move || secret.get()).await;
        let refresh = match stored {
            Ok(Some(t)) => Token::new(t),
            Ok(None) => {
                lock(&self.inner).auth = Auth::SignedOut;
                self.changed();
                return Err(NoToken::SignedOut);
            }
            Err(e) => {
                self.set_keyring(&Err(e.clone()));
                return Err(NoToken::Failed(e.to_string()));
            }
        };
        let api = self.api().map_err(NoToken::Failed)?;
        match api.refresh(&refresh).await {
            Ok(r) => {
                let secret = self.secret();
                let new_refresh = r.refresh_token.expose().to_owned();
                if let Err(e) = blocking(move || secret.set(&new_refresh)).await {
                    // the old token is dead now; without the new one the
                    // sign-in cannot continue
                    self.set_keyring(&Err(e.clone()));
                    let _ = self
                        .forget_sign_in(Some(format!("sign-in not kept: {e}")))
                        .await;
                    return Err(NoToken::SignedOut);
                }
                {
                    let mut i = lock(&self.inner);
                    i.auth = Auth::Active {
                        access: r.access_token.clone(),
                        expires_at: now + r.access_expires_in.max(0),
                    };
                    i.account = Some(r.account);
                    i.server_unavailable = false;
                    i.last_error = None;
                }
                self.changed();
                Ok(r.access_token)
            }
            Err(e) if e.status() == Some(401) => {
                let _ = self
                    .forget_sign_in(Some(
                        "your sign-in has expired or was revoked – sign in again".into(),
                    ))
                    .await;
                Err(NoToken::SignedOut)
            }
            Err(e) => {
                self.note_error(&e);
                Err(NoToken::Failed(e.to_string()))
            }
        }
    }

    fn note_error(&self, e: &ApiError) {
        let unavailable = matches!(e, ApiError::Status { status: 503, .. });
        {
            let mut i = lock(&self.inner);
            i.server_unavailable = unavailable;
            i.last_error = Some(if unavailable {
                "online sync is not available on the server right now".into()
            } else {
                e.to_string()
            });
        }
        self.changed();
    }

    /// The next upload must refresh first (the server refused the token).
    fn drop_access(&self) {
        let mut i = lock(&self.inner);
        if matches!(i.auth, Auth::Active { .. }) {
            i.auth = Auth::Stored;
        }
    }

    /// One pass: restores the sign-in if needed and uploads due entries.
    /// Returns the number of attempts made.
    pub async fn step(&self, now: i64) -> usize {
        if !lock(&self.inner).settings.enabled {
            return 0;
        }
        self.restore().await;
        if !matches!(lock(&self.inner).auth, Auth::Stored | Auth::Active { .. }) {
            return 0;
        }
        let due = match self.deps.store.sync_due(now, PASS_LIMIT) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "online sync queue not readable");
                return 0;
            }
        };
        if due.is_empty() {
            return 0;
        }
        let mut token = match self.access_token(now).await {
            Ok(t) => t,
            Err(_) => return 0,
        };
        if lock(&self.inner)
            .account
            .as_ref()
            .is_some_and(|a| !a.upload_allowed)
        {
            // membership lost: uploads are closed, the queue waits
            return 0;
        }
        let api = match self.api() {
            Ok(a) => a,
            Err(_) => return 0,
        };
        let mut attempts = 0;
        for d in due {
            let check = {
                let i = lock(&self.inner);
                Self::may_upload(&i, d.character_id)
            };
            let attempt = match check {
                Err(reason) => Attempt::Rejected { reason },
                Ok(()) => {
                    let body = match self.session_body(d.entry.session_id) {
                        Ok(Some(b)) => b,
                        Ok(None) => continue,
                        Err(reason) => {
                            self.record(&d, Attempt::Rejected { reason }, now);
                            continue;
                        }
                    };
                    attempts += 1;
                    let mut r = api
                        .upload(&token, d.character_id, d.started_at, body.clone())
                        .await;
                    if r.as_ref()
                        .is_err_and(|e| classify(e) == ErrorClass::Unauthorized)
                    {
                        // once: refresh and try again
                        self.drop_access();
                        match self.access_token(now).await {
                            Ok(t) => {
                                token = t;
                                r = api.upload(&token, d.character_id, d.started_at, body).await;
                            }
                            Err(_) => return attempts,
                        }
                    }
                    match r {
                        Ok(_) => Attempt::Uploaded,
                        Err(e) => match self.judge(&e) {
                            Judged::Record(a) => a,
                            Judged::Stop(a) => {
                                if let Some(a) = a {
                                    self.record(&d, a, now);
                                }
                                return attempts;
                            }
                        },
                    }
                }
            };
            self.record(&d, attempt, now);
        }
        attempts
    }

    fn record(&self, d: &super::persist::DueEntry, a: Attempt, now: i64) {
        let e = apply(d.entry.clone(), &a, now);
        if let Err(err) = self.deps.store.sync_record(&e, d.generation, now) {
            tracing::warn!(error = %err, "online sync result not saved");
        }
        match &a {
            Attempt::Uploaded => {
                tracing::info!(session = d.entry.session_id, "session uploaded")
            }
            Attempt::Retry { reason, .. } => {
                tracing::info!(session = d.entry.session_id, %reason, "upload failed, will retry")
            }
            Attempt::Rejected { reason } => {
                tracing::warn!(session = d.entry.session_id, %reason, "upload rejected")
            }
        }
        self.changed();
    }

    /// The upload body: the session file as the export writes it.
    fn session_body(&self, session_id: i64) -> Result<Option<String>, String> {
        let s = match self
            .deps
            .store
            .load_session(session_id, SessionConfig::default())
        {
            Ok(s) => s,
            Err(StoreError::NotFound(_)) => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        if s.ended_at.is_none() {
            return Ok(None);
        }
        let file = {
            let names = lock(&self.deps.names);
            SessionFile::new(&s, &names)
        };
        let body = serde_json::to_string(&file).map_err(|e| e.to_string())?;
        if body.len() as u64 > MAX_FILE_BYTES {
            return Err("session file larger than 25 MiB".into());
        }
        Ok(Some(body))
    }

    fn judge(&self, e: &ApiError) -> Judged {
        let code = match e {
            ApiError::Status { code, .. } => code.as_deref().unwrap_or(""),
            _ => "",
        };
        match (classify(e), e.status(), code) {
            (_, Some(403), "membership_lost") => {
                if let Some(a) = lock(&self.inner).account.as_mut() {
                    a.upload_allowed = false;
                }
                self.changed();
                Judged::Stop(None)
            }
            (_, Some(503), _) => {
                self.note_error(e);
                Judged::Stop(None)
            }
            (ErrorClass::Unauthorized, ..) => Judged::Stop(Some(Attempt::Retry {
                reason: e.to_string(),
                retry_after: None,
            })),
            (ErrorClass::Permanent, ..) => Judged::Record(Attempt::Rejected {
                reason: e.to_string(),
            }),
            (ErrorClass::Transient, status, _) => {
                let retry_after = match e {
                    ApiError::Status { retry_after, .. } => *retry_after,
                    _ => None,
                };
                let a = Attempt::Retry {
                    reason: e.to_string(),
                    retry_after,
                };
                if status == Some(429) {
                    // the limit is per account: the rest waits as well
                    Judged::Stop(Some(a))
                } else {
                    Judged::Record(a)
                }
            }
        }
    }

    /// Background loop: a pass now, then whenever something was queued or
    /// the next entry is due (at most a minute apart).
    pub async fn run(self: Arc<Self>, now: fn() -> i64) {
        loop {
            self.step(now()).await;
            let wait = match self.deps.store.sync_next_due() {
                Ok(Some(t)) => (t - now()).clamp(1, IDLE_SECS),
                _ => IDLE_SECS,
            };
            tokio::select! {
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(wait as u64)) => {}
            }
        }
    }

    /// Discord sign-in: loopback listener, system browser, code exchange,
    /// refresh token into the keyring. Without a keyring nothing is sent.
    pub async fn login(&self, now: fn() -> i64) -> Result<SyncStatus, String> {
        let cancel = Arc::new(Notify::new());
        let (client_id, ports) = {
            let mut i = lock(&self.inner);
            if !i.settings.enabled {
                return Err("switch online sync on first".into());
            }
            let Some(id) = i.settings.effective_client_id() else {
                return Err("this build has no Discord client ID for online sync".into());
            };
            if i.login.is_some() {
                return Err("a sign-in is already running".into());
            }
            i.login = Some(cancel.clone());
            (id, i.ports.clone())
        };
        self.changed();
        let r = self.login_inner(&client_id, &ports, cancel, now).await;
        {
            let mut i = lock(&self.inner);
            i.login = None;
            if let Err(e) = &r {
                i.last_error = Some(e.clone());
            }
        }
        self.changed();
        r.map(|()| self.status())
    }

    async fn login_inner(
        &self,
        client_id: &str,
        ports: &[u16],
        cancel: Arc<Notify>,
        now: fn() -> i64,
    ) -> Result<(), String> {
        let secret = self.secret();
        let probe = blocking(move || secret.probe()).await;
        self.set_keyring(&probe);
        if let Err(e) = probe {
            return Err(format!(
                "no keyring available, so the sign-in cannot be kept ({e}). \
                 Install or unlock a Secret Service keyring (e.g. GNOME Keyring or KWallet)."
            ));
        }
        let p = pkce::Pkce::new().map_err(|e| format!("random numbers: {e}"))?;
        let state = pkce::random_token().map_err(|e| format!("random numbers: {e}"))?;
        let lb = Loopback::bind(ports).await.map_err(|e| e.to_string())?;
        let redirect = lb.redirect_uri();
        let url = authorize_url(client_id, &redirect, &p.challenge, &state);
        (self.deps.open_browser)(&url)?;
        let code = tokio::select! {
            r = lb.wait(&state, Duration::from_secs(LOGIN_TIMEOUT_SECS)) => r.map_err(|e| e.to_string())?,
            _ = cancel.notified() => return Err("sign-in cancelled".into()),
        };
        let api = self.api()?;
        let r = api
            .exchange(&code, &p.verifier, &redirect)
            .await
            .map_err(|e| login_error(&e))?;
        let secret = self.secret();
        let refresh = r.refresh_token.expose().to_owned();
        if let Err(e) = blocking(move || secret.set(&refresh)).await {
            self.set_keyring(&Err(e.clone()));
            // do not leave a live sign-in on the server that nobody holds
            let _ = api.logout(&r.refresh_token).await;
            return Err(format!("sign-in not kept: {e}"));
        }
        {
            let mut i = lock(&self.inner);
            i.auth = Auth::Active {
                access: r.access_token,
                expires_at: now() + r.access_expires_in.max(0),
            };
            i.account = Some(r.account);
            i.last_error = None;
            i.server_unavailable = false;
        }
        Ok(())
    }

    pub fn cancel_login(&self) {
        if let Some(c) = lock(&self.inner).login.as_ref() {
            c.notify_one();
        }
    }

    /// Signs out: revokes the sign-in on the server (best effort) and
    /// deletes the keyring entry.
    pub async fn logout(&self) -> Result<SyncStatus, String> {
        let secret = self.secret();
        let stored = blocking(move || secret.get()).await;
        if let Ok(Some(t)) = stored
            && let Ok(api) = self.api()
            && let Err(e) = api.logout(&Token::new(t)).await
        {
            tracing::warn!(error = %e, "sign-out not confirmed by the stats server");
        }
        self.forget_sign_in(None).await?;
        Ok(self.status())
    }

    /// Deletes the account and every uploaded session on the server, then
    /// signs out locally and forgets the local sync states.
    pub async fn delete_account(&self, now: i64) -> Result<SyncStatus, String> {
        self.restore().await;
        let token = match self.access_token(now).await {
            Ok(t) => t,
            Err(NoToken::SignedOut) => return Err("sign in first".into()),
            Err(NoToken::Failed(e)) => return Err(e),
        };
        self.api()?
            .delete_account(&token)
            .await
            .map_err(|e| e.to_string())?;
        if let Err(e) = self.deps.store.sync_clear() {
            tracing::warn!(error = %e, "local sync states not cleared");
        }
        self.forget_sign_in(None).await?;
        Ok(self.status())
    }
}

enum Judged {
    /// Write this result and go on with the next entry.
    Record(Attempt),
    /// Stop the pass (optionally after writing a result).
    Stop(Option<Attempt>),
}

/// Sign-in error → message for the user.
fn login_error(e: &ApiError) -> String {
    let code = match e {
        ApiError::Status { code, .. } => code.as_deref(),
        _ => None,
    };
    match (e.status(), code) {
        (Some(403), _) => "your Discord account is not linked to an ELME character. \
                            Verify in the ELME Discord first, then sign in again."
            .into(),
        (Some(401), _) => "Discord did not accept the sign-in – please try again.".into(),
        (Some(503), _) => "online sync is not available on the server right now".into(),
        (Some(429), _) => "too many sign-in attempts – please wait a few minutes".into(),
        _ => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::api::mock::{Request, Response, start};
    use super::super::secret::MemorySecret;
    use super::*;
    use crate::census::NameCache;
    use crate::online_sync::queue::SyncState;
    use crate::session::Session;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ME: u64 = 5428000000000000001;
    const SQUAD: u64 = 5428000000000000002;
    const T0: i64 = 1_790_000_000;

    fn t0() -> i64 {
        T0
    }

    fn auth(access: &str, refresh: &str) -> serde_json::Value {
        super::super::api::tests::auth_json(access, refresh)
    }

    struct Rig {
        worker: Arc<SyncWorker>,
        store: Arc<Store>,
        secret: Arc<MemorySecret>,
        changes: Arc<AtomicUsize>,
    }

    fn rig(base: &str, secret: MemorySecret, opener: BrowserOpener) -> Rig {
        let store = Arc::new(Store::in_memory().unwrap());
        let secret = Arc::new(secret);
        let s2 = secret.clone();
        let changes = Arc::new(AtomicUsize::new(0));
        let c2 = changes.clone();
        let deps = Deps {
            store: store.clone(),
            names: Arc::new(Mutex::new(NameCache::default())),
            secrets: Arc::new(move |_| s2.clone() as Arc<dyn SecretStore>),
            open_browser: opener,
            on_change: Arc::new(move || {
                c2.fetch_add(1, Ordering::Relaxed);
            }),
        };
        let settings = SyncSettings {
            enabled: true,
            api_base: base.into(),
            discord_client_id: crate::online_sync::test_snowflake('7'),
        };
        let worker = SyncWorker::new(deps, settings, HashSet::from([ME]));
        worker.set_ports(vec![0]);
        Rig {
            worker,
            store,
            secret,
            changes,
        }
    }

    fn no_browser() -> BrowserOpener {
        Arc::new(|_| Err("no browser in this test".into()))
    }

    fn ended_session(store: &Store, cid: u64, start: i64) -> i64 {
        let mut s = Session::new(cid, "Me", Some(1), start, SessionConfig::default());
        s.end(start + 600);
        store.save_session(&mut s).unwrap()
    }

    /// A browser that follows the authorization URL straight to the
    /// redirect URI with a code (Discord approved), keeping what it saw.
    fn approving_browser(seen: Arc<Mutex<Option<reqwest::Url>>>) -> BrowserOpener {
        Arc::new(move |url| {
            let u = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
            *seen.lock().unwrap() = Some(u.clone());
            let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
            let target = format!("{}?code=the-code&state={}", q["redirect_uri"], q["state"]);
            tokio::spawn(async move {
                let _ = reqwest::get(target).await;
            });
            Ok(())
        })
    }

    #[tokio::test]
    async fn sign_in_end_to_end() {
        let srv = start(Arc::new(|r: &Request| match r.path.as_str() {
            "/auth/discord" => Response::json(200, auth("acc-1", "ref-1")),
            _ => Response::empty(404),
        }))
        .await;
        let seen = Arc::new(Mutex::new(None));
        let r = rig(
            &srv.base,
            MemorySecret::default(),
            approving_browser(seen.clone()),
        );
        let st = r.worker.login(t0).await.unwrap();
        assert!(st.signed_in);
        assert_eq!(st.keyring, KeyringState::Available);
        assert!(st.account.unwrap().allows(ME));
        assert_eq!(r.secret.get().unwrap().as_deref(), Some("ref-1"));
        // the verifier sent to the server belongs to the challenge Discord got
        let url = seen.lock().unwrap().clone().unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        let reqs = srv.requests();
        assert_eq!(reqs.len(), 1);
        let body: serde_json::Value = serde_json::from_str(&reqs[0].body).unwrap();
        assert_eq!(body["code"], "the-code");
        assert_eq!(body["redirect_uri"], q["redirect_uri"]);
        assert_eq!(
            pkce::challenge(body["code_verifier"].as_str().unwrap()),
            q["code_challenge"]
        );
        assert!(q["redirect_uri"].starts_with("http://127.0.0.1:"));
        assert!(r.changes.load(Ordering::Relaxed) > 0);
    }

    #[tokio::test]
    async fn sign_in_not_linked_and_without_keyring() {
        let srv = start(Arc::new(|_: &Request| {
            Response::json(
                403,
                json!({"error": "not_linked", "message": "verify first"}),
            )
        }))
        .await;
        let r = rig(
            &srv.base,
            MemorySecret::default(),
            approving_browser(Arc::new(Mutex::new(None))),
        );
        let e = r.worker.login(t0).await.unwrap_err();
        assert!(e.contains("not linked to an ELME character"), "{e}");
        assert!(!r.worker.status().signed_in);
        assert_eq!(r.secret.get().unwrap(), None);
        // no keyring: refused before anything is sent
        let srv2 = start(Arc::new(|_: &Request| Response::empty(500))).await;
        let r = rig(&srv2.base, MemorySecret::unavailable(), no_browser());
        let e = r.worker.login(t0).await.unwrap_err();
        assert!(e.contains("no keyring"), "{e}");
        assert_eq!(r.worker.status().keyring, KeyringState::Unavailable);
        assert!(srv2.requests().is_empty());
    }

    #[tokio::test]
    async fn switched_off_means_no_request_and_no_queue() {
        let srv = start(Arc::new(|_: &Request| Response::empty(500))).await;
        let r = rig(&srv.base, MemorySecret::default(), no_browser());
        r.secret.set("ref-0").unwrap();
        let mut off = lock(&r.worker.inner).settings.clone();
        off.enabled = false;
        r.worker.configure(off, HashSet::from([ME]));
        let id = ended_session(&r.store, ME, T0);
        assert!(!r.worker.session_finished(id, ME, T0));
        assert_eq!(r.worker.step(T0).await, 0);
        assert!(r.worker.login(t0).await.is_err());
        assert!(r.worker.upload_now(id, T0).is_err());
        assert!(srv.requests().is_empty());
        assert_eq!(r.store.sync_entry(id).unwrap(), None);
    }

    #[tokio::test]
    async fn uploads_with_refresh_rotation_and_retry_on_401() {
        let puts = Arc::new(AtomicUsize::new(0));
        let p2 = puts.clone();
        let srv = start(Arc::new(move |r: &Request| {
            match (r.method.as_str(), r.path.as_str()) {
                ("POST", "/auth/refresh") if r.body.contains("ref-0") => {
                    Response::json(200, auth("acc-1", "ref-1"))
                }
                ("POST", "/auth/refresh") if r.body.contains("ref-1") => {
                    Response::json(200, auth("acc-2", "ref-2"))
                }
                ("PUT", _) => {
                    // the first access token is refused once
                    if r.header("authorization") == Some("Bearer acc-1") {
                        Response::json(401, json!({"error": "invalid_token"}))
                    } else {
                        p2.fetch_add(1, Ordering::Relaxed);
                        Response::json(201, json!({}))
                    }
                }
                _ => Response::empty(404),
            }
        }))
        .await;
        let r = rig(&srv.base, MemorySecret::default(), no_browser());
        r.secret.set("ref-0").unwrap();
        let id = ended_session(&r.store, ME, T0);
        let squad = ended_session(&r.store, SQUAD, T0);
        assert!(r.worker.session_finished(id, ME, T0 + 600));
        assert!(!r.worker.session_finished(squad, SQUAD, T0 + 600), "squad");
        assert_eq!(r.worker.step(T0 + 600).await, 1);
        assert_eq!(puts.load(Ordering::Relaxed), 1);
        let e = r.store.sync_entry(id).unwrap().unwrap();
        assert_eq!(e.state, SyncState::Synced);
        // rotated twice, the newest refresh token is stored
        assert_eq!(r.secret.get().unwrap().as_deref(), Some("ref-2"));
        let reqs = srv.requests();
        let put = reqs.iter().rfind(|q| q.method == "PUT").unwrap();
        assert_eq!(put.path, format!("/sessions/{ME}/{T0}"));
        // the body is a valid session file of that session
        let f = crate::sessionfile::from_json(&put.body).unwrap();
        f.validate(T0 + 700).unwrap();
        assert_eq!((f.session.character_id, f.session.started_at), (ME, T0));
        assert!(!reqs.iter().any(|q| q.path.contains(&SQUAD.to_string())));
        // nothing due any more: no request
        let before = srv.requests().len();
        assert_eq!(r.worker.step(T0 + 700).await, 0);
        assert_eq!(srv.requests().len(), before);
        // the account's characters drive "Upload existing sessions"
        let old = ended_session(&r.store, ME, T0 - 86_400);
        assert_eq!(r.worker.upload_existing(T0 + 800).unwrap(), 1);
        assert!(r.store.sync_entry(old).unwrap().is_some());
    }

    #[tokio::test]
    async fn rejections_rate_limits_and_server_errors() {
        let srv = start(Arc::new(|r: &Request| {
            match (r.method.as_str(), r.path.as_str()) {
                ("POST", "/auth/refresh") => Response::json(200, auth("acc", "ref")),
                ("PUT", p) if p.ends_with(&format!("/{T0}")) => Response::json(
                    422,
                    json!({"error": "path_mismatch", "message": "start differs"}),
                ),
                ("PUT", p) if p.ends_with(&format!("/{}", T0 + 1000)) => Response {
                    status: 429,
                    headers: vec![("Retry-After", "120".into())],
                    body: json!({"error": "rate_limited"}).to_string(),
                },
                ("PUT", _) => Response::empty(500),
                _ => Response::empty(404),
            }
        }))
        .await;
        let r = rig(&srv.base, MemorySecret::default(), no_browser());
        r.secret.set("ref").unwrap();
        let a = ended_session(&r.store, ME, T0);
        let b = ended_session(&r.store, ME, T0 + 1000);
        let c = ended_session(&r.store, ME, T0 + 2000);
        for id in [a, b, c] {
            r.worker.session_finished(id, ME, T0 + 3000);
        }
        r.worker.step(T0 + 3000).await;
        let ea = r.store.sync_entry(a).unwrap().unwrap();
        assert_eq!(ea.state, SyncState::Failed);
        assert!(ea.last_error.unwrap().contains("path_mismatch"));
        let eb = r.store.sync_entry(b).unwrap().unwrap();
        assert_eq!(
            (eb.state, eb.next_attempt_at),
            (SyncState::Queued, T0 + 3120)
        );
        // the rate limit stopped the pass: c was not tried
        let ec = r.store.sync_entry(c).unwrap().unwrap();
        assert_eq!((ec.attempts, ec.next_attempt_at), (0, T0 + 3000));
        // a failed entry is not retried by itself, only by "Retry"
        r.worker.step(T0 + 5000).await;
        assert_eq!(
            r.store.sync_entry(a).unwrap().unwrap().state,
            SyncState::Failed
        );
        let ec = r.store.sync_entry(c).unwrap().unwrap();
        assert_eq!(
            (ec.state, ec.attempts),
            (SyncState::Queued, 1),
            "500 = retry later"
        );
        r.worker.upload_now(a, T0 + 5000).unwrap();
        assert_eq!(
            r.store.sync_entry(a).unwrap().unwrap().state,
            SyncState::Queued
        );
        // a squad session cannot be queued by hand either
        let s = ended_session(&r.store, SQUAD, T0);
        assert!(r.worker.upload_now(s, T0).is_err());
    }

    #[tokio::test]
    async fn revoked_sign_in_and_logout() {
        let srv = start(Arc::new(|r: &Request| match r.path.as_str() {
            "/auth/refresh" => Response::json(401, json!({"error": "token_reused"})),
            "/auth/logout" => Response::empty(204),
            _ => Response::empty(404),
        }))
        .await;
        let r = rig(&srv.base, MemorySecret::default(), no_browser());
        r.secret.set("ref-old").unwrap();
        let id = ended_session(&r.store, ME, T0);
        r.worker.session_finished(id, ME, T0);
        assert_eq!(r.worker.step(T0).await, 0);
        let st = r.worker.status();
        assert!(!st.signed_in);
        assert!(st.last_error.unwrap().contains("sign in again"));
        assert_eq!(r.secret.get().unwrap(), None);
        assert_eq!(
            r.store.sync_entry(id).unwrap().unwrap().state,
            SyncState::Queued
        );
        // logout revokes on the server and clears the keyring
        r.secret.set("ref-x").unwrap();
        r.worker.logout().await.unwrap();
        assert_eq!(r.secret.get().unwrap(), None);
        let out = srv
            .requests()
            .into_iter()
            .rfind(|q| q.path == "/auth/logout")
            .unwrap();
        assert_eq!(out.body, r#"{"refresh_token":"ref-x"}"#);
    }

    #[tokio::test]
    async fn membership_lost_keeps_the_queue() {
        let srv = start(Arc::new(|r: &Request| match r.path.as_str() {
            "/auth/refresh" => {
                let mut a = auth("acc", "ref");
                a["account"]["upload_allowed"] = json!(false);
                Response::json(200, a)
            }
            _ => Response::empty(500),
        }))
        .await;
        let r = rig(&srv.base, MemorySecret::default(), no_browser());
        r.secret.set("ref").unwrap();
        let id = ended_session(&r.store, ME, T0);
        r.worker.session_finished(id, ME, T0);
        assert_eq!(r.worker.step(T0).await, 0);
        assert!(!srv.requests().iter().any(|q| q.method == "PUT"));
        let e = r.store.sync_entry(id).unwrap().unwrap();
        assert_eq!((e.state, e.attempts), (SyncState::Queued, 0));
        assert!(!r.worker.status().account.unwrap().upload_allowed);
    }

    /// The app spawns these futures on its multi-threaded runtime.
    #[test]
    fn futures_are_send() {
        fn send<T: Send + 'static>(_: T) {}
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let r = rig(
                "https://example.invalid/api/v1",
                MemorySecret::default(),
                no_browser(),
            );
            send(r.worker.clone().run(t0));
            let w = r.worker.clone();
            send(async move { w.login(t0).await });
            let w = r.worker.clone();
            send(async move { w.logout().await });
            let w = r.worker.clone();
            send(async move { w.delete_account(T0).await });
        });
    }

    /// The keyring is not touched before it is needed.
    #[test]
    fn keyring_is_contacted_lazily() {
        // creating the entry must not connect (it would block and could
        // prompt while the sync is off)
        let _ = crate::online_sync::secret::KeyringSecret::new("test");
    }

    #[tokio::test]
    async fn cancel_a_running_sign_in() {
        let srv = start(Arc::new(|_: &Request| Response::empty(500))).await;
        // a browser that never comes back
        let r = rig(&srv.base, MemorySecret::default(), Arc::new(|_| Ok(())));
        let w = r.worker.clone();
        let task = tokio::spawn(async move { w.login(t0).await });
        for _ in 0..100 {
            if r.worker.status().login_pending {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // the listener may still be starting: cancel until the task ends
        while !task.is_finished() {
            r.worker.cancel_login();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let e = task.await.unwrap().unwrap_err();
        assert!(e.contains("cancelled"), "{e}");
        assert!(!r.worker.status().login_pending);
        assert!(srv.requests().is_empty());
    }
}
