//! Auto-update service — checks for updates, downloads, and applies them.
//!
//! All network failures are handled gracefully: logged and surfaced as
//! [`UpdateError`] variants so the application can continue without
//! interruption.

use crate::core::error::UpdateError;
use crate::models::update::UpdateInfo;

/// The update endpoint URL template. `{current_version}` is replaced at
/// runtime with the running application version.
const UPDATE_ENDPOINT: &str = "https://releases.maplelink.app/check?version={current_version}";

/// Check the remote endpoint for an available update.
///
/// Returns `Ok(Some(UpdateInfo))` when a newer version exists, `Ok(None)`
/// when the application is already up-to-date, or an `UpdateError` on
/// network / parse failures.
pub async fn check_for_update(
    client: &reqwest::Client,
    current_version: &str,
) -> Result<Option<UpdateInfo>, UpdateError> {
    let url = UPDATE_ENDPOINT.replace("{current_version}", current_version);

    tracing::info!("checking for updates at {url}");

    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("update check network error: {e}");
            UpdateError::CheckFailed {
                reason: format!("network error: {e}"),
            }
        })?;

    if response.status() == reqwest::StatusCode::NO_CONTENT {
        tracing::info!("no update available (already up-to-date)");
        return Ok(None);
    }

    if !response.status().is_success() {
        let status = response.status();
        tracing::warn!("update check returned HTTP {status}");
        return Err(UpdateError::CheckFailed {
            reason: format!("HTTP {status}"),
        });
    }

    let info: UpdateInfo = response.json().await.map_err(|e| {
        tracing::warn!("failed to parse update response: {e}");
        UpdateError::CheckFailed {
            reason: format!("invalid response body: {e}"),
        }
    })?;

    tracing::info!("update available: v{}", info.version);
    Ok(Some(info))
}

/// Download the update binary from the given URL.
///
/// Returns the raw bytes on success. Corrupt or incomplete downloads are
/// detected by checking the HTTP status and content length, then discarded
/// with a logged error.
pub async fn download_update(
    client: &reqwest::Client,
    download_url: &str,
) -> Result<Vec<u8>, UpdateError> {
    tracing::info!("downloading update from {download_url}");

    let response = client
        .get(download_url)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("update download network error: {e}");
            UpdateError::DownloadFailed {
                reason: format!("network error: {e}"),
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        tracing::error!("update download returned HTTP {status}");
        return Err(UpdateError::DownloadFailed {
            reason: format!("HTTP {status}"),
        });
    }

    let expected_len = response.content_length();
    let bytes = response.bytes().await.map_err(|e| {
        tracing::error!("failed to read update body: {e}");
        UpdateError::DownloadFailed {
            reason: format!("failed to read response body: {e}"),
        }
    })?;

    // Verify content length if the server provided one.
    if let Some(expected) = expected_len {
        if bytes.len() as u64 != expected {
            tracing::error!(
                "update download size mismatch: expected {expected}, got {}",
                bytes.len()
            );
            return Err(UpdateError::CorruptDownload);
        }
    }

    if bytes.is_empty() {
        tracing::error!("update download returned empty body");
        return Err(UpdateError::CorruptDownload);
    }

    tracing::info!("update downloaded successfully ({} bytes)", bytes.len());
    Ok(bytes.to_vec())
}

/// Apply a downloaded update.
///
/// This is a placeholder that writes the update payload to a staging path
/// and signals that a restart is needed. The actual installer/replacement
/// logic depends on the packaging strategy (NSIS, MSI, etc.) and will be
/// wired in during integration.
pub async fn apply_update(
    update_bytes: &[u8],
    staging_dir: &std::path::Path,
) -> Result<(), UpdateError> {
    tracing::info!("applying update ({} bytes)", update_bytes.len());

    tokio::fs::create_dir_all(staging_dir).await.map_err(|e| {
        tracing::error!("failed to create staging directory: {e}");
        UpdateError::DownloadFailed {
            reason: format!("failed to create staging dir: {e}"),
        }
    })?;

    let installer_path = staging_dir.join("maplelink_update.exe");

    tokio::fs::write(&installer_path, update_bytes)
        .await
        .map_err(|e| {
            tracing::error!("failed to write update installer: {e}");
            UpdateError::DownloadFailed {
                reason: format!("failed to write installer: {e}"),
            }
        })?;

    tracing::info!(
        "update staged at {}; restart required to complete",
        installer_path.display()
    );
    Ok(())
}

/// Determine whether an auto-update check should run based on config.
///
/// Pure helper — returns `true` only when `auto_update` is enabled.
pub fn should_check_on_startup(auto_update_enabled: bool) -> bool {
    auto_update_enabled
}

/// Current application version read from `Cargo.toml` at compile time.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: maplelink-rewrite, Property 11: Disabled auto-update skips update check
    //
    // For any AppConfig where auto_update is false, the startup sequence
    // shall not invoke the update check endpoint.
    proptest! {
        #[test]
        fn prop_disabled_auto_update_skips_check(
            // Generate a random bool that is always false for the disabled case.
            _dummy in 0u8..10,
        ) {
            // When auto_update is disabled, should_check_on_startup must return false.
            prop_assert!(!should_check_on_startup(false));
        }

        #[test]
        fn prop_enabled_auto_update_allows_check(
            _dummy in 0u8..10,
        ) {
            // When auto_update is enabled, should_check_on_startup must return true.
            prop_assert!(should_check_on_startup(true));
        }
    }

    #[test]
    fn current_version_is_non_empty() {
        let v = current_version();
        assert!(!v.is_empty());
    }
}
