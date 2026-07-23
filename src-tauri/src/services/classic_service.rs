//! MapleStory Classic (懷舊服 / "mstc") login + launch.
//!
//! Classic runs on the Gamania "galaxy" login gateway, not the regular game's
//! LR/OTP path. The user authenticates through the normal beanfun login (which
//! leaves a `bfWebToken` in the session cookie jar), then a cookie-seeded webview
//! drives the galaxy SSO through to `maplestoryclassic.beanfun.com/Main`.
//!
//! The Main page auto-fires its own `ngm://` launch, which WebView2 would show a
//! "open Nexon Game Manager" prompt for. We intercept that at the WebView2 layer
//! (`LaunchingExternalUriScheme`), cancel the prompt, and start Nexon Game
//! Manager ourselves from its registered handler — so the whole thing runs in a
//! hidden window with no manual click. If interception isn't available (old
//! runtime) or NGM isn't installed, the portal is revealed for a manual launch.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::services::cookie_native;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

/// Galaxy classic (mstc) login entry. Issues a fresh OTT, stores it in the page's
/// localStorage and redirects to the init page (whose HK button we auto-click);
/// SSO via the seeded `bfWebToken` then flows through to the portal, which fires
/// its own `ngm://` launch on arrival.
const CLASSIC_ENTRY_URL: &str = "https://galaxy.games.gamania.com/webapi/view/login/mstc?redirect_url=https://maplestoryclassic.beanfun.com/Main?af_click_id=";

/// Injected on every navigation. On the OTT init page it clicks the HK button to
/// drive the beanfun SSO; a no-op everywhere else.
const AUTO_HK_SCRIPT: &str = r#"
(function () {
  function driveHk() {
    if (location.href.indexOf('/login/init/mstc/') === -1) return;
    var btn = document.querySelector('.btnLogin-beanfun');
    if (btn) { btn.click(); }
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () { setTimeout(driveHk, 200); });
  } else {
    setTimeout(driveHk, 200);
  }
})();
"#;

// Launch state shared between the intercept callback and the poll task.
const PENDING: u8 = 0;
const LAUNCHED: u8 = 1;
const FAILED: u8 = 2;

/// Start Nexon Game Manager for a captured `ngm://` URL by invoking its
/// registered handler directly (`HKCR\ngm\shell\open\command`).
///
/// Deliberately no shell fallback: we're called from inside the intercept that
/// just cancelled WebView2's prompt, and handing the URL to the shell would only
/// pop the prompt straight back. If NGM isn't registered this fails, and the
/// caller reveals the portal so the user can install / launch it by hand.
#[cfg(target_os = "windows")]
fn launch_ngm(url: &str) -> Result<(), String> {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    let command: String = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"ngm\shell\open\command")
        .and_then(|k| k.get_value(""))
        .map_err(|e| {
            format!("ngm handler not registered (is Nexon Game Manager installed?): {e}")
        })?;

    let (exe, args) = parse_handler_command(&command, url)
        .ok_or_else(|| format!("could not parse ngm handler command: {command}"))?;

    std::process::Command::new(&exe)
        .args(&args)
        .spawn()
        .map_err(|e| format!("failed to launch NGM ({exe}): {e}"))?;
    tracing::info!("classic: launched NGM directly ({exe})");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_ngm(_url: &str) -> Result<(), String> {
    Err("ngm launch is only supported on Windows".to_string())
}

/// Parse a registered protocol handler command (`"exe" "%1"` / `exe %1`) into the
/// executable and its arguments, substituting the URL for every `%1`.
#[cfg(target_os = "windows")]
fn parse_handler_command(command: &str, url: &str) -> Option<(String, Vec<String>)> {
    let command = command.trim();
    let (exe, rest) = if let Some(after) = command.strip_prefix('"') {
        let end = after.find('"')?;
        (after[..end].to_string(), &after[end + 1..])
    } else {
        let end = command.find(' ').unwrap_or(command.len());
        (command[..end].to_string(), &command[end..])
    };
    if exe.is_empty() {
        return None;
    }
    let args = rest
        .split_whitespace()
        .map(|a| a.trim_matches('"').replace("%1", url))
        .collect::<Vec<_>>();
    // If the handler declares no %1 slot, pass the URL as a trailing argument.
    let args = if args.iter().any(|a| a.contains(url)) {
        args
    } else {
        vec![url.to_string()]
    };
    Some((exe, args))
}

/// Result of the classic-readiness self-check, shown so users can tell whether
/// the pieces the launch relies on are present.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassicCheck {
    /// Nexon Game Manager's `ngm://` protocol handler is registered.
    pub ngm_registered: bool,
    /// The handler's executable path, if we could read it.
    pub ngm_exe: Option<String>,
    /// That executable actually exists on disk.
    pub ngm_exe_exists: bool,
    /// Installed WebView2 runtime version (drives whether the launch prompt can
    /// be auto-suppressed), if detectable.
    pub webview2_version: Option<String>,
}

/// Check that the local prerequisites for the classic launch are in place.
#[cfg(target_os = "windows")]
pub fn self_check() -> ClassicCheck {
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let mut check = ClassicCheck::default();

    if let Ok(command) = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"ngm\shell\open\command")
        .and_then(|k| k.get_value::<String, _>(""))
    {
        check.ngm_registered = true;
        if let Some((exe, _)) = parse_handler_command(&command, "") {
            check.ngm_exe_exists = std::path::Path::new(&exe).exists();
            check.ngm_exe = Some(exe);
        }
    }

    // WebView2 Evergreen Runtime version, machine-wide then per-user.
    const WV2: &str =
        r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const WV2_USER: &str =
        r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    check.webview2_version = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(WV2)
        .and_then(|k| k.get_value::<String, _>("pv"))
        .or_else(|_| {
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(WV2_USER)
                .and_then(|k| k.get_value::<String, _>("pv"))
        })
        .ok();

    check
}

#[cfg(not(target_os = "windows"))]
pub fn self_check() -> ClassicCheck {
    ClassicCheck::default()
}

/// Open the classic portal for an already-authenticated session and auto-launch
/// the game once it lands, reusing the session's cookies so no re-login is needed.
pub async fn open_classic_login(
    session_id: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let ss = state.require_session(&session_id).await?;
    let label = "classic-login";

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    // Seed the session's beanfun cookies so the HK SSO step skips re-login.
    let seed_cookies = cookie_native::cookies_from_jar(
        &ss.cookie_jar,
        &[
            "https://bfweb.hk.beanfun.com/",
            "https://login.hk.beanfun.com/",
            "https://beanfun.com/",
            "https://login.beanfun.com/",
            "https://tw.beanfun.com/",
            "https://tw.newlogin.beanfun.com/",
        ],
    );

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    let win = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("新楓之谷：經典版")
    .inner_size(1024.0, 720.0)
    .min_inner_size(400.0, 300.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(false)
    .data_directory(data_dir)
    .user_agent(WEBVIEW_USER_AGENT)
    .initialization_script(AUTO_HK_SCRIPT)
    .build()
    .map_err(|e| ErrorDto {
        code: "SYS_POPUP_FAILED".to_string(),
        message: format!("Failed to open classic portal: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    if let Err(e) = cookie_native::register_new_window_handler(&win) {
        tracing::warn!("classic portal: NewWindowRequested handler failed: {e}");
    }
    if let Err(e) = cookie_native::seed_cookies_native(&win, &seed_cookies) {
        tracing::warn!("classic portal: native cookie seeding failed: {e}");
    }

    // Intercept the portal's own ngm:// launch: cancel WebView2's prompt and start
    // NGM ourselves. The flag lets the poll task react (close on success, reveal
    // for manual launch on failure).
    let flag = Arc::new(AtomicU8::new(PENDING));
    let flag_cb = flag.clone();
    let intercept_ok = cookie_native::register_external_uri_handler(&win, move |url| {
        if !(url.starts_with("ngm:") || url.starts_with("nexonplug:")) {
            return;
        }
        let outcome = match launch_ngm(url) {
            Ok(()) => LAUNCHED,
            Err(e) => {
                tracing::warn!("classic: ngm launch failed: {e}");
                FAILED
            }
        };
        flag_cb.store(outcome, Ordering::SeqCst);
    })
    .inspect_err(|e| tracing::warn!("classic: external-uri interception unavailable: {e}"))
    .is_ok();

    let _ = win.eval(format!("window.location.href = '{CLASSIC_ENTRY_URL}';"));

    // Without interception the prompt can't be suppressed — reveal the window so
    // the user can complete the launch by hand.
    if !intercept_ok {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    // Hidden auto-launch: wait for the intercept to fire, then close (success) or
    // reveal for manual completion (failure / timeout).
    tauri::async_runtime::spawn(async move {
        tracing::info!("classic portal running (hidden), waiting for launch");
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if win.title().is_err() {
                return; // window gone
            }
            match flag.load(Ordering::SeqCst) {
                LAUNCHED => {
                    let _ = win.app_handle().emit("classic-launched", ());
                    let _ = win.destroy();
                    return;
                }
                FAILED => {
                    let _ = win.app_handle().emit("classic-launch-failed", ());
                    let _ = win.show();
                    let _ = win.set_focus();
                    return;
                }
                _ => {}
            }
        }
        tracing::warn!("classic: no launch within timeout — revealing portal");
        let _ = win.app_handle().emit("classic-launch-timeout", ());
        let _ = win.show();
        let _ = win.set_focus();
    });

    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_and_bare_handler_commands() {
        let (exe, args) = parse_handler_command(r#""C:\NGM\ngm.exe" "%1""#, "ngm://x").unwrap();
        assert_eq!(exe, r"C:\NGM\ngm.exe");
        assert_eq!(args, vec!["ngm://x".to_string()]);

        // No %1 slot → the URL is appended as a trailing argument.
        let (exe, args) = parse_handler_command(r#""C:\NGM\ngm.exe""#, "ngm://y").unwrap();
        assert_eq!(exe, r"C:\NGM\ngm.exe");
        assert_eq!(args, vec!["ngm://y".to_string()]);
    }
}
