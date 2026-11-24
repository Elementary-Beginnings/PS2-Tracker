//! HTTP calls to the stats server. Its own `reqwest` client (not the Census
//! client and not its rate limiter), so uploads never delay Census requests.
//!
//! Tokens are wrapped in [`Token`], whose `Debug` shows nothing; error texts
//! carry the status, the server's error code and a shortened message, never a
//! token or a request body.

use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

/// A bearer or refresh token. `Debug`/`Display` never show the value.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    pub fn new(v: impl Into<String>) -> Self {
        Self(v.into())
    }
    /// The value for a request or the keyring. Do not log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(***)")
    }
}

/// A character the server allows this account to upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedCharacter {
    #[serde(with = "crate::events::id_string")]
    pub character_id: u64,
    #[serde(default)]
    pub name: String,
}

/// Stored data of the account (`storage` of `GET /me`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Storage {
    #[serde(default)]
    pub sessions: u64,
    #[serde(default)]
    pub bytes: u64,
}

/// The signed-in account (`GET /me`, also part of every sign-in/refresh).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub discord_id: String,
    /// ELME characters linked at the last sign-in or refresh; only their
    /// sessions can be uploaded.
    #[serde(default)]
    pub characters: Vec<AllowedCharacter>,
    /// False once no ELME character is linked any more (uploads closed;
    /// reading and deleting keep working until `purge_at`).
    #[serde(default = "yes")]
    pub upload_allowed: bool,
    #[serde(default)]
    pub membership_lost_at: Option<i64>,
    #[serde(default)]
    pub purge_at: Option<i64>,
    #[serde(default)]
    pub storage: Storage,
}

fn yes() -> bool {
    true
}

impl Account {
    pub fn allows(&self, character_id: u64) -> bool {
        self.characters
            .iter()
            .any(|c| c.character_id == character_id)
    }
}

/// Answer of `POST /auth/discord` and `POST /auth/refresh`.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub access_token: Token,
    /// Lifetime of the access token in seconds (15 minutes).
    pub access_expires_in: i64,
    /// The new refresh token (single use: the old one is dead now).
    pub refresh_token: Token,
    pub account: Account,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("stats server not reachable: {0}")]
    Network(String),
    #[error("{}", status_text(*.status, .code.as_deref(), .message.as_deref()))]
    Status {
        status: u16,
        /// Machine-readable `error` of the body.
        code: Option<String>,
        message: Option<String>,
        /// `Retry-After` in seconds (429/503).
        retry_after: Option<i64>,
    },
    #[error("unexpected answer from the stats server: {0}")]
    Shape(String),
}

fn status_text(status: u16, code: Option<&str>, message: Option<&str>) -> String {
    let mut s = format!("HTTP {status}");
    if let Some(c) = code {
        s.push_str(&format!(" ({c})"));
    }
    if let Some(m) = message {
        s.push_str(&format!(": {m}"));
    }
    s
}

impl ApiError {
    pub fn status(&self) -> Option<u16> {
        match self {
            ApiError::Status { status, .. } => Some(*status),
            _ => None,
        }
    }
}

/// How the worker treats a failed call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// 401: refresh the access token (once), then try again.
    Unauthorized,
    /// Repeating does not help: 403, 413, 422 and other 4xx.
    Permanent,
    /// Worth repeating later: network, 408, 429, 5xx.
    Transient,
}

pub fn classify(e: &ApiError) -> ErrorClass {
    match e {
        ApiError::Network(_) => ErrorClass::Transient,
        ApiError::Shape(_) => ErrorClass::Transient,
        ApiError::Status { status, .. } => match *status {
            401 => ErrorClass::Unauthorized,
            408 | 425 | 429 => ErrorClass::Transient,
            s if s >= 500 => ErrorClass::Transient,
            _ => ErrorClass::Permanent,
        },
    }
}

#[derive(Serialize)]
struct ExchangeBody<'a> {
    code: &'a str,
    code_verifier: &'a str,
    redirect_uri: &'a str,
    /// Shown to the user as "signed in from".
    client: &'a str,
}

/// `client` of the sign-in and the `generator` of the session files.
pub const CLIENT: &str = concat!("ps2-tracker ", env!("CARGO_PKG_VERSION"));

#[derive(Serialize)]
struct RefreshBody<'a> {
    refresh_token: &'a str,
}

#[derive(Deserialize, Default)]
struct ErrorBody {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Client for one API base (e.g. `https://stats.elme.games/api/v1`).
#[derive(Clone)]
pub struct SyncApi {
    http: reqwest::Client,
    base: String,
}

impl SyncApi {
    pub fn new(base: &str) -> Result<Self, ApiError> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("ps2-tracker/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(15))
            // a redirect would resend the bearer token to wherever it points
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// `POST /auth/discord`: authorization code → tokens and allowed
    /// characters. 403 = the Discord account is not linked to an ELME
    /// character.
    pub async fn exchange(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<AuthResponse, ApiError> {
        let body = ExchangeBody {
            code,
            code_verifier,
            redirect_uri,
            client: CLIENT,
        };
        let r = self
            .send(self.http.post(self.url("/auth/discord")).json(&body))
            .await?;
        parse_json(r).await
    }

    /// `POST /auth/refresh`: refresh token → new tokens (rotating).
    pub async fn refresh(&self, refresh_token: &Token) -> Result<AuthResponse, ApiError> {
        let body = RefreshBody {
            refresh_token: refresh_token.expose(),
        };
        let r = self
            .send(self.http.post(self.url("/auth/refresh")).json(&body))
            .await?;
        parse_json(r).await
    }

    /// `POST /auth/logout`: invalidates the refresh token on the server.
    pub async fn logout(&self, refresh_token: &Token) -> Result<(), ApiError> {
        let body = RefreshBody {
            refresh_token: refresh_token.expose(),
        };
        self.send(self.http.post(self.url("/auth/logout")).json(&body))
            .await?;
        Ok(())
    }

    /// `GET /me`: the account as the server sees it now.
    pub async fn me(&self, access: &Token) -> Result<Account, ApiError> {
        let r = self
            .send(self.http.get(self.url("/me")).bearer_auth(access.expose()))
            .await?;
        parse_json(r).await
    }

    /// `DELETE /me`: deletes the account, all its sessions and tokens on the
    /// server.
    pub async fn delete_account(&self, access: &Token) -> Result<(), ApiError> {
        self.send(
            self.http
                .delete(self.url("/me"))
                .bearer_auth(access.expose()),
        )
        .await?;
        Ok(())
    }

    /// `PUT /sessions/{character_id}/{started_at}` with the session file as
    /// body. Idempotent: the same key replaces the server's copy. Returns
    /// true for a new session (201), false for a replaced one (200).
    pub async fn upload(
        &self,
        access: &Token,
        character_id: u64,
        started_at: i64,
        body: String,
    ) -> Result<bool, ApiError> {
        let req = self
            .http
            .put(self.url(&format!("/sessions/{character_id}/{started_at}")))
            .bearer_auth(access.expose())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);
        let r = self.send(req).await?;
        Ok(r.status() == StatusCode::CREATED)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response, ApiError> {
        // without_url: the URL holds no secret, but keep errors short
        let r = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.without_url().to_string()))?;
        let status = r.status();
        if status.is_success() {
            return Ok(r);
        }
        Err(error_from(status, r).await)
    }
}

async fn error_from(status: StatusCode, r: reqwest::Response) -> ApiError {
    let retry_after = r
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|s| *s >= 0);
    // only the start of an error body is read; the texts are cut anyway
    let text = match read_limited(r, MAX_ERROR_BYTES).await {
        Ok(b) | Err(Limited::TooLarge(b)) => b,
        Err(Limited::Network(_)) => Vec::new(),
    };
    let body: ErrorBody = serde_json::from_slice(&text).unwrap_or_default();
    ApiError::Status {
        status: status.as_u16(),
        code: body.error.map(|c| clean(&c, 64)),
        message: body.message.map(|m| clean(&m, 200)),
        retry_after,
    }
}

/// Printable characters only, at most `max` of them.
fn clean(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Largest JSON answer read from the stats server.
pub const MAX_JSON_BYTES: usize = 1024 * 1024;
/// How much of an error body is read.
pub const MAX_ERROR_BYTES: usize = 4 * 1024;

enum Limited {
    /// More than the limit: the first `limit` bytes.
    TooLarge(Vec<u8>),
    Network(String),
}

/// Reads a body chunk by chunk, at most `limit` bytes.
async fn read_limited(mut r: reqwest::Response, limit: usize) -> Result<Vec<u8>, Limited> {
    if r.content_length().is_some_and(|n| n > limit as u64) {
        return Err(Limited::TooLarge(Vec::new()));
    }
    let mut out = Vec::new();
    while let Some(chunk) = r
        .chunk()
        .await
        .map_err(|e| Limited::Network(e.without_url().to_string()))?
    {
        let room = limit - out.len();
        if chunk.len() > room {
            out.extend_from_slice(&chunk[..room]);
            return Err(Limited::TooLarge(out));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

async fn parse_json<T: serde::de::DeserializeOwned>(r: reqwest::Response) -> Result<T, ApiError> {
    let bytes = read_limited(r, MAX_JSON_BYTES).await.map_err(|e| match e {
        Limited::TooLarge(_) => {
            ApiError::Shape(format!("answer larger than {} KiB", MAX_JSON_BYTES / 1024))
        }
        Limited::Network(e) => ApiError::Network(e),
    })?;
    // the serde error names the field, never the value of a token
    serde_json::from_slice(&bytes).map_err(|e| ApiError::Shape(format!("{e:#}")))
}

/// A minimal HTTP/1.1 server for tests: records requests, answers from a
/// closure.
#[cfg(test)]
pub(crate) mod mock {
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[derive(Debug, Clone)]
    pub struct Request {
        pub method: String,
        pub path: String,
        pub headers: Vec<(String, String)>,
        pub body: String,
    }

    impl Request {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }
    }

    pub struct Response {
        pub status: u16,
        pub headers: Vec<(&'static str, String)>,
        pub body: String,
    }

    impl Response {
        pub fn json(status: u16, body: serde_json::Value) -> Self {
            Self {
                status,
                headers: vec![],
                body: body.to_string(),
            }
        }
        pub fn empty(status: u16) -> Self {
            Self {
                status,
                headers: vec![],
                body: String::new(),
            }
        }
    }

    pub type Handler = Arc<dyn Fn(&Request) -> Response + Send + Sync>;

    pub struct MockServer {
        pub base: String,
        pub requests: Arc<Mutex<Vec<Request>>>,
    }

    impl MockServer {
        pub fn requests(&self) -> Vec<Request> {
            self.requests.lock().unwrap().clone()
        }
    }

    pub async fn start(handler: Handler) -> MockServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/api/v1", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                let handler = handler.clone();
                let log = log.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let head_end = loop {
                        let n = s.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break p + 4;
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
                    let mut lines = head.split("\r\n");
                    let mut first = lines.next().unwrap_or("").split(' ');
                    let method = first.next().unwrap_or("").to_owned();
                    let full = first.next().unwrap_or("").to_owned();
                    let path = full.strip_prefix("/api/v1").unwrap_or(&full).to_owned();
                    let headers: Vec<(String, String)> = lines
                        .filter_map(|l| l.split_once(':'))
                        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                        .collect();
                    let len: usize = headers
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .and_then(|(_, v)| v.parse().ok())
                        .unwrap_or(0);
                    while buf.len() < head_end + len {
                        let n = s.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    let body =
                        String::from_utf8_lossy(&buf[head_end..(head_end + len).min(buf.len())])
                            .into_owned();
                    let req = Request {
                        method,
                        path,
                        headers,
                        body,
                    };
                    let resp = handler(&req);
                    log.lock().unwrap().push(req);
                    let mut out = format!(
                        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n",
                        resp.status,
                        resp.body.len()
                    );
                    for (k, v) in resp.headers {
                        out.push_str(&format!("{k}: {v}\r\n"));
                    }
                    out.push_str("\r\n");
                    out.push_str(&resp.body);
                    let _ = s.write_all(out.as_bytes()).await;
                    let _ = s.shutdown().await;
                });
            }
        });
        MockServer { base, requests }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::mock::{Response, start};
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    /// Shape as in the contract (documentation/online-sync-api.md).
    pub(crate) fn auth_json(access: &str, refresh: &str) -> serde_json::Value {
        json!({
            "token_type": "Bearer",
            "access_token": access, "access_expires_in": 900,
            "refresh_token": refresh, "refresh_expires_in": 2592000,
            "account": {
                "discord_id": crate::online_sync::test_snowflake('9'),
                "characters": [{"character_id": "5428000000000000001", "name": "Me"}],
                "upload_allowed": true, "membership_lost_at": null, "purge_at": null,
                "created_at": 1795300000, "storage": {"sessions": 42, "bytes": 1834221}
            }
        })
    }

    #[tokio::test]
    async fn exchange_refresh_logout_upload() {
        let srv = start(Arc::new(|r| match (r.method.as_str(), r.path.as_str()) {
            ("POST", "/auth/discord") | ("POST", "/auth/refresh") => {
                Response::json(200, auth_json("acc-1", "ref-2"))
            }
            ("POST", "/auth/logout") => Response::empty(204),
            ("PUT", _) => Response::json(201, json!({"id": "x"})),
            ("GET", "/me") => Response::json(200, auth_json("", "")["account"].clone()),
            ("DELETE", "/me") => Response::empty(204),
            _ => Response::empty(404),
        }))
        .await;
        let api = SyncApi::new(&srv.base).unwrap();
        let a = api
            .exchange("c0de", "verif", "http://127.0.0.1:1/callback")
            .await
            .unwrap();
        assert_eq!(a.access_token.expose(), "acc-1");
        assert_eq!(a.access_expires_in, 900);
        assert_eq!(a.account.characters[0].character_id, 5428000000000000001);
        assert!(a.account.allows(5428000000000000001));
        assert!(!a.account.allows(1));
        assert!(a.account.upload_allowed);
        assert_eq!(a.account.storage.sessions, 42);
        assert!(!format!("{a:?}").contains("acc-1"), "tokens in Debug");
        api.refresh(&Token::new("ref-1")).await.unwrap();
        api.logout(&Token::new("ref-2")).await.unwrap();
        assert!(
            api.upload(
                &Token::new("acc-1"),
                5428000000000000001,
                1700000000,
                "{}".into()
            )
            .await
            .unwrap(),
            "201 = new"
        );
        let me = api.me(&Token::new("acc-1")).await.unwrap();
        assert_eq!(me.discord_id, crate::online_sync::test_snowflake('9'));
        api.delete_account(&Token::new("acc-1")).await.unwrap();
        let reqs = srv.requests();
        let ex: serde_json::Value = serde_json::from_str(&reqs[0].body).unwrap();
        assert_eq!(
            ex,
            json!({"code": "c0de", "code_verifier": "verif",
                   "redirect_uri": "http://127.0.0.1:1/callback", "client": CLIENT})
        );
        assert_eq!(reqs[5].method, "DELETE");
        assert_eq!(reqs[5].header("authorization"), Some("Bearer acc-1"));
        assert_eq!(reqs[1].body, r#"{"refresh_token":"ref-1"}"#);
        let up = &reqs[3];
        assert_eq!(up.path, "/sessions/5428000000000000001/1700000000");
        assert_eq!(up.header("authorization"), Some("Bearer acc-1"));
        assert_eq!(up.header("content-type"), Some("application/json"));
        // nothing here sends the client secret: there is none
        assert!(reqs.iter().all(|r| !r.body.contains("client_secret")));
    }

    #[tokio::test]
    async fn errors_are_classified_without_secrets() {
        let srv = start(Arc::new(|r| match r.path.as_str() {
            "/auth/discord" => Response::json(
                403,
                json!({"error": "not_linked", "message": "no ELME character\u{7}"}),
            ),
            "/auth/refresh" => Response::json(200, json!({"access_token": "only"})),
            p if p.ends_with("/1") => Response {
                status: 429,
                headers: vec![("Retry-After", "17".into())],
                body: String::new(),
            },
            p if p.ends_with("/2") => Response::empty(413),
            p if p.ends_with("/3") => Response::json(401, json!({"error": "token_expired"})),
            _ => Response::empty(502),
        }))
        .await;
        let api = SyncApi::new(&srv.base).unwrap();
        let e = api.exchange("c", "v", "r").await.unwrap_err();
        assert_eq!(e.status(), Some(403));
        assert_eq!(classify(&e), ErrorClass::Permanent);
        assert_eq!(e.to_string(), "HTTP 403 (not_linked): no ELME character");
        let e = api
            .refresh(&Token::new("secret-refresh"))
            .await
            .unwrap_err();
        assert!(matches!(e, ApiError::Shape(_)));
        let t = Token::new("secret-access");
        let e = api.upload(&t, 1, 1, "{}".into()).await.unwrap_err();
        assert!(matches!(
            e,
            ApiError::Status {
                status: 429,
                retry_after: Some(17),
                ..
            }
        ));
        assert_eq!(classify(&e), ErrorClass::Transient);
        let e = api.upload(&t, 1, 2, "{}".into()).await.unwrap_err();
        assert_eq!(classify(&e), ErrorClass::Permanent);
        let e = api.upload(&t, 1, 3, "{}".into()).await.unwrap_err();
        assert_eq!(classify(&e), ErrorClass::Unauthorized);
        let e = api.upload(&t, 1, 4, "{}".into()).await.unwrap_err();
        assert_eq!(classify(&e), ErrorClass::Transient);
        for e in [
            api.upload(&t, 1, 3, "{}".into()).await.unwrap_err(),
            api.refresh(&Token::new("secret-refresh"))
                .await
                .unwrap_err(),
        ] {
            let s = format!("{e} {e:?}");
            assert!(!s.contains("secret-"), "{s}");
        }
        // unreachable server
        let dead = SyncApi::new("http://127.0.0.1:9/api/v1").unwrap();
        let e = dead.upload(&t, 1, 1, "{}".into()).await.unwrap_err();
        assert!(matches!(e, ApiError::Network(_)));
        assert_eq!(classify(&e), ErrorClass::Transient);
    }

    #[tokio::test]
    async fn answers_are_read_up_to_a_limit() {
        let big = "x".repeat(MAX_JSON_BYTES + 1);
        let srv = start(Arc::new(move |r| match r.path.as_str() {
            "/me" => Response {
                status: 200,
                headers: vec![],
                body: format!("{{\"discord_id\":\"{big}\"}}"),
            },
            _ => Response::json(400, json!({"error": "bad", "message": big.clone()})),
        }))
        .await;
        let api = SyncApi::new(&srv.base).unwrap();
        let e = api.me(&Token::new("a")).await.unwrap_err();
        assert!(matches!(e, ApiError::Shape(_)), "{e:?}");
        assert!(e.to_string().contains("larger than"), "{e}");
        // an oversized error body still gives the status
        let e = api.logout(&Token::new("r")).await.unwrap_err();
        assert_eq!(e.status(), Some(400));

        // without Content-Length: the stream is cut at the limit
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf).await;
            let _ = s
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n")
                .await;
            let chunk = vec![b' '; 64 * 1024];
            for _ in 0..64 {
                if s.write_all(&chunk).await.is_err() {
                    return;
                }
            }
        });
        let api = SyncApi::new(&format!("http://{addr}/api/v1")).unwrap();
        let e = api.me(&Token::new("a")).await.unwrap_err();
        assert!(matches!(e, ApiError::Shape(_)), "{e:?}");
    }
}
