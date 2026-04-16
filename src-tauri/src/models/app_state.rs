//! Application-wide managed state shared across Tauri commands.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::config::AppConfig;
use super::session_state::{SessionId, SessionState};
use crate::services::account_storage::SavedAccount;

/// Tauri managed state holding all runtime data.
///
/// Supports multiple concurrent login sessions. Each session has its own
/// HTTP client, cookie jar, beanfun session, and game accounts.
pub struct AppState {
    /// All active login sessions, keyed by session ID.
    pub sessions: RwLock<HashMap<SessionId, Arc<SessionState>>>,
    /// Application configuration (shared across all sessions).
    pub config: RwLock<AppConfig>,
    /// Path to the INI config file on disk.
    pub config_path: PathBuf,
    /// Saved login accounts loaded from `accounts.json`.
    pub saved_accounts: RwLock<Vec<SavedAccount>>,
    /// Path to the `accounts.json` file on disk.
    pub accounts_path: PathBuf,
    /// A shared HTTP client for non-session operations (update checks, etc.)
    pub http_client: reqwest::Client,
}

impl AppState {
    /// Get a session by ID, or return an error if not found.
    pub async fn get_session(&self, session_id: &str) -> Option<Arc<SessionState>> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// Get a session by ID, or return an ErrorDto if not found.
    pub async fn require_session(
        &self,
        session_id: &str,
    ) -> Result<Arc<SessionState>, crate::models::error::ErrorDto> {
        self.get_session(session_id)
            .await
            .ok_or_else(|| crate::models::error::ErrorDto {
                code: "AUTH_SESSION_NOT_FOUND".to_string(),
                message: format!("session '{session_id}' not found"),
                category: crate::models::error::ErrorCategory::Authentication,
                details: None,
            })
    }

    /// Create a new session and return its ID.
    pub async fn create_session(&self) -> (SessionId, Arc<SessionState>) {
        let id = uuid::Uuid::new_v4().to_string();
        let session_state = Arc::new(SessionState::new());
        self.sessions
            .write()
            .await
            .insert(id.clone(), session_state.clone());
        (id, session_state)
    }

    /// Remove a session by ID and clear its credentials.
    pub async fn remove_session(&self, session_id: &str) {
        if let Some(ss) = self.sessions.write().await.remove(session_id) {
            ss.clear_credentials().await;
        }
    }

    /// Clear all sessions.
    pub async fn clear_all_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        for ss in sessions.values() {
            ss.clear_credentials().await;
        }
        sessions.clear();
    }

    /// Get all session IDs and their basic info (for frontend listing).
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        let mut result = Vec::new();
        for (id, ss) in sessions.iter() {
            let session = ss.session.read().await;
            if let Some(s) = session.as_ref() {
                result.push(SessionInfo {
                    id: id.clone(),
                    account_name: s.account_name.clone(),
                    region: format!("{:?}", s.region),
                });
            }
        }
        result
    }

    /// Check if any session has a running game process.
    pub async fn is_any_game_running(&self) -> bool {
        let sessions = self.sessions.read().await;
        for ss in sessions.values() {
            let active = ss.active_processes.read().await;
            for &pid in active.keys() {
                if pid > 0 && crate::services::process_service::is_process_running(pid) {
                    return true;
                }
            }
        }
        crate::services::process_service::is_process_name_running("MapleStory.exe")
    }

    /// Get the PID of any running game across all sessions.
    pub async fn get_any_game_pid(&self) -> u32 {
        let sessions = self.sessions.read().await;
        for ss in sessions.values() {
            let active = ss.active_processes.read().await;
            for &pid in active.keys() {
                if pid > 0 && crate::services::process_service::is_process_running(pid) {
                    return pid;
                }
            }
        }
        0
    }
}

/// Basic info about a session for frontend display.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub account_name: String,
    pub region: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    use crate::models::game_account::GameAccount;
    use crate::models::session::{Region, Session};

    fn arb_region() -> impl Strategy<Value = Region> {
        prop_oneof![Just(Region::TW), Just(Region::HK)]
    }

    fn arb_session() -> impl Strategy<Value = Session> {
        (
            "[a-zA-Z0-9]{8,32}",
            proptest::option::of("[a-zA-Z0-9]{8,32}"),
            (0i64..=86400),
            arb_region(),
            "[a-zA-Z0-9_]{3,20}",
        )
            .prop_map(
                |(token, refresh_token, expires_in, region, account_name)| Session {
                    token,
                    refresh_token,
                    expires_at: chrono::Utc::now() + chrono::Duration::seconds(expires_in),
                    region,
                    account_name,
                    session_key: None,
                    totp_state: None,
                },
            )
    }

    fn arb_game_account() -> impl Strategy<Value = GameAccount> {
        (
            "[a-zA-Z0-9]{4,16}",
            "[a-zA-Z0-9 ]{3,20}",
            "[a-zA-Z0-9]{2,10}",
            "[0-9]{6,12}",
            "active|inactive",
            "2020-01-01",
        )
            .prop_map(|(id, display_name, game_type, sn, status, created_at)| {
                GameAccount {
                    id,
                    display_name,
                    game_type,
                    sn,
                    status,
                    created_at,
                }
            })
    }

    // Feature: maplelink-rewrite, Property 4: Logout clears all in-memory credentials
    proptest! {
        #[test]
        fn prop_logout_clears_all_credentials(
            session in arb_session(),
            accounts in proptest::collection::vec(arb_game_account(), 0..10),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let ss = SessionState::new();
                *ss.session.write().await = Some(session);
                *ss.game_accounts.write().await = accounts;
                ss.active_processes.write().await.insert(1234, "acc_1".to_string());

                assert!(ss.session.read().await.is_some());

                ss.clear_credentials().await;

                assert!(ss.session.read().await.is_none());
                assert!(ss.game_accounts.read().await.is_empty());
                assert!(ss.active_processes.read().await.is_empty());
            });
        }
    }
}
