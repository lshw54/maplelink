//! Auto-update service — checks GitHub Releases for updates.
//!
//! Supports ghproxy.com mirror for users in mainland China.

use crate::core::error::UpdateError;
use crate::models::update::UpdateInfo;

/// GitHub API endpoint for latest release.
const GITHUB_API_URL: &str = "https://api.github.com/repos/lshw54/maplelink/releases/latest";

/// ghproxy mirror prefix for accelerated downloads in China.
const GHPROXY_PREFIX: &str = "https://mirror.ghproxy.com/";

/// Check GitHub Releases for an available update.
/// Check GitHub Releases for an available update.
///
/// When `include_prerelease` is true, checks all releases (including pre-release).
/// When false, only checks the latest stable release.
pub async fn check_for_update(
    client: &reqwest::Client,
    current_version: &str,
    include_prerelease: bool,
) -> Result<Option<UpdateInfo>, UpdateError> {
    tracing::info!(
        "checking for updates (current: v{current_version}, include_prerelease={include_prerelease})"
    );

    // /releases/latest only returns stable releases (404 if none exist).
    // /releases returns all releases including pre-releases.
    let url = if include_prerelease {
        "https://api.github.com/repos/lshw54/maplelink/releases?per_page=5"
    } else {
        GITHUB_API_URL
    };

    let response = client
        .get(url)
        .header("User-Agent", "MapleLink-Updater")
        .header("Accept", "application/vnd.github.v3+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| UpdateError::CheckFailed {
            reason: format!("network error: {e}"),
        })?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        tracing::info!("no releases found (404)");
        return Ok(None);
    }

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        // GitHub API rate limit (60 req/hr unauthenticated)
        tracing::info!("GitHub API rate limited (403), skipping update check");
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(UpdateError::CheckFailed {
            reason: format!("HTTP {}", response.status()),
        });
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| UpdateError::CheckFailed {
            reason: format!("invalid response: {e}"),
        })?;

    // For /releases (array), pick the first one. For /releases/latest (object), use directly.
    let release = if body.is_array() {
        body.as_array()
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    } else {
        body
    };

    if release.is_null() {
        return Ok(None);
    }

    let tag = release["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    if tag.is_empty() {
        return Ok(None);
    }

    if !is_newer(tag, current_version) {
        tracing::info!("no update available (latest: v{tag})");
        return Ok(None);
    }

    let download_url = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find_map(|a| {
                let name = a["name"].as_str().unwrap_or("");
                if name.to_lowercase().ends_with(".exe") {
                    a["browser_download_url"].as_str().map(String::from)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();

    let changelog = release["body"].as_str().unwrap_or("").to_string();
    let is_prerelease = release["prerelease"].as_bool().unwrap_or(false);

    tracing::info!("update available: v{tag} (prerelease={is_prerelease})");

    Ok(Some(UpdateInfo {
        version: tag.to_string(),
        changelog,
        download_url,
        is_prerelease,
    }))
}

/// Get the download URL, optionally proxied through ghproxy for China users.
pub fn get_download_url(original_url: &str, use_proxy: bool) -> String {
    if use_proxy && !original_url.is_empty() {
        format!("{GHPROXY_PREFIX}{original_url}")
    } else {
        original_url.to_string()
    }
}

/// Test if GitHub API is reachable (for detecting if user needs ghproxy).
pub async fn test_github_connectivity(client: &reqwest::Client) -> bool {
    client
        .head("https://api.github.com")
        .header("User-Agent", "MapleLink-Updater")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Download the update binary.
pub async fn download_update(
    client: &reqwest::Client,
    download_url: &str,
) -> Result<Vec<u8>, UpdateError> {
    tracing::info!("downloading update from {download_url}");

    let response = client
        .get(download_url)
        .header("User-Agent", "MapleLink-Updater")
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("network error: {e}"),
        })?;

    if !response.status().is_success() {
        return Err(UpdateError::DownloadFailed {
            reason: format!("HTTP {}", response.status()),
        });
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("read error: {e}"),
        })?;

    if bytes.is_empty() {
        return Err(UpdateError::CorruptDownload);
    }

    tracing::info!("downloaded {} bytes", bytes.len());
    Ok(bytes.to_vec())
}

/// Download, self-replace, and prompt restart.
pub async fn apply_update(
    update_bytes: &[u8],
    _staging_dir: &std::path::Path,
) -> Result<std::path::PathBuf, UpdateError> {
    let current_exe = std::env::current_exe().map_err(|e| UpdateError::DownloadFailed {
        reason: format!("failed to get current exe path: {e}"),
    })?;

    let backup = current_exe.with_extension("exe.old");
    let temp = current_exe.with_extension("exe.new");

    // Write new exe to temp file next to current exe
    tokio::fs::write(&temp, update_bytes)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to write new exe: {e}"),
        })?;

    // Rename current → .old (Windows allows renaming a running exe)
    if backup.exists() {
        let _ = tokio::fs::remove_file(&backup).await;
    }
    tokio::fs::rename(&current_exe, &backup)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to backup current exe: {e}"),
        })?;

    // Rename new → current
    tokio::fs::rename(&temp, &current_exe).await.map_err(|e| {
        // Try to restore backup
        let _ = std::fs::rename(&backup, &current_exe);
        UpdateError::DownloadFailed {
            reason: format!("failed to replace exe: {e}"),
        }
    })?;

    tracing::info!("self-replace complete, restart required");
    Ok(current_exe)
}

/// Simple semver comparison: returns true if `new` > `current`.
fn is_newer(new: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let n = parse(new);
    let c = parse(current);
    n > c
}

/// Determine whether an auto-update check should run.
pub fn should_check_on_startup(auto_update_enabled: bool) -> bool {
    auto_update_enabled
}

/// Current application version from Cargo.toml.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn is_newer_works() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn current_version_is_non_empty() {
        assert!(!current_version().is_empty());
    }

    proptest! {
        #[test]
        fn prop_disabled_auto_update_skips_check(_dummy in 0u8..10) {
            prop_assert!(!should_check_on_startup(false));
        }

        #[test]
        fn prop_enabled_auto_update_allows_check(_dummy in 0u8..10) {
            prop_assert!(should_check_on_startup(true));
        }
    }
}
