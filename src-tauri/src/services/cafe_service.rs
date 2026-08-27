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
    // Deliberately NOT announcement.json — it only records which announcement
    // has been read (no user data), and wiping it would force every café user
    // through the mandatory notice again.
    let files = [
        state.accounts_path.with_extension("dat"),
        state.accounts_path.with_extension("key"),
        state.overrides_path.with_extension("dat"),
        state.overrides_path.with_extension("key"),
        state.config_path.clone(),
    ];
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

    let script = webview_wipe_script(pid, &targets);

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

/// The PowerShell one-liner the wipe helper runs: wait for us to exit, let the
/// webview children follow, then delete each folder.
///
/// Paths are single-quoted, and any single quote inside them is doubled — the
/// literal-string escape. The tail of each path is a fixed identifier but the
/// head is the user profile, which can hold anything a Windows account name can,
/// apostrophes included. Unescaped, the script is a *parse* error: PowerShell
/// then runs none of it, not even the removes that would have been fine.
fn webview_wipe_script(pid: u32, targets: &[String]) -> String {
    let removes = targets
        .iter()
        .map(|t| {
            let quoted = t.replace('\'', "''");
            format!(
                "Remove-Item -LiteralPath '{quoted}' -Recurse -Force -ErrorAction SilentlyContinue"
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
         Start-Sleep -Milliseconds 800; {removes}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipe_script_quotes_each_target() {
        let script = webview_wipe_script(
            42,
            &[r"C:\Users\bob\AppData\Local\com.maplelink.app\EBWebView".to_string()],
        );
        assert!(script.contains("Wait-Process -Id 42"));
        assert!(script
            .contains(r"-LiteralPath 'C:\Users\bob\AppData\Local\com.maplelink.app\EBWebView'"));
    }

    #[test]
    fn wipe_script_escapes_an_apostrophe_in_the_profile_name() {
        let script = webview_wipe_script(1, &[r"C:\Users\O'Brien\AppData\Local\x".to_string()]);
        assert!(
            script.contains(r"'C:\Users\O''Brien\AppData\Local\x'"),
            "apostrophe not doubled: {script}"
        );
        // Every quote in the script must pair up, or PowerShell won't parse it.
        assert_eq!(script.matches('\'').count() % 2, 0, "unbalanced: {script}");
    }

    #[test]
    fn wipe_script_keeps_non_ascii_profile_names_verbatim() {
        // The command line reaches PowerShell as UTF-16, so CJK and full-width
        // punctuation need no special handling — but they must not be mangled.
        let path = r"C:\Users\简体（管理員）\AppData\Local\com.maplelink.app\EBWebView";
        let script = webview_wipe_script(1, &[path.to_string()]);
        assert!(script.contains(path), "path altered: {script}");
    }

    #[test]
    fn wipe_script_joins_multiple_targets() {
        let script = webview_wipe_script(1, &["a".to_string(), "b".to_string()]);
        assert_eq!(script.matches("Remove-Item").count(), 2);
        assert!(script.contains("SilentlyContinue; Remove-Item -LiteralPath 'b'"));
    }
}
