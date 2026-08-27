//! Auto-update service — checks GitHub Releases for updates.
//!
//! Automatically detects GitHub connectivity and falls back to proxy mirrors
//! (ghproxy.net, ghfast.top, gh-proxy.com, ghproxy.cc) for users in mainland
//! China. Each mirror is probed before use, so a dead one is simply skipped.
//! The probe result is cached for the entire session.
//!
//! Between "direct works" and "use a mirror" sits a third route: direct, but
//! with GitHub's IPs supplied by [`github_hosts`] instead of by a poisoned
//! resolver. It is tried only after the plain direct probe fails, so users who
//! can already reach GitHub pay nothing for it and keep their own DNS.

use std::sync::OnceLock;

use crate::core::error::UpdateError;
use crate::models::update::UpdateInfo;
use crate::services::{github_hosts, http_util};

/// GitHub API endpoint for latest release.
const GITHUB_API_URL: &str = "https://api.github.com/repos/lshw54/maplelink/releases/latest";

/// The release asset the self-replace update path consumes.
///
/// Every release also ships `MapleLink-Setup.exe`, a self-extracting archive
/// meant for new users only. Matching on any `.exe` picked that one up (GitHub
/// orders `MapleLink-Setup.exe` first — `-` sorts before `.`), so updating left
/// users running the extractor, which unpacks a fresh copy on every launch.
const UPDATE_ASSET: &str = "MapleLink.exe";

/// Proxy mirrors to try (in order) when direct GitHub access fails. Each is
/// probed in `ensure_proxy_resolved` before use, so unreachable ones are
/// skipped — extra entries only add redundancy. (Old `ghproxy.com` is dead.)
const PROXY_MIRRORS: &[&str] = &[
    "https://ghproxy.net/",
    "https://ghfast.top/",
    "https://gh-proxy.com/",
    "https://ghproxy.cc/",
];

/// A ceiling on the update itself. The build is ~12 MB and the largest shipped
/// was 23 MB, so this is several times any real one — it bounds what a mirror
/// can make us hold, not what a build may weigh.
const MAX_UPDATE_BYTES: u64 = 256 * 1024 * 1024;

/// How much to reserve up front for a download that declares `declared` bytes.
///
/// `Content-Length` is whatever the mirror said, and the mirrors are third
/// parties. Unclamped, one answering `Content-Length: 999999999999` had us ask
/// the allocator for 931 GB before a single byte arrived — which fails, and a
/// failed allocation aborts the process. Not a panic: no unwind, no message the
/// user sees, the app is simply gone mid-update.
fn download_buffer_capacity(declared: u64) -> usize {
    declared.min(MAX_UPDATE_BYTES) as usize
}

/// How much release metadata is worth reading. Thirty releases with their
/// assets is well under a megabyte; this is room to spare, and a stop on a
/// mirror answering the API with something else.
const MAX_RELEASE_JSON_BYTES: u64 = 8 * 1024 * 1024;

/// Read a release listing, bounded.
async fn read_release_json(response: reqwest::Response) -> Result<serde_json::Value, UpdateError> {
    let body = http_util::read_capped(response, MAX_RELEASE_JSON_BYTES)
        .await
        .map_err(|reason| UpdateError::CheckFailed { reason })?;
    serde_json::from_slice(&body).map_err(|e| UpdateError::CheckFailed {
        reason: format!("invalid response: {e}"),
    })
}

/// Cached connectivity probe result.
/// - `None` inside the Option = direct GitHub works (no proxy needed)
/// - `Some(prefix)` = use this proxy prefix for GitHub URLs
static PROXY_CACHE: OnceLock<Option<String>> = OnceLock::new();

/// The hosts-override client, once one has been proven to reach GitHub. Held
/// for the session so the download uses the same route the check did.
static HOSTS_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Can this client reach the GitHub API directly?
async fn direct_reachable(client: &reqwest::Client) -> bool {
    client
        .head("https://api.github.com")
        .header("User-Agent", "MapleLink-Updater")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Find a working proxy mirror and record it, or record "no proxy" if none
/// answers.
async fn resolve_mirror(client: &reqwest::Client) {
    tracing::info!("GitHub direct access failed, testing proxy mirrors...");

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

/// Ensure the proxy cache is populated. Must be called before github_get.
///
/// This is the hosts-less path, kept for callers that have no config to consult
/// — [`resolve_route`] is what the update flow actually uses.
async fn ensure_proxy_resolved(client: &reqwest::Client) {
    if PROXY_CACHE.get().is_some() {
        return;
    }

    tracing::info!("probing GitHub connectivity...");

    if direct_reachable(client).await {
        tracing::info!("GitHub direct access OK, no proxy needed");
        let _ = PROXY_CACHE.set(None);
        return;
    }

    resolve_mirror(client).await;
}

/// Decide how this session reaches GitHub, and hand back the client to use for
/// it. Resolved once per session; later calls just return the same client.
///
/// The order is direct → hosts-override → proxy mirror, each step tried only
/// because the one before it failed. `hosts_enabled` is the user's setting;
/// with it off the behaviour is exactly what it was before the override
/// existed.
pub async fn resolve_route(client: &reqwest::Client, hosts_enabled: bool) -> reqwest::Client {
    if PROXY_CACHE.get().is_some() {
        return github_client(client);
    }

    tracing::info!("probing GitHub connectivity...");

    if direct_reachable(client).await {
        tracing::info!("GitHub direct access OK, no proxy needed");
        let _ = PROXY_CACHE.set(None);
        return client.clone();
    }

    if hosts_enabled {
        let map = github_hosts::load(client, PROXY_MIRRORS).await;
        if let Some(hosts_client) = github_hosts::build_client(&map) {
            if direct_reachable(&hosts_client).await {
                tracing::info!("GitHub reachable directly using the hosts override");
                let _ = PROXY_CACHE.set(None);
                let _ = HOSTS_CLIENT.set(hosts_client.clone());
                return hosts_client;
            }
            tracing::info!("hosts override did not make GitHub reachable, falling back to mirrors");
        }
    }

    resolve_mirror(client).await;
    client.clone()
}

/// The client to send GitHub traffic through: the hosts-override one when this
/// session established that route, otherwise `fallback`.
///
/// Overrides only cover GitHub's own domains, so the same client still reaches
/// the proxy mirrors normally.
pub fn github_client(fallback: &reqwest::Client) -> reqwest::Client {
    HOSTS_CLIENT
        .get()
        .cloned()
        .unwrap_or_else(|| fallback.clone())
}

/// Measure how fast `url` actually delivers bytes, by pulling the first slice of
/// the real asset. Returns bytes/sec, or `None` if it never produced any data.
///
/// Reachability and throughput are different questions: a mirror can answer a
/// HEAD instantly and then trickle the file. This asks the question that
/// matters, on the file being downloaded.
async fn measure_speed(client: &reqwest::Client, url: &str) -> Option<u64> {
    use futures_util::StreamExt;

    /// Enough of the file to see past connection setup, small enough that
    /// probing every mirror stays cheap.
    const PROBE_BYTES: u64 = 384 * 1024;
    const PROBE_LIMIT: std::time::Duration = std::time::Duration::from_secs(4);

    let started = std::time::Instant::now();
    let response = client
        .get(url)
        .header("User-Agent", "MapleLink-Updater")
        // Mirrors that ignore Range just start sending the whole file; the read
        // loop below stops either way.
        .header("Range", format!("bytes=0-{}", PROBE_BYTES - 1))
        .timeout(PROBE_LIMIT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }

    let mut got: u64 = 0;
    let mut stream = response.bytes_stream();
    // On timeout the read future is dropped and `got` keeps whatever arrived, so
    // a slow mirror still scores rather than being discarded.
    let _ = tokio::time::timeout(PROBE_LIMIT, async {
        while let Some(Ok(chunk)) = stream.next().await {
            got += chunk.len() as u64;
            if got >= PROBE_BYTES {
                break;
            }
        }
    })
    .await;

    let secs = started.elapsed().as_secs_f64();
    (got > 0 && secs > 0.0).then(|| (got as f64 / secs) as u64)
}

/// Pick the fastest way to fetch `url`, by racing the candidates against each
/// other on the actual file.
///
/// Mirror speed swings by the hour, so the mirror that answered a probe at
/// startup is no guide to which one will serve the download now. `direct_too`
/// adds GitHub itself to the race — left out when the user explicitly asked for
/// a proxy.
pub async fn fastest_download_url(client: &reqwest::Client, url: &str, direct_too: bool) -> String {
    let mut candidates: Vec<String> = Vec::new();
    if direct_too {
        candidates.push(url.to_string());
    }
    candidates.extend(PROXY_MIRRORS.iter().map(|m| format!("{m}{url}")));

    let measured = futures_util::future::join_all(
        candidates
            .iter()
            .map(|c| async move { (c.clone(), measure_speed(client, c).await) }),
    )
    .await;

    let mut best: Option<(String, u64)> = None;
    for (candidate, speed) in measured {
        match speed {
            Some(bps) => {
                tracing::info!("mirror probe: {} KB/s — {candidate}", bps / 1024);
                if best.as_ref().is_none_or(|(_, b)| bps > *b) {
                    best = Some((candidate, bps));
                }
            }
            None => tracing::debug!("mirror probe: no response — {candidate}"),
        }
    }

    match best {
        Some((candidate, bps)) => {
            tracing::info!("downloading via the fastest route ({} KB/s)", bps / 1024);
            candidate
        }
        None => {
            tracing::warn!("no candidate responded to the speed probe; using the default route");
            maybe_proxy_url(url)
        }
    }
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
        // Collect candidates from the releases LIST (which includes pre-releases)
        // AND from /releases/latest. GitHub's /releases list has been observed to
        // omit a freshly-published stable release (e.g. v0.4.0 was returned by
        // /releases/latest and /releases/tags/v0.4.0 but was absent from the list),
        // so relying on the list alone can miss the newest stable build on the
        // pre-release channel. Take the highest version across both sources.
        let mut candidates: Vec<UpdateInfo> = Vec::new();

        let list_url =
            maybe_proxy_url("https://api.github.com/repos/lshw54/maplelink/releases?per_page=30");
        if let Some(response) = github_get(client, &list_url).await? {
            let body: serde_json::Value = read_release_json(response).await?;
            for release in body.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                let tag = release["tag_name"]
                    .as_str()
                    .unwrap_or("")
                    .trim_start_matches('v');
                if tag.is_empty() || !is_newer(tag, current_version) {
                    continue;
                }
                if let Some(info) = extract_update_info(release)? {
                    candidates.push(info);
                }
            }
        }

        // Newest stable — covers the list-omission quirk above.
        let latest_url = maybe_proxy_url(GITHUB_API_URL);
        if let Some(response) = github_get(client, &latest_url).await? {
            let release: serde_json::Value = read_release_json(response).await?;
            if !release.is_null() {
                let tag = release["tag_name"]
                    .as_str()
                    .unwrap_or("")
                    .trim_start_matches('v');
                if !tag.is_empty() && is_newer(tag, current_version) {
                    if let Some(info) = extract_update_info(&release)? {
                        candidates.push(info);
                    }
                }
            }
        }

        let best = candidates
            .into_iter()
            .max_by(|a, b| parse_version(&a.version).cmp(&parse_version(&b.version)));
        if best.is_none() {
            tracing::info!("no update available (checked all releases + latest)");
        }
        Ok(best)
    } else {
        let url = maybe_proxy_url(GITHUB_API_URL);
        let response = github_get(client, &url).await?;
        let response = match response {
            Some(r) => r,
            None => return Ok(None),
        };

        let release: serde_json::Value = read_release_json(response).await?;

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
                if name.eq_ignore_ascii_case(UPDATE_ASSET) {
                    a["browser_download_url"].as_str().map(String::from)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();

    if download_url.is_empty() {
        tracing::warn!("release v{tag} ships no {UPDATE_ASSET} — it cannot be applied");
    }

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
///
/// "Reachable" includes reachable via the hosts override — that route is still
/// a direct connection, so the dialog should not push the user onto a mirror.
pub async fn test_github_connectivity(client: &reqwest::Client, hosts_enabled: bool) -> bool {
    resolve_route(client, hosts_enabled).await;
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

    // Try streaming download first for progress reporting.
    // Some proxy mirrors return responses that fail during streaming
    // (chunked encoding issues, gzip decode errors). If streaming fails,
    // fall back to a simple non-streaming download.
    let buf = {
        let mut downloaded: u64 = 0;
        let mut buf = Vec::with_capacity(download_buffer_capacity(total));
        let mut stream = response.bytes_stream();
        let start = std::time::Instant::now();
        let mut last_emit = std::time::Instant::now();
        let mut stream_failed = false;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    // The clamp only bounds the reservation. A sender free to
                    // lie about the length is equally free to keep sending, so
                    // what actually arrives is counted too.
                    if downloaded + chunk.len() as u64 > MAX_UPDATE_BYTES {
                        return Err(UpdateError::DownloadFailed {
                            reason: format!(
                                "the download ran past {MAX_UPDATE_BYTES} bytes without ending"
                            ),
                        });
                    }
                    downloaded += chunk.len() as u64;
                    buf.extend_from_slice(&chunk);

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
                Err(e) => {
                    tracing::warn!(
                        "stream error during download: {e}, falling back to non-streaming"
                    );
                    stream_failed = true;
                    break;
                }
            }
        }

        if stream_failed {
            // Fallback: re-download without streaming
            tracing::info!("retrying download without streaming...");
            let _ = app_handle.emit(
                "update-download-progress",
                serde_json::json!({ "downloaded": 0u64, "total": total, "speed": 0u64 }),
            );

            let fallback_resp = client
                .get(download_url)
                .header("User-Agent", "MapleLink-Updater")
                .timeout(std::time::Duration::from_secs(300))
                .send()
                .await
                .map_err(|e| UpdateError::DownloadFailed {
                    reason: format!("fallback network error: {e}"),
                })?;

            // Same mirror, same ceiling — `bytes()` would read whatever it is
            // given.
            let bytes = http_util::read_capped(fallback_resp, MAX_UPDATE_BYTES)
                .await
                .map_err(|reason| UpdateError::DownloadFailed {
                    reason: format!("fallback read error: {reason}"),
                })?;

            let _ = app_handle.emit(
                "update-download-progress",
                serde_json::json!({
                    "downloaded": bytes.len() as u64,
                    "total": bytes.len() as u64,
                    "speed": 0u64,
                }),
            );

            bytes.to_vec()
        } else {
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
            buf
        }
    };

    if buf.is_empty() {
        return Err(UpdateError::CorruptDownload);
    }

    tracing::info!("downloaded {} bytes", buf.len());
    Ok(buf)
}

/// Download, self-replace, and prompt restart.
pub async fn apply_update(
    update_bytes: &[u8],
    staging_dir: &std::path::Path,
) -> Result<std::path::PathBuf, UpdateError> {
    // Use current_exe() as the target so the update writes back to WHATEVER name
    // the user is running under (e.g. accelerator users rename to Beanfun.exe).
    // This is intentional — do not hardcode "MapleLink.exe" here, or auto-update
    // would silently revert a user's rename.
    let current_exe = std::env::current_exe().map_err(|e| UpdateError::DownloadFailed {
        reason: format!("failed to get current exe path: {e}"),
    })?;

    swap_in_new_exe(update_bytes, staging_dir, &current_exe).await
}

/// Put the downloaded build where the running one is, keeping the old one until
/// that has actually happened. Split from [`apply_update`] so the swap can be
/// tested against real files rather than against whatever is running the tests.
async fn swap_in_new_exe(
    update_bytes: &[u8],
    staging_dir: &std::path::Path,
    current_exe: &std::path::Path,
) -> Result<std::path::PathBuf, UpdateError> {
    let staged = stage_new_exe(update_bytes, staging_dir, current_exe).await?;
    swap_staged_exe(&staged, update_bytes.len() as u64, current_exe).await
}

/// Replace `current_exe` with an already-staged build of `expected_len` bytes,
/// keeping the old one until that has happened.
async fn swap_staged_exe(
    staged: &std::path::Path,
    expected_len: u64,
    current_exe: &std::path::Path,
) -> Result<std::path::PathBuf, UpdateError> {
    swap_staged_exe_using(staged, expected_len, current_exe, move_file).await
}

/// As [`swap_staged_exe`], with the move injected.
///
/// That move is the one step that can fail *after* the running program has been
/// moved aside, so it is the only way into the recovery path below — and it
/// fails for reasons a test cannot arrange on demand (a volume filling up, a
/// scanner holding the destination open). Passing it in is what lets the
/// recovery be tested at all, rather than reasoned about.
async fn swap_staged_exe_using(
    staged: &std::path::Path,
    expected_len: u64,
    current_exe: &std::path::Path,
    mover: fn(&std::path::Path, &std::path::Path) -> std::io::Result<()>,
) -> Result<std::path::PathBuf, UpdateError> {
    // Read the staged file back before anything irreversible happens. Anti-virus
    // software that dislikes the download deletes it between the write and the
    // swap — and by then the running program has been moved aside, leaving only
    // the rollback below between the user and a directory with nothing runnable
    // in it. Checking first turns that into an update that simply did not
    // happen, which is a thing the user can retry.
    let staged_len = tokio::fs::metadata(staged)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    if staged_len != expected_len {
        let _ = tokio::fs::remove_file(staged).await;
        return Err(UpdateError::DownloadFailed {
            reason: format!(
                "the staged update is {staged_len} bytes rather than \
                 {expected_len}; anti-virus software may have removed it"
            ),
        });
    }

    let backup = current_exe.with_extension("exe.old");
    if backup.exists() {
        let _ = tokio::fs::remove_file(&backup).await;
    }
    tokio::fs::rename(current_exe, &backup)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to backup current exe: {e}"),
        })?;

    if let Err(e) = mover(staged, current_exe) {
        // Put the user's program back. If even that fails there is nothing left
        // to run, which is a different situation from a failed update and has to
        // read as one — including where the only remaining copy is.
        return Err(match std::fs::rename(&backup, current_exe) {
            Ok(()) => UpdateError::DownloadFailed {
                reason: format!("failed to replace exe: {e}"),
            },
            Err(restore) => UpdateError::DownloadFailed {
                reason: format!(
                    "failed to replace exe: {e}; the previous version could not be \
                     put back either ({restore}) and is at {}",
                    backup.display()
                ),
            },
        });
    }

    // `move_file` may have fallen back to copying, which leaves the original.
    let _ = tokio::fs::remove_file(staged).await;

    tracing::info!("self-replace complete, restart required");
    Ok(current_exe.to_path_buf())
}

/// Write the downloaded build somewhere it can wait, under the name it will run
/// under.
///
/// This used to be `<name>.exe.new`, written beside the running program. A PE
/// with a disguised extension, dropped next to a running program by that
/// program, is the shape of a dropper — and Windows Defender scores shapes:
/// users running from their desktop had the download quarantined as
/// `Trojan:Win32/Bearfoos.A!ml`, a machine-learning verdict on a file it had
/// never seen before, so the update died before it could be swapped in.
///
/// Signing the binaries is what actually fixes that, and this does not replace
/// it. But a half-written executable belongs in the app's own data directory
/// with a real extension regardless of who is watching; someone's desktop is
/// not a scratch directory.
async fn stage_new_exe(
    update_bytes: &[u8],
    staging_dir: &std::path::Path,
    current_exe: &std::path::Path,
) -> Result<std::path::PathBuf, UpdateError> {
    tokio::fs::create_dir_all(staging_dir)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to prepare {}: {e}", staging_dir.display()),
        })?;

    // The name the user runs under, so the staged copy carries the same
    // extension it will keep — and so a renamed install stays renamed.
    let name = current_exe
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("MapleLink.exe"));
    let staged = staging_dir.join(name);

    tokio::fs::write(&staged, update_bytes)
        .await
        .map_err(|e| UpdateError::DownloadFailed {
            reason: format!("failed to write {}: {e}", staged.display()),
        })?;

    Ok(staged)
}

/// Move a file that may be on a different volume from its destination: the data
/// directory and the program need not share one, and `rename` cannot cross that
/// boundary.
fn move_file(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to)?;
            let _ = std::fs::remove_file(from);
            Ok(())
        }
    }
}

/// Simple semver comparison: returns true if `new` > `current`.
fn is_newer(new: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let n = parse(new);
    let c = parse(current);
    n > c
}

/// Determine whether an auto-update check should run.
pub fn should_check(is_manual: bool, auto_update_enabled: bool) -> bool {
    // A manual check is the user asking outright, so it ignores the setting.
    // Everything else — startup, the frontend's missed-event fallback — must
    // stay silent when the user turned auto-update off, or the toggle does
    // nothing.
    is_manual || auto_update_enabled
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

    /// A release JSON shaped like ours: GitHub lists `MapleLink-Setup.exe`
    /// first, because `-` sorts before `.`.
    fn release_json() -> serde_json::Value {
        serde_json::json!({
            "tag_name": "v9.9.9",
            "body": "notes",
            "prerelease": false,
            "assets": [
                {
                    "name": "MapleLink-Setup.exe",
                    "browser_download_url": "https://example.com/MapleLink-Setup.exe"
                },
                {
                    "name": "MapleLink.exe",
                    "browser_download_url": "https://example.com/MapleLink.exe"
                }
            ]
        })
    }

    #[test]
    fn picks_the_standalone_exe_not_the_installer() {
        let info = extract_update_info(&release_json()).unwrap().unwrap();
        assert_eq!(info.download_url, "https://example.com/MapleLink.exe");
    }

    #[test]
    fn download_url_is_empty_when_the_standalone_exe_is_missing() {
        let release = serde_json::json!({
            "tag_name": "v9.9.9",
            "body": "",
            "prerelease": false,
            "assets": [
                {
                    "name": "MapleLink-Setup.exe",
                    "browser_download_url": "https://example.com/MapleLink-Setup.exe"
                }
            ]
        });
        let info = extract_update_info(&release).unwrap().unwrap();
        // Better to offer nothing than to hand the extractor to self-replace.
        assert!(info.download_url.is_empty());
    }

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

    // Feature: maplelink-rewrite, Property 11: Disabled auto-update skips update check
    //
    // For any AppConfig where auto_update is false, no background check may run —
    // not at startup, and not from the frontend's missed-event fallback. Both go
    // through `should_check`, which is the whole gate.
    //
    // The earlier version of this test only covered the auto_update argument, so
    // it stayed green while every real check passed is_manual = true and skipped
    // the gate entirely. The is_manual row below is the one that matters.
    //
    // **Validates: Requirements 8.6**
    #[test]
    fn should_check_truth_table() {
        // Background check, user turned auto-update off — the toggle must hold.
        assert!(!should_check(false, false));
        // Background check, auto-update on.
        assert!(should_check(false, true));
        // The user pressed "check now"; the setting does not apply.
        assert!(should_check(true, false));
        assert!(should_check(true, true));
    }

    proptest! {
        #[test]
        fn prop_disabled_auto_update_skips_background_check(_dummy in 0u8..10) {
            prop_assert!(!should_check(false, false));
        }

        #[test]
        fn prop_enabled_auto_update_allows_background_check(_dummy in 0u8..10) {
            prop_assert!(should_check(false, true));
        }
    }

    /// A real update sizes its own buffer.
    #[test]
    fn an_honest_length_is_reserved_in_full() {
        let real = 11_800_000;
        assert_eq!(download_buffer_capacity(real), real as usize);
    }

    /// A claimed one cannot. 999999999999 is what the repro used; unclamped it
    /// aborted the process with 0xC0000409 before any byte was read.
    #[test]
    fn a_claimed_length_cannot_reserve_more_than_a_real_update() {
        assert_eq!(
            download_buffer_capacity(999_999_999_999),
            MAX_UPDATE_BYTES as usize
        );
        assert_eq!(
            download_buffer_capacity(u64::MAX),
            MAX_UPDATE_BYTES as usize
        );
    }

    /// The clamp must not be so generous that it is the same bug in slow motion:
    /// whatever is reserved has to be an amount a machine can actually give.
    #[test]
    fn the_ceiling_is_an_allocatable_amount() {
        let ceiling = download_buffer_capacity(u64::MAX);
        assert!(
            ceiling <= 512 * 1024 * 1024,
            "{ceiling} is too much to hold"
        );
        let buf: Vec<u8> = Vec::with_capacity(ceiling);
        assert_eq!(buf.capacity(), ceiling);
    }

    // ----- self-replace -------------------------------------------------

    fn swap_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("maplelink_swap_{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn the_staged_build_keeps_the_name_it_will_run_under() {
        let dir = swap_dir("staged_name");
        let staging = dir.join("updates");
        // An accelerator user's rename, which the update must not undo.
        let current = dir.join("Beanfun (2).exe");

        let staged = stage_new_exe(b"new build", &staging, &current)
            .await
            .unwrap();

        assert_eq!(staged.file_name().unwrap(), "Beanfun (2).exe");
        assert_eq!(staged.parent().unwrap(), staging);
        // Nothing named `.exe.new` anywhere near the program itself: that shape
        // is what got the download quarantined.
        assert!(!dir.join("Beanfun (2).exe.new").exists());
        assert_eq!(std::fs::read(&staged).unwrap(), b"new build");
    }

    #[tokio::test]
    async fn a_swap_replaces_the_exe_and_keeps_the_old_one() {
        let dir = swap_dir("swap_ok");
        let current = dir.join("MapleLink.exe");
        std::fs::write(&current, b"old build").unwrap();

        swap_in_new_exe(b"new build", &dir.join("updates"), &current)
            .await
            .unwrap();

        assert_eq!(std::fs::read(&current).unwrap(), b"new build");
        assert_eq!(
            std::fs::read(dir.join("MapleLink.exe.old")).unwrap(),
            b"old build"
        );
        // The staging copy is not left behind to be scanned again later.
        assert!(!dir.join("updates").join("MapleLink.exe").exists());
    }

    /// The case this guard exists for: anti-virus quarantines the download
    /// between staging and the swap. The running program must still be there.
    #[tokio::test]
    async fn a_quarantined_download_leaves_the_running_exe_alone() {
        let dir = swap_dir("swap_quarantined");
        let current = dir.join("MapleLink.exe");
        std::fs::write(&current, b"old build").unwrap();
        let staged = dir.join("updates").join("MapleLink.exe");
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        // Staged, then taken away — exactly what Defender did to `.exe.new`.
        std::fs::write(&staged, b"new build").unwrap();
        std::fs::remove_file(&staged).unwrap();

        let err = swap_staged_exe(&staged, b"new build".len() as u64, &current)
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("anti-virus"),
            "the reason should point at the likely cause, got: {err}"
        );
        assert_eq!(std::fs::read(&current).unwrap(), b"old build");
        // Never moved aside in the first place, so there is nothing to restore.
        assert!(!dir.join("MapleLink.exe.old").exists());
    }

    /// A truncated staged file is the same hazard wearing a different hat.
    #[tokio::test]
    async fn a_half_written_download_is_refused_too() {
        let dir = swap_dir("swap_truncated");
        let current = dir.join("MapleLink.exe");
        std::fs::write(&current, b"old build").unwrap();
        let staged = dir.join("updates").join("MapleLink.exe");
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, b"new").unwrap();

        swap_staged_exe(&staged, b"new build".len() as u64, &current)
            .await
            .unwrap_err();

        assert_eq!(std::fs::read(&current).unwrap(), b"old build");
        // The bad copy is cleared out rather than left to be retried.
        assert!(!staged.exists());
    }

    #[tokio::test]
    async fn a_copy_is_used_when_a_rename_cannot_reach_the_destination() {
        let dir = swap_dir("move_fallback");
        let from = dir.join("from.exe");
        let to = dir.join("to.exe");
        std::fs::write(&from, b"payload").unwrap();

        move_file(&from, &to).unwrap();

        assert_eq!(std::fs::read(&to).unwrap(), b"payload");
        assert!(!from.exists());
    }

    /// The move fails after the running program has been moved aside, and the
    /// rollback puts it back. The user gets a failed update, not a missing app.
    #[tokio::test]
    async fn a_failed_move_puts_the_old_exe_back() {
        let dir = swap_dir("swap_rollback");
        let current = dir.join("MapleLink.exe");
        std::fs::write(&current, b"old build").unwrap();
        let staged = dir.join("updates").join("MapleLink.exe");
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, b"new build").unwrap();

        fn refuse(_: &std::path::Path, _: &std::path::Path) -> std::io::Result<()> {
            Err(std::io::Error::other("no"))
        }
        let err = swap_staged_exe_using(&staged, b"new build".len() as u64, &current, refuse)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("failed to replace exe"));
        assert_eq!(std::fs::read(&current).unwrap(), b"old build");
        assert!(!dir.join("MapleLink.exe.old").exists());
    }

    /// The rollback fails too — the one case where the user is left with
    /// nothing to run. The error has to say that, and say where the old build
    /// went, rather than reporting only the move that failed first.
    #[tokio::test]
    async fn a_failed_rollback_says_so_and_names_the_backup() {
        let dir = swap_dir("swap_rollback_fails");
        let current = dir.join("MapleLink.exe");
        std::fs::write(&current, b"old build").unwrap();
        let staged = dir.join("updates").join("MapleLink.exe");
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, b"new build").unwrap();

        // Fails, and leaves the destination name occupied by something a rename
        // cannot overwrite — so the rollback fails for a reason of its own.
        fn refuse_and_block(_: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
            std::fs::create_dir_all(to).unwrap();
            std::fs::write(to.join("in the way"), b"x").unwrap();
            Err(std::io::Error::other("no"))
        }
        let err = swap_staged_exe_using(
            &staged,
            b"new build".len() as u64,
            &current,
            refuse_and_block,
        )
        .await
        .unwrap_err();

        let msg = err.to_string();
        assert!(msg.contains("could not be put back"), "got: {msg}");
        assert!(
            msg.contains("MapleLink.exe.old"),
            "the message must name where the old build is, got: {msg}"
        );
        // And it really is there, which is what the message promises.
        assert_eq!(
            std::fs::read(dir.join("MapleLink.exe.old")).unwrap(),
            b"old build"
        );
    }
}
