//! MapleStory Classic (懷舊服 / "mstc") login portal.
//!
//! Classic runs on the Gamania "galaxy" login gateway, not the regular game's
//! LR/OTP path. The user authenticates through the normal beanfun login (which
//! leaves a `bfWebToken` in the session cookie jar), then a cookie-seeded webview
//! is pointed at the galaxy classic entry URL. Beanfun SSO carries it through to
//! `maplestoryclassic.beanfun.com/Main`, where the game itself is started by the
//! site's own `ngm://` protocol handler (a separately-installed client).

use tauri::Manager;

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::services::cookie_native;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

/// Galaxy classic (mstc) login entry. It issues a fresh OTT, stores it in the
/// page's localStorage and redirects to the init page (whose HK button we
/// auto-click); SSO via the seeded `bfWebToken` then flows through to the portal.
const CLASSIC_ENTRY_URL: &str = "https://galaxy.games.gamania.com/webapi/view/login/mstc?redirect_url=https://maplestoryclassic.beanfun.com/Main?af_click_id=";

/// Auto-click the HK ("gamania (HK)") button on the OTT init page. It's a plain
/// `<a href>` to beanfun's `default.aspx`, so clicking it navigates straight into
/// the SSO chain. The page stores its OTT in localStorage as it loads, so we wait
/// for the DOM before clicking. Runs on every navigation but no-ops anywhere
/// except the init page.
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

/// Open the classic portal in a cookie-seeded webview, reusing the caller's
/// already-authenticated session so the SSO step needs no re-login.
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

    let nav_rx = cookie_native::on_navigation_completed(&win).ok();
    let _ = win.eval(format!("window.location.href = '{CLASSIC_ENTRY_URL}';"));

    let win_clone = win.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(rx) = nav_rx {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(8), rx).await;
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let _ = win_clone.show();
        let _ = win_clone.set_focus();
        tracing::info!("classic portal opened");
    });

    Ok(())
}
