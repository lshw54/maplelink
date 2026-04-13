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
pub async fn check_for_update(
    client: &reqwest::Client,
    current_version: &str,
) -> Result<Option<UpdateInfo>, UpdateError> {
    tracing::info!("checking for updates (current: v{current_version})");

    let response = client
        .get(GITHUB_API_URL)
        .header("User-Agent", "MapleLink-Updater")
        .header("Accept", "application/vnd.github.v3+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| UpdateError::CheckFailed {
            reason: format!("network error: {e}"),
        })?;

    if !response.status().is_success() {
        return Err(UpdateError::CheckFailed {
            reason: format!("HTTP {}", response.status()),
        });
    }

    let release: serde_json::Value =
        response
            .json()
            .await
            .map_err(|e| UpdateError::CheckFailed {
                reason: format!("invalid response: {e}"),
            })?;

    let tag = release["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    if tag.is_empty() {
        return Ok(None);
    }

    // Compare versions
    if !is_newer(tag, current_version) {
        tracing::info!("no update available (latest: v{tag})");
        return Ok(None);
    }

    // Find the .exe asset (standalone exe, not NSIS installer)
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

/// Save downloaded update to a temp file and launch the installer.
pub async fn apply_update(
    update_bytes: &[u8],
    staging_dir: &std::path::Path,
) -> Result<std::path::PathBuf, UpdateError> {
    tokio::fs::create_dir_all(staging_dir)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to create staging dir: {e}"),
        })?;

    let installer_path = staging_dir.join("MapleLink_update.exe");
    tokio::fs::write(&installer_path, update_bytes)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to write installer: {e}"),
        })?;

    tracing::info!("update staged at {}", installer_path.display());
    Ok(installer_path)
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
