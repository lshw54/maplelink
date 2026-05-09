//! Per-session state for multi-session support.
//!
//! Each login session gets its own HTTP client, cookie jar, beanfun session,
//! game accounts, and serialization lock. This allows multiple accounts
//! (potentially across different regions) to be logged in simultaneously.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use super::game_account::GameAccount;
use super::session::Session;

/// Unique identifier for a login session.
pub type SessionId = String;

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
        let http_client = reqwest::Client::builder()
            .cookie_provider(cookie_jar.clone())
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
        }
    }

    /// Clear all in-memory credentials and game account data.
    pub async fn clear_credentials(&self) {
        *self.session.write().await = None;
        self.game_accounts.write().await.clear();
        self.active_processes.write().await.clear();
    }
}
