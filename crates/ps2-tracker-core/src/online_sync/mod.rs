//! Online sync: uploads finished sessions of ELME members to the stats
//! server (stats.elme.games). Opt-in and off by default; without switching it
//! on and signing in the tracker makes no request to that server.
//!
//! - [`pkce`], [`loopback`]: Discord sign-in (authorization code + PKCE,
//!   loopback redirect on 127.0.0.1).
//! - [`api`]: the HTTP calls to the stats server (own client, independent of
//!   the Census rate limiter).
//! - [`secret`]: the refresh token, only in the operating system's keyring.
//! - [`queue`]: states of the persistent upload queue (rows in SQLite).
//! - [`worker`]: sign-in, token refresh and the background uploads.

pub mod api;
pub mod loopback;
pub mod persist;
pub mod pkce;
pub mod queue;
pub mod secret;
pub mod worker;

use serde::{Deserialize, Serialize};

/// API of the ELME stats server.
pub const DEFAULT_API_BASE: &str = "https://stats.elme.games/api/v1";

/// Loopback ports of the sign-in, tried in this order. Each
/// `http://127.0.0.1:<port>/callback` is registered with Discord and accepted
/// by the server (contract: documentation/online-sync-api.md of the server).
pub const REDIRECT_PORTS: [u16; 3] = [47_821, 47_822, 47_823];

/// Discord's authorization endpoint.
pub const DISCORD_AUTHORIZE: &str = "https://discord.com/oauth2/authorize";

/// How long the tracker waits for the browser to come back.
pub const LOGIN_TIMEOUT_SECS: u64 = 300;

/// Discord application of the stats server in official builds (a public
/// OAuth client ID, not a secret); unset or empty in other builds.
const BUILT_IN_CLIENT_ID: Option<&str> = option_env!("PS2_SYNC_DISCORD_CLIENT_ID");

/// Discord client IDs are snowflakes: 17–20 digits.
pub fn is_client_id(id: &str) -> bool {
    crate::presence::is_app_id(id)
}

/// The built-in client ID, if this build has a valid one.
pub fn built_in_client_id() -> Option<&'static str> {
    BUILT_IN_CLIENT_ID.filter(|id| is_client_id(id))
}

/// Settings → Online sync (`[online_sync]` in config.toml). Holds no tokens:
/// the refresh token lives in the keyring, the access token only in memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncSettings {
    /// Upload finished sessions (the user agreed to the upload).
    pub enabled: bool,
    /// Base URL of the API, https only (http://localhost in debug builds).
    pub api_base: String,
    /// Discord client ID of the server's application; empty = built-in.
    pub discord_client_id: String,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            api_base: DEFAULT_API_BASE.into(),
            discord_client_id: String::new(),
        }
    }
}

impl SyncSettings {
    pub fn validate(&self) -> Result<(), String> {
        api_base(&self.api_base, cfg!(debug_assertions))?;
        if !self.discord_client_id.is_empty() && !is_client_id(&self.discord_client_id) {
            return Err("Discord client ID must be 17–20 digits".into());
        }
        Ok(())
    }

    /// Own client ID, otherwise the built-in one.
    pub fn effective_client_id(&self) -> Option<String> {
        if is_client_id(&self.discord_client_id) {
            Some(self.discord_client_id.clone())
        } else if self.discord_client_id.is_empty() {
            built_in_client_id().map(str::to_owned)
        } else {
            None
        }
    }

    /// The API base without a trailing slash (empty = default).
    pub fn base(&self) -> String {
        let b = self.api_base.trim().trim_end_matches('/');
        if b.is_empty() {
            DEFAULT_API_BASE.into()
        } else {
            b.into()
        }
    }
}

/// Checks an API base URL: https, a host, no credentials, query or
/// fragment. Plain http only for localhost/127.0.0.1/[::1] and only when
/// `allow_local_http` (debug builds). Empty means the default.
pub fn api_base(url: &str, allow_local_http: bool) -> Result<reqwest::Url, String> {
    let url = url.trim();
    let url = if url.is_empty() {
        DEFAULT_API_BASE
    } else {
        url
    };
    let u = reqwest::Url::parse(url).map_err(|_| "API address is not a valid URL".to_string())?;
    let local = matches!(
        u.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("[::1]")
    );
    match u.scheme() {
        "https" => {}
        "http" if local && allow_local_http => {}
        _ => return Err("API address must start with https://".into()),
    }
    if u.host_str().is_none_or(str::is_empty)
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("API address must be a plain https:// address".into());
    }
    Ok(u)
}

/// The Discord authorization URL of one sign-in (scope `identify`, PKCE S256,
/// `prompt=consent` as the contract asks).
pub fn authorize_url(client_id: &str, redirect_uri: &str, challenge: &str, state: &str) -> String {
    let mut u = reqwest::Url::parse(DISCORD_AUTHORIZE).expect("constant URL");
    u.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("scope", "identify")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("prompt", "consent");
    u.into()
}

/// Privacy policy of the server: `/privacy` on the host of the API.
pub fn privacy_url(base: &str) -> Option<String> {
    let u = reqwest::Url::parse(base).ok()?;
    let mut p = u.clone();
    p.set_path("/privacy");
    p.set_query(None);
    Some(p.into())
}

/// Keyring account of an API base (one sign-in per server).
pub fn keyring_account(base: &str) -> String {
    format!("refresh-token@{}", base.trim_end_matches('/'))
}

/// A made-up Discord snowflake for tests, built at runtime (no 18-digit
/// literal in the source that secret scanners take for a real ID).
#[cfg(test)]
pub(crate) fn test_snowflake(digit: char) -> String {
    std::iter::repeat_n(digit, 18).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_base_rules() {
        assert!(api_base(DEFAULT_API_BASE, false).is_ok());
        assert!(api_base("", false).is_ok());
        assert!(api_base("http://stats.elme.games/api/v1", true).is_err());
        assert!(api_base("http://localhost:8080/api/v1", false).is_err());
        assert!(api_base("http://localhost:8080/api/v1", true).is_ok());
        assert!(api_base("http://127.0.0.1:8080", true).is_ok());
        assert!(api_base("http://192.168.1.2:8080", true).is_err());
        assert!(api_base("https://user:pw@stats.elme.games", false).is_err());
        assert!(api_base("https://stats.elme.games/?x=1", false).is_err());
        assert!(api_base("ftp://stats.elme.games", false).is_err());
        assert!(api_base("not a url", false).is_err());
    }

    #[test]
    fn authorize_url_has_every_parameter() {
        let u = reqwest::Url::parse(&authorize_url(
            &test_snowflake('7'),
            "http://127.0.0.1:47821/callback",
            "chal",
            "st",
        ))
        .unwrap();
        assert_eq!(u.host_str(), Some("discord.com"));
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["client_id"], test_snowflake('7'));
        assert_eq!(q["scope"], "identify");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:47821/callback");
        assert_eq!(q["code_challenge"], "chal");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "st");
        assert_eq!(q["prompt"], "consent");
        assert!(
            u.as_str()
                .contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A47821%2Fcallback")
        );
        assert_eq!(
            privacy_url(DEFAULT_API_BASE).as_deref(),
            Some("https://stats.elme.games/privacy")
        );
    }

    #[test]
    fn settings_defaults_and_client_id() {
        let s = SyncSettings::default();
        assert!(!s.enabled, "online sync is off by default");
        assert_eq!(s.base(), DEFAULT_API_BASE);
        assert!(s.validate().is_ok());
        let own = SyncSettings {
            discord_client_id: test_snowflake('7'),
            api_base: "https://example.org/api/".into(),
            ..s.clone()
        };
        assert_eq!(
            own.effective_client_id().as_deref(),
            Some(test_snowflake('7').as_str())
        );
        assert_eq!(own.base(), "https://example.org/api");
        let bad = SyncSettings {
            discord_client_id: "12ab".into(),
            ..s
        };
        assert!(bad.validate().is_err());
        assert_eq!(bad.effective_client_id(), None);
        assert_ne!(
            keyring_account("https://a.example/api/v1/"),
            keyring_account("https://b.example/api/v1")
        );
    }
}
