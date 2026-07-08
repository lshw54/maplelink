//! Per-session state for multi-session support.
//!
//! Each login session gets its own HTTP client, cookie jar, beanfun session,
//! game accounts, and serialization lock. This allows multiple accounts
//! (potentially across different regions) to be logged in simultaneously.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT_ENCODING, ACCEPT_LANGUAGE, USER_AGENT,
};
use tokio::sync::{Mutex, RwLock};

use super::game_account::GameAccount;
use super::session::Session;

/// Unique identifier for a login session.
pub type SessionId = String;

/// In-progress TW Regular login, carried between the two reCAPTCHA phases.
///
/// Phase 1 (`tw_login_check`) bootstraps the session key + form token and
/// passes the first reCAPTCHA; phase 2 (`tw_login_submit`) reuses them to
/// submit the password with the second reCAPTCHA.
#[derive(Debug, Clone)]
pub struct PendingTwLogin {
    pub skey: String,
    pub form_token: String,
    pub account: String,
}

/// State for a single login session.
pub struct SessionState {
    /// Beanfun session (token, region, account name, etc.)
    pub session: RwLock<Option<Session>>,
    /// Game accounts associated with this session.
    pub game_accounts: RwLock<Vec<GameAccount>>,
    /// HTTP client with its own cookie jar for this session.
    pub http_client: reqwest::Client,
    /// Cookie jar for reading cookies (e.g. bfWebToken).
    pub cookie_jar: Arc<reqwest::cookie::Jar>,
    /// Serializes beanfun HTTP operations for this session.
    pub bf_client_lock: Mutex<()>,
    /// Maps PID → account ID for game processes launched from this session.
    pub active_processes: RwLock<HashMap<u32, String>>,
    /// In-progress two-phase TW login state (set by `tw_login_check`).
    pub pending_tw_login: RwLock<Option<PendingTwLogin>>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    /// Create a new session state with a fresh HTTP client and cookie jar.
    pub fn new() -> Self {
        let cookie_jar = Arc::new(reqwest::cookie::Jar::default());
        // Present as a current Chrome browser (UA + client hints) on every
        // request. These are constant for a browser so they're safe as
        // client-wide defaults; the TW login POSTs add per-request Sec-Fetch-*
        // via `with_browser_xhr_headers` in beanfun_service.
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
            ),
        );
        default_headers.insert(
            HeaderName::from_static("sec-ch-ua"),
            HeaderValue::from_static(
                "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"",
            ),
        );
        default_headers.insert(
            HeaderName::from_static("sec-ch-ua-mobile"),
            HeaderValue::from_static("?0"),
        );
        default_headers.insert(
            HeaderName::from_static("sec-ch-ua-platform"),
            HeaderValue::from_static("\"Windows\""),
        );
        default_headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-TW,zh;q=0.9,en-US;q=0.8,en;q=0.7"),
        );
        default_headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        let http_client = reqwest::Client::builder()
            .cookie_provider(cookie_jar.clone())
            .default_headers(default_headers)
            .http1_only()
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()
            .expect("failed to build HTTP client");

        Self {
            session: RwLock::new(None),
            game_accounts: RwLock::new(Vec::new()),
            http_client,
            cookie_jar,
            bf_client_lock: Mutex::new(()),
            active_processes: RwLock::new(HashMap::new()),
            pending_tw_login: RwLock::new(None),
        }
    }

    /// Clear all in-memory credentials and game account data.
    pub async fn clear_credentials(&self) {
        *self.session.write().await = None;
        self.game_accounts.write().await.clear();
        self.active_processes.write().await.clear();
        *self.pending_tw_login.write().await = None;
    }
}
