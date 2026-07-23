//! MapleStory Classic (懷舊服 / "mstc") login + launch.
//!
//! Classic runs on the Gamania "galaxy" login gateway, not the regular game's
//! LR/OTP path. The user authenticates through the normal beanfun login (which
//! leaves a `bfWebToken` in the session cookie jar), then a cookie-seeded webview
//! drives the galaxy SSO through to `maplestoryclassic.beanfun.com/Main`.
//!
//! From there the launch is fully automated: an injected script fetches the
//! one-time launch info (`/api/Login/GetOneTimeWebInfo`) and hands it to the
//! native side via `document.title` (the page's CSP usually blocks Tauri IPC, but
//! the title is always readable). The backend then builds the `ngm://` URL the
//! site would have used and hands it to the Nexon Game Manager via the shell —
//! no manual "start game" click, and no browser protocol prompt.

use tauri::Manager;

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::services::cookie_native;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

/// Galaxy classic (mstc) login entry. Issues a fresh OTT, stores it in the page's
/// localStorage and redirects to the init page (whose HK button we auto-click);
/// SSO via the seeded `bfWebToken` then flows through to the portal.
const CLASSIC_ENTRY_URL: &str = "https://galaxy.games.gamania.com/webapi/view/login/mstc?redirect_url=https://maplestoryclassic.beanfun.com/Main?af_click_id=";

/// Marker prefix the injected script writes to `document.title` once it has the
/// launch info, so the native poller can pick it up.
const LAUNCH_MARKER: &str = "NGMLAUNCH:";

/// Injected on every navigation. Two no-op-elsewhere behaviours:
/// 1. On the OTT init page — click the HK button to drive the beanfun SSO.
/// 2. On the classic portal Main page — POST the OTT to `GetOneTimeWebInfo` and
///    publish the returned launch info through `document.title`.
const INIT_SCRIPT: &str = r#"
(function () {
  function driveHk() {
    if (location.href.indexOf('/login/init/mstc/') === -1) return;
    var btn = document.querySelector('.btnLogin-beanfun');
    if (btn) { btn.click(); }
  }
  function fetchLaunch() {
    if (location.href.indexOf('maplestoryclassic.beanfun.com/Main') === -1) return;
    var ott = null;
    try { ott = localStorage.getItem('LOGIN_OTT_mstc'); } catch (e) {}
    if (!ott) ott = new URLSearchParams(location.search).get('OTT');
    if (!ott) return;
    fetch('/api/Login/GetOneTimeWebInfo', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ OTT: ott })
    }).then(function (r) { return r.json(); }).then(function (j) {
      if (j && j.code === 1 && j.data) {
        document.title = 'NGMLAUNCH:' + JSON.stringify(j.data);
      }
    }).catch(function () {});
  }
  function run() { driveHk(); fetchLaunch(); }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () { setTimeout(run, 200); });
  } else {
    setTimeout(run, 200);
  }
})();
"#;

/// The launch info returned by `GetOneTimeWebInfo` (its `data` object).
#[derive(Debug, serde::Deserialize)]
struct LaunchInfo {
    game: String,
    gid: String,
    #[serde(rename = "userSessionToken")]
    user_session_token: String,
    #[serde(rename = "userObjectID")]
    user_object_id: i64,
    #[serde(rename = "galaxy_GameId")]
    galaxy_game_id: i64,
}

/// Build the `ngm://` launch URL exactly as the classic portal does: the argument
/// string after `ngm://launch/` is `encodeURIComponent`-encoded (single quotes
/// kept literal, which `encodeURIComponent` also does).
fn build_ngm_url(info: &LaunchInfo, timestamp_ms: i64) -> String {
    let passarg = format!(
        "{} {} {} {}",
        info.user_object_id, info.user_session_token, info.gid, info.galaxy_game_id
    );
    let args = format!(
        " -mode:launch -game:'{}' -passarg:'{}' -position:'GameWeb|https://maplestoryclassic.beanfun.com/Main?af_click_id=' -architectureplatform:'none' -timestamp:{}",
        info.game, passarg, timestamp_ms
    );
    let encoded = urlencoding::encode(&args).replace("%27", "'");
    format!("ngm://launch/{encoded}")
}

/// Launch the `ngm://` URL via the desktop shell so Nexon Game Manager starts
/// unelevated (MapleLink runs elevated; a game inheriting that token is best
/// avoided) and without a browser protocol prompt.
#[cfg(target_os = "windows")]
fn launch_ngm(url: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("explorer.exe")
        .arg(url)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to launch ngm via explorer: {e}"))
}

#[cfg(not(target_os = "windows"))]
fn launch_ngm(_url: &str) -> Result<(), String> {
    Err("ngm launch is only supported on Windows".to_string())
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
    .initialization_script(INIT_SCRIPT)
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

    let nav_rx = cookie_native::on_navigation_completed(&win).ok();
    let _ = win.eval(format!("window.location.href = '{CLASSIC_ENTRY_URL}';"));

    // Show the portal while it works (SSO + Main), then poll the title for the
    // launch info the injected script publishes. On success, launch the game and
    // close the portal; on timeout leave it open so the user can act manually.
    tauri::async_runtime::spawn(async move {
        if let Some(rx) = nav_rx {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(8), rx).await;
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
        let _ = win.show();
        let _ = win.set_focus();
        tracing::info!("classic portal opened, waiting for launch info");

        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let Ok(title) = win.title() else { return }; // window gone
            let Some(json) = title.strip_prefix(LAUNCH_MARKER) else {
                continue;
            };
            match serde_json::from_str::<LaunchInfo>(json) {
                Ok(info) => {
                    let ts = chrono::Utc::now().timestamp_millis();
                    let url = build_ngm_url(&info, ts);
                    tracing::info!("classic: launching game via ngm ({} bytes)", url.len());
                    if let Err(e) = launch_ngm(&url) {
                        tracing::warn!("classic: ngm launch failed: {e}");
                    }
                    let _ = win.destroy();
                    return;
                }
                Err(e) => tracing::warn!("classic: could not parse launch info: {e}"),
            }
        }
        tracing::warn!("classic: no launch info within timeout — leaving portal open");
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_ngm_url_like_the_portal() {
        let info = LaunchInfo {
            game: "2982@2141".into(),
            gid: "2373".into(),
            user_session_token: "sessb9c9eb4345e36b1f8b4d4e3e86fb5506".into(),
            user_object_id: 4571368,
            galaxy_game_id: 944,
        };
        let url = build_ngm_url(&info, 1784824258582);
        // Matches the captured browser URL: percent-encoded, single quotes kept.
        assert_eq!(
            url,
            "ngm://launch/%20-mode%3Alaunch%20-game%3A'2982%402141'%20-passarg%3A'4571368%20sessb9c9eb4345e36b1f8b4d4e3e86fb5506%202373%20944'%20-position%3A'GameWeb%7Chttps%3A%2F%2Fmaplestoryclassic.beanfun.com%2FMain%3Faf_click_id%3D'%20-architectureplatform%3A'none'%20-timestamp%3A1784824258582"
        );
    }
}
