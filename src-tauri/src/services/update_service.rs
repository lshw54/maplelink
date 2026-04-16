//! Auto-update service — checks GitHub Releases for updates.
//!
//! Automatically detects GitHub connectivity and falls back to proxy mirrors
//! (ghproxy.vip, ghproxy.net, ghfast.top) for users in mainland China.
//! The probe result is cached for the entire session.

use std::sync::OnceLock;

use crate::core::error::UpdateError;
use crate::models::update::UpdateInfo;

/// GitHub API endpoint for latest release.
const GITHUB_API_URL: &str = "https://api.github.com/repos/lshw54/maplelink/releases/latest";

/// Proxy mirrors to try when direct GitHub access fails.
const PROXY_MIRRORS: &[&str] = &[
    "https://ghproxy.vip/",
    "https://ghproxy.net/",
    "https://ghfast.top/",
];

/// Cached connectivity probe result.
/// - `None` inside the Option = direct GitHub works (no proxy needed)
/// - `Some(prefix)` = use this proxy prefix for GitHub URLs
static PROXY_CACHE: OnceLock<Option<String>> = OnceLock::new();

/// Ensure the proxy cache is populated. Must be called before github_get.
async fn ensure_proxy_resolved(client: &reqwest::Client) {
    if PROXY_CACHE.get().is_some() {
        return;
    }

    tracing::info!("probing GitHub connectivity...");

    // Test direct access (5 second timeout)
    let direct_ok = client
        .head("https://api.github.com")
        .header("User-Agent", "MapleLink-Updater")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    if direct_ok {
        tracing::info!("GitHub direct access OK, no proxy needed");
        let _ = PROXY_CACHE.set(None);
        return;
    }

    tracing::info!("GitHub direct access failed, testing proxy mirrors...");

    // Test each proxy mirror
    for &mirror in PROXY_MIRRORS {
        let test_url = format!("{mirror}https://api.github.com");
        let ok = client
            .head(&test_url)
            .header("User-Agent", "MapleLink-Updater")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success() || r.status().is_redirection())
            .unwrap_or(false);

        if ok {
            tracing::info!("proxy mirror works: {mirror}");
            let _ = PROXY_CACHE.set(Some(mirror.to_string()));
            return;
        }
        tracing::debug!("proxy mirror failed: {mirror}");
    }

    // No proxy works either — proceed without proxy (will likely fail later)
    tracing::warn!("no proxy mirror reachable, proceeding with direct access");
    let _ = PROXY_CACHE.set(None);
}

/// Apply proxy prefix to a URL if needed.
fn maybe_proxy_url(url: &str) -> String {
    match PROXY_CACHE.get() {
        Some(Some(prefix)) => format!("{prefix}{url}"),
        _ => url.to_string(),
    }
}

/// Check GitHub Releases for an available update.
///
/// When `include_prerelease` is true, checks all releases (including pre-release)
/// and picks the newest version across both stable and pre-release.
/// When false, only checks the latest stable release.
///
/// Automatically uses a proxy mirror if direct GitHub access is unavailable.
pub async fn check_for_update(
    client: &reqwest::Client,
    current_version: &str,
    include_prerelease: bool,
) -> Result<Option<UpdateInfo>, UpdateError> {
    tracing::info!(
        "checking for updates (current: v{current_version}, include_prerelease={include_prerelease})"
    );

    // Ensure proxy detection has run (cached for session)
    ensure_proxy_resolved(client).await;

    if include_prerelease {
        let url =
            maybe_proxy_url("https://api.github.com/repos/lshw54/maplelink/releases?per_page=10");

        let response = github_get(client, &url).await?;
        let response = match response {
            Some(r) => r,
            None => return Ok(None),
        };

        let body: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| UpdateError::CheckFailed {
                    reason: format!("invalid response: {e}"),
                })?;

        let releases = body.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        if releases.is_empty() {
            return Ok(None);
        }

        let mut best: Option<(&serde_json::Value, Vec<u32>)> = None;
        for release in releases {
            let tag = release["tag_name"]
                .as_str()
                .unwrap_or("")
                .trim_start_matches('v');
            if tag.is_empty() {
                continue;
            }
            let parsed = parse_version(tag);
            if parsed.is_empty() || !is_newer(tag, current_version) {
                continue;
            }
            if best.as_ref().is_none_or(|(_, bv)| parsed > *bv) {
                best = Some((release, parsed));
            }
        }

        match best {
            Some((release, _)) => extract_update_info(release),
            None => {
                tracing::info!("no update available (checked all releases)");
                Ok(None)
            }
        }
    } else {
        let url = maybe_proxy_url(GITHUB_API_URL);
        let response = github_get(client, &url).await?;
        let response = match response {
            Some(r) => r,
            None => return Ok(None),
        };

        let release: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| UpdateError::CheckFailed {
                    reason: format!("invalid response: {e}"),
                })?;

        if release.is_null() {
            return Ok(None);
        }

        let tag = release["tag_name"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches('v');

        if tag.is_empty() || !is_newer(tag, current_version) {
            tracing::info!("no update available (latest: v{tag})");
            return Ok(None);
        }

        extract_update_info(&release)
    }
}

/// Send a GET request to the GitHub API, handling 404 and 403 gracefully.
async fn github_get(
    client: &reqwest::Client,
    url: &str,
) -> Result<Option<reqwest::Response>, UpdateError> {
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
        tracing::info!("GitHub API rate limited (403), skipping update check");
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(UpdateError::CheckFailed {
            reason: format!("HTTP {}", response.status()),
        });
    }

    Ok(Some(response))
}

/// Extract `UpdateInfo` from a GitHub release JSON object.
fn extract_update_info(release: &serde_json::Value) -> Result<Option<UpdateInfo>, UpdateError> {
    let tag = release["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

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

/// Parse a version string into a vector of numeric parts.
fn parse_version(s: &str) -> Vec<u32> {
    s.split('.').filter_map(|p| p.parse().ok()).collect()
}

/// Get the download URL, automatically proxied if GitHub is not directly reachable.
/// The `use_proxy` flag from the frontend overrides auto-detection when true.
pub fn get_download_url(original_url: &str, use_proxy: bool) -> String {
    if use_proxy && !original_url.is_empty() {
        // Explicit proxy request from frontend — use cached mirror or first fallback
        match PROXY_CACHE.get() {
            Some(Some(prefix)) => format!("{prefix}{original_url}"),
            _ => format!("{}{original_url}", PROXY_MIRRORS[0]),
        }
    } else if !use_proxy {
        // Check if auto-proxy is active
        maybe_proxy_url(original_url)
    } else {
        original_url.to_string()
    }
}

/// Test if GitHub API is reachable (for frontend proxy toggle detection).
/// Uses the cached probe result if available.
pub async fn test_github_connectivity(client: &reqwest::Client) -> bool {
    ensure_proxy_resolved(client).await;
    matches!(PROXY_CACHE.get(), Some(None))
}

/// Download the update binary.
/// Download the update binary with progress reporting.
///
/// Emits `update-download-progress` events to the given window with:
/// `{ downloaded: u64, total: u64, speed: u64 }` (speed in bytes/sec).
pub async fn download_update_with_progress(
    client: &reqwest::Client,
    download_url: &str,
    app_handle: &tauri::AppHandle,
) -> Result<Vec<u8>, UpdateError> {
    use futures_util::StreamExt;
    use tauri::Emitter;

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

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut buf = Vec::with_capacity(total as usize);
    let mut stream = response.bytes_stream();
    let start = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| UpdateError::DownloadFailed {
            reason: format!("stream error: {e}"),
        })?;
        downloaded += chunk.len() as u64;
        buf.extend_from_slice(&chunk);

        // Emit progress at most every 200ms to avoid flooding
        if last_emit.elapsed().as_millis() >= 200 {
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let speed = (downloaded as f64 / elapsed) as u64;
            let _ = app_handle.emit(
                "update-download-progress",
                serde_json::json!({
                    "downloaded": downloaded,
                    "total": total,
                    "speed": speed,
                }),
            );
            last_emit = std::time::Instant::now();
        }
    }

    // Final progress event
    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    let speed = (downloaded as f64 / elapsed) as u64;
    let _ = app_handle.emit(
        "update-download-progress",
        serde_json::json!({
            "downloaded": downloaded,
            "total": total,
            "speed": speed,
        }),
    );

    if buf.is_empty() {
        return Err(UpdateError::CorruptDownload);
    }

    tracing::info!("downloaded {} bytes in {:.1}s", buf.len(), elapsed);
    Ok(buf)
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

    tokio::fs::write(&temp, update_bytes)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to write new exe: {e}"),
        })?;

    if backup.exists() {
        let _ = tokio::fs::remove_file(&backup).await;
    }
    tokio::fs::rename(&current_exe, &backup)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to backup current exe: {e}"),
        })?;

    tokio::fs::rename(&temp, &current_exe).await.map_err(|e| {
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

/// Whether a proxy mirror is currently active (for frontend display).
pub fn is_proxy_active() -> bool {
    matches!(PROXY_CACHE.get(), Some(Some(_)))
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
