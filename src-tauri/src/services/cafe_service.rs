//! Café / shared-PC mode: wipe all local user data when the app closes.
//!
//! When `cafe_mode` is on, closing MapleLink erases everything it stored on the
//! machine so the next person starts from nothing: saved credentials, display
//! overrides, config, logs, and the WebView2 login session.
//!
//! Most of that can be deleted immediately — the files aren't locked while we
//! run. The WebView2 session folder (`EBWebView`) is the exception: it stays
//! locked until our process and its webview child processes have exited, so it's
//! handed to a small detached helper that waits for us to quit and removes it
//! afterwards.

use crate::models::app_state::AppState;

/// Erase all local user data for café mode. Safe to call on the window-close
/// path — the immediate deletions are a handful of small files, and the locked
/// WebView2 folder is deferred to a detached process.
pub fn wipe_local_data(app: &tauri::AppHandle, state: &AppState) {
    use tauri::Manager;

    // Credential store + display overrides (DPAPI .dat/.key pairs) and config.
    let mut files = vec![
        state.accounts_path.with_extension("dat"),
        state.accounts_path.with_extension("key"),
        state.overrides_path.with_extension("dat"),
        state.overrides_path.with_extension("key"),
        state.config_path.clone(),
    ];
    // The announcement-seen marker lives beside config in the app data dir.
    if let Ok(dir) = app.path().app_data_dir() {
        files.push(dir.join("announcement.json"));
    }
    for f in &files {
        match std::fs::remove_file(f) {
            Ok(()) => tracing::info!("cafe wipe: removed {}", f.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!("cafe wipe: could not remove {}: {e}", f.display()),
        }
    }

    // Logs can contain account ids / OTPs, so they go too.
    if let Ok(log_dir) = app.path().app_log_dir() {
        if let Err(e) = std::fs::remove_dir_all(&log_dir) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    "cafe wipe: could not remove logs {}: {e}",
                    log_dir.display()
                );
            }
        }
    }

    // WebView2 session — locked until we exit, so defer it.
    schedule_webview_wipe();
}

/// Spawn a detached helper that waits for THIS process to exit, then deletes the
/// WebView2 session folder(s). Matches the locations the startup cleanup uses.
#[cfg(target_os = "windows")]
fn schedule_webview_wipe() {
    use std::os::windows::process::CommandExt;

    let pid = std::process::id();
    let mut targets = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        targets.push(format!("{local}\\com.maplelink.app\\EBWebView"));
    }
    if let Ok(roaming) = std::env::var("APPDATA") {
        targets.push(format!("{roaming}\\com.maplelink.app\\EBWebView"));
    }
    if targets.is_empty() {
        return;
    }

    // Single-quote the paths for PowerShell; a data-dir path can't contain a
    // single quote (it's under a fixed identifier), so no escaping is needed.
    let removes = targets
        .iter()
        .map(|t| {
            format!("Remove-Item -LiteralPath '{t}' -Recurse -Force -ErrorAction SilentlyContinue")
        })
        .collect::<Vec<_>>()
        .join("; ");
    // Wait for us (and the webview children that exit with us) to release the
    // lock, then remove the folders. The brief sleep covers child teardown.
    let script =
        format!("Wait-Process -Id {pid} -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 800; {removes}");

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    match std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
    {
        Ok(child) => tracing::info!(
            "cafe wipe: scheduled webview cleanup (helper pid={})",
            child.id()
        ),
        Err(e) => tracing::warn!("cafe wipe: could not schedule webview cleanup: {e}"),
    }
}

#[cfg(not(target_os = "windows"))]
fn schedule_webview_wipe() {}
