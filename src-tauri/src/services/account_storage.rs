//! Persistence layer for saved login accounts.
//!
//! Stores account credentials encrypted with Windows DPAPI (tied to current user).
//! The encrypted data is stored in `accounts.dat` with the entropy key in `accounts.key`.
//! Falls back to reading legacy plaintext `accounts.json` for migration.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::utils::dpapi;

/// A saved login account record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAccount {
    pub region: String,
    pub account: String,
    pub password: String,
    pub remember_password: bool,
}

/// Load saved accounts from encrypted storage.
///
/// Tries `accounts.dat` + `accounts.key` (DPAPI) first.
/// Falls back to legacy `accounts.json` (plaintext) and auto-migrates.
pub async fn load_accounts(path: &Path) -> Vec<SavedAccount> {
    let dat_path = path.with_extension("dat");
    let key_path = path.with_extension("key");

    // Try encrypted format first
    if dat_path.exists() && key_path.exists() {
        match load_encrypted(&dat_path, &key_path).await {
            Ok(accounts) => return accounts,
            Err(e) => {
                tracing::warn!("failed to load encrypted accounts: {e}");
            }
        }
    }

    // Fall back to legacy plaintext JSON
    if path.exists() {
        match tokio::fs::read_to_string(path).await {
            Ok(contents) => {
                let accounts: Vec<SavedAccount> =
                    serde_json::from_str(&contents).unwrap_or_else(|e| {
                        tracing::warn!("failed to parse legacy accounts file: {e}");
                        Vec::new()
                    });

                // Auto-migrate: save as encrypted and remove plaintext
                if !accounts.is_empty() {
                    if let Err(e) = save_accounts(path, &accounts).await {
                        tracing::warn!("failed to migrate accounts to encrypted format: {e}");
                    } else {
                        // Remove legacy plaintext file after successful migration
                        let _ = tokio::fs::remove_file(path).await;
                        tracing::info!(
                            "migrated accounts from plaintext to DPAPI encrypted format"
                        );
                    }
                }
                return accounts;
            }
            Err(e) => {
                tracing::warn!("failed to read legacy accounts file: {e}");
            }
        }
    }

    Vec::new()
}

/// Save accounts to DPAPI-encrypted storage.
pub async fn save_accounts(path: &Path, accounts: &[SavedAccount]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create directories: {e}"))?;
    }

    let json = serde_json::to_string_pretty(accounts)
        .map_err(|e| format!("failed to serialize accounts: {e}"))?;

    let (ciphertext, entropy) =
        dpapi::protect(json.as_bytes()).map_err(|e| format!("DPAPI encrypt failed: {e}"))?;

    let dat_path = path.with_extension("dat");
    let key_path = path.with_extension("key");

    tokio::fs::write(&dat_path, &ciphertext)
        .await
        .map_err(|e| format!("failed to write accounts.dat: {e}"))?;

    tokio::fs::write(&key_path, &entropy)
        .await
        .map_err(|e| format!("failed to write accounts.key: {e}"))?;

    Ok(())
}

/// Load and decrypt accounts from .dat + .key files.
async fn load_encrypted(dat_path: &Path, key_path: &Path) -> Result<Vec<SavedAccount>, String> {
    let ciphertext = tokio::fs::read(dat_path)
        .await
        .map_err(|e| format!("failed to read accounts.dat: {e}"))?;

    let entropy = tokio::fs::read(key_path)
        .await
        .map_err(|e| format!("failed to read accounts.key: {e}"))?;

    let plaintext = dpapi::unprotect(&ciphertext, &entropy)
        .map_err(|e| format!("DPAPI decrypt failed: {e}"))?;

    let json = String::from_utf8(plaintext)
        .map_err(|e| format!("decrypted data is not valid UTF-8: {e}"))?;

    serde_json::from_str(&json).map_err(|e| format!("failed to parse decrypted accounts: {e}"))
}

/// Add or update an account entry.
pub fn upsert_account(
    accounts: &mut Vec<SavedAccount>,
    region: &str,
    account: &str,
    password: &str,
    remember: bool,
) {
    // Remove existing entry first so we can re-add at the end.
    // This ensures get_last_account returns the most recently logged-in account.
    let existing = accounts
        .iter()
        .position(|a| a.region == region && a.account == account)
        .map(|idx| accounts.remove(idx));

    let pwd = if remember {
        password.to_string()
    } else if let Some(prev) = &existing {
        // Keep old password if user unchecked remember but had one saved
        if prev.remember_password {
            prev.password.clone()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    accounts.push(SavedAccount {
        region: region.to_string(),
        account: account.to_string(),
        password: pwd,
        remember_password: remember,
    });
}

/// Get all saved accounts for a specific region.
pub fn get_accounts_for_region<'a>(
    accounts: &'a [SavedAccount],
    region: &str,
) -> Vec<&'a SavedAccount> {
    accounts.iter().filter(|a| a.region == region).collect()
}

/// Get the last saved account for a region.
pub fn get_last_account<'a>(
    accounts: &'a [SavedAccount],
    region: &str,
) -> Option<&'a SavedAccount> {
    accounts.iter().rev().find(|a| a.region == region)
}

/// Get a specific saved account by region and account ID.
pub fn get_account<'a>(
    accounts: &'a [SavedAccount],
    region: &str,
    account: &str,
) -> Option<&'a SavedAccount> {
    accounts
        .iter()
        .find(|a| a.region == region && a.account == account)
}

/// Remove a saved account by region and account ID.
pub fn remove_account(accounts: &mut Vec<SavedAccount>, region: &str, account: &str) -> bool {
    let before = accounts.len();
    accounts.retain(|a| !(a.region == region && a.account == account));
    accounts.len() < before
}

// ---------------------------------------------------------------------------
// Display name overrides (local-only renames, DPAPI encrypted)
// ---------------------------------------------------------------------------

/// Local account customizations: display name overrides + sort order.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DisplayOverrides {
    /// account_id → custom display name
    #[serde(default)]
    pub names: std::collections::HashMap<String, String>,
    /// Custom account sort order (list of account IDs)
    #[serde(default)]
    pub order: Vec<String>,
}

/// Load display overrides from DPAPI-encrypted .dat + .key files.
pub async fn load_display_overrides(path: &Path) -> DisplayOverrides {
    let dat_path = path.with_extension("dat");
    let key_path = path.with_extension("key");

    if dat_path.exists() && key_path.exists() {
        let ciphertext = match tokio::fs::read(&dat_path).await {
            Ok(d) => d,
            Err(_) => return DisplayOverrides::default(),
        };
        let entropy = match tokio::fs::read(&key_path).await {
            Ok(k) => k,
            Err(_) => return DisplayOverrides::default(),
        };
        return match dpapi::unprotect(&ciphertext, &entropy) {
            Ok(plaintext) => {
                let json = String::from_utf8_lossy(&plaintext);
                serde_json::from_str(&json).unwrap_or_default()
            }
            Err(e) => {
                tracing::warn!("failed to decrypt display overrides: {e}");
                DisplayOverrides::default()
            }
        };
    }

    // Legacy plaintext fallback + auto-migrate
    if path.exists() {
        if let Ok(json) = tokio::fs::read_to_string(path).await {
            if let Ok(o) = serde_json::from_str::<DisplayOverrides>(&json) {
                let _ = save_display_overrides(path, &o).await;
                let _ = tokio::fs::remove_file(path).await;
                return o;
            }
            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&json) {
                let o = DisplayOverrides { names: map, order: Vec::new() };
                let _ = save_display_overrides(path, &o).await;
                let _ = tokio::fs::remove_file(path).await;
                return o;
            }
        }
    }

    DisplayOverrides::default()
}

/// Save display overrides encrypted with DPAPI.
pub async fn save_display_overrides(
    path: &Path,
    overrides: &DisplayOverrides,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create dir: {e}"))?;
    }
    let json = serde_json::to_string(overrides)
        .map_err(|e| format!("failed to serialize overrides: {e}"))?;

    let (ciphertext, entropy) =
        dpapi::protect(json.as_bytes()).map_err(|e| format!("DPAPI encrypt failed: {e}"))?;

    let dat_path = path.with_extension("dat");
    let key_path = path.with_extension("key");

    tokio::fs::write(&dat_path, &ciphertext)
        .await
        .map_err(|e| format!("failed to write overrides.dat: {e}"))?;
    tokio::fs::write(&key_path, &entropy)
        .await
        .map_err(|e| format!("failed to write overrides.key: {e}"))
}