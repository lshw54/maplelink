//! Application-wide managed state shared across Tauri commands.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::config::AppConfig;
use super::game_account::GameAccount;
use super::session::Session;
use crate::services::account_storage::SavedAccount;

/// Tauri managed state holding all runtime data.
///
/// Each field is wrapped in a [`RwLock`] so commands can read/write
/// concurrently without blocking the async runtime.
pub struct AppState {
    pub session: RwLock<Option<Session>>,
    pub config: RwLock<AppConfig>,
    pub game_accounts: RwLock<Vec<GameAccount>>,
    /// Maps PID → account ID for running game processes.
    pub active_processes: RwLock<HashMap<u32, String>>,
    pub http_client: reqwest::Client,
    /// Shared cookie jar for reading cookies (e.g. bfWebToken).
    pub cookie_jar: Arc<reqwest::cookie::Jar>,
    /// Path to the INI config file on disk.
    pub config_path: PathBuf,
    /// Saved login accounts loaded from `accounts.json`.
    pub saved_accounts: RwLock<Vec<SavedAccount>>,
    /// Path to the `accounts.json` file on disk.
    pub accounts_path: PathBuf,
}

impl AppState {
    /// Clear all in-memory credentials and game account data.
    ///
    /// Called on logout and application exit to ensure no sensitive data
    /// lingers in memory.
    pub async fn clear_credentials(&self) {
        *self.session.write().await = None;
        self.game_accounts.write().await.clear();
        self.active_processes.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    use crate::models::session::Region;

    /// Strategy for generating a random [`Region`].
    fn arb_region() -> impl Strategy<Value = Region> {
        prop_oneof![Just(Region::TW), Just(Region::HK)]
    }

    /// Strategy for generating a random [`Session`].
    fn arb_session() -> impl Strategy<Value = Session> {
        (
            "[a-zA-Z0-9]{8,32}",                       // token
            proptest::option::of("[a-zA-Z0-9]{8,32}"), // refresh_token
            (0i64..=86400),                            // expires_in seconds from now
            arb_region(),
            "[a-zA-Z0-9_]{3,20}", // account_name
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

    /// Strategy for generating a random [`GameAccount`].
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
    //
    // For any AppState containing a valid Session, after invoking clear_credentials,
    // the session field shall be None and the game accounts list shall be empty.
    proptest! {
        #[test]
        fn prop_logout_clears_all_credentials(
            session in arb_session(),
            accounts in proptest::collection::vec(arb_game_account(), 0..10),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let state = AppState {
                    session: RwLock::new(Some(session)),
                    config: RwLock::new(AppConfig::default()),
                    game_accounts: RwLock::new(accounts),
                    active_processes: RwLock::new({
                        let mut m = HashMap::new();
                        m.insert(1234, "acc_1".to_string());
                        m
                    }),
                    http_client: reqwest::Client::new(),
                    cookie_jar: std::sync::Arc::new(reqwest::cookie::Jar::default()),
                    config_path: std::path::PathBuf::from("test.ini"),
                    saved_accounts: RwLock::new(Vec::new()),
                    accounts_path: std::path::PathBuf::from("accounts.json"),
                };

                // Pre-condition: session exists
                assert!(state.session.read().await.is_some());

                // Act
                state.clear_credentials().await;

                // Post-condition: everything cleared
                assert!(state.session.read().await.is_none());
                assert!(state.game_accounts.read().await.is_empty());
                assert!(state.active_processes.read().await.is_empty());
            });
        }
    }
}
