//! Commands backing the Beanfun browser window.
//!
//! Everything here is thin: the toolbar webview invokes, this layer resolves the
//! content webview and hands off to `services::browser_window` /
//! `services::webview_nav`.
//!
//! The native navigation calls block on a round trip to the main thread, so they
//! run on the blocking pool rather than tying up an async worker for what can be
//! several milliseconds each.

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::services::browser_window::{self, Bookmark};
use crate::services::webview_nav::{self, NavState};

/// The content webview, or an error when the browser is not open.
fn content(app: &tauri::AppHandle) -> Result<tauri::Webview, ErrorDto> {
    browser_window::content_view(app).ok_or_else(|| ErrorDto {
        code: "BROWSER_NOT_OPEN".to_string(),
        message: "Beanfun browser is not open".to_string(),
        category: ErrorCategory::Process,
        details: None,
    })
}

/// Run a blocking native navigation call off the async workers.
async fn off_thread<T, F>(f: F) -> Result<T, ErrorDto>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let joined = tauri::async_runtime::spawn_blocking(f).await;
    match joined {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(message)) => Err(ErrorDto {
            code: "BROWSER_NAV_FAILED".to_string(),
            message,
            category: ErrorCategory::Process,
            details: None,
        }),
        Err(e) => Err(ErrorDto {
            code: "BROWSER_NAV_FAILED".to_string(),
            message: format!("navigation task failed: {e}"),
            category: ErrorCategory::Process,
            details: None,
        }),
    }
}

/// Open the Beanfun browser for a session, optionally at a given URL.
#[tauri::command]
pub async fn open_beanfun_browser(
    session_id: String,
    url: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), ErrorDto> {
    browser_window::open(session_id, url, app, state.inner()).await
}

/// Navigate the content view. Called by the address bar.
#[tauri::command]
pub async fn browser_navigate(url: String, app: tauri::AppHandle) -> Result<(), ErrorDto> {
    let target = browser_window::sanitize_url(&url)?;
    let view = content(&app)?;
    off_thread(move || webview_nav::navigate(&view, &target)).await
}

#[tauri::command]
pub async fn browser_back(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    let view = content(&app)?;
    off_thread(move || webview_nav::go_back(&view)).await
}

#[tauri::command]
pub async fn browser_forward(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    let view = content(&app)?;
    off_thread(move || webview_nav::go_forward(&view)).await
}

#[tauri::command]
pub async fn browser_reload(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    let view = content(&app)?;
    off_thread(move || webview_nav::reload(&view)).await
}

/// The content view's current state, for a toolbar that has just loaded.
#[tauri::command]
pub async fn browser_state(app: tauri::AppHandle) -> Result<NavState, ErrorDto> {
    let view = content(&app)?;
    off_thread(move || webview_nav::nav_state(&view)).await
}

/// Reported by the toolbar once it has mounted, so a toolbar that never came up
/// is visible in the log rather than only on screen.
#[tauri::command]
pub async fn browser_toolbar_ready() -> Result<(), ErrorDto> {
    browser_window::toolbar_ready();
    Ok(())
}

/// The shortcut list for the session the browser was opened from.
#[tauri::command]
pub async fn browser_bookmarks(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Bookmark>, ErrorDto> {
    let Some(session_id) = browser_window::current_session() else {
        return Ok(Vec::new());
    };
    let ss = state.require_session(&session_id).await?;
    let region = crate::services::web_popup_service::session_region(&ss, state.inner()).await;
    let token =
        crate::services::web_popup_service::web_token_from_jar(&ss.cookie_jar, state.inner())
            .await?;
    Ok(browser_window::bookmarks(&region, &token))
}

/// Who answered for the page on screen, and with what certificate.
///
/// See `services::tls_info` for why this is a fresh handshake rather than a
/// readback of the connection the page arrived over.
#[tauri::command]
pub async fn browser_connection_info(
    app: tauri::AppHandle,
) -> Result<crate::services::tls_info::ConnectionInfo, ErrorDto> {
    let view = content(&app)?;
    let state = off_thread(move || webview_nav::nav_state(&view)).await?;
    let url: url::Url = state.url.parse().map_err(|_| ErrorDto {
        code: "BROWSER_INVALID_URL".to_string(),
        message: format!("The page has no address to inspect: {}", state.url),
        category: ErrorCategory::Process,
        details: None,
    })?;

    tauri::async_runtime::spawn_blocking(move || crate::services::tls_info::inspect(&url))
        .await
        .map_err(|e| ErrorDto {
            code: "BROWSER_TLS_FAILED".to_string(),
            message: format!("connection check failed: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })
}

/// Grow or shrink the toolbar so a panel it opens has somewhere to be drawn.
#[tauri::command]
pub async fn browser_set_chrome_height(height: f64, app: tauri::AppHandle) -> Result<(), ErrorDto> {
    browser_window::set_chrome_height(&app, height);
    Ok(())
}

/// Hand the current page to the user's own browser.
///
/// It arrives signed out — the session's cookies never leave this process — so
/// the toolbar says as much next to the button.
#[tauri::command]
pub async fn browser_open_external(url: String) -> Result<(), ErrorDto> {
    let target = browser_window::sanitize_url(&url)?;
    // Via the desktop shell so the user's browser does not inherit our elevated
    // token — see `utils::shell_open` for what that costs them otherwise.
    crate::utils::shell_open::open_external_url(&target).map_err(|e| ErrorDto {
        code: "BROWSER_EXTERNAL_FAILED".to_string(),
        message: format!("Failed to open {target}: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })
}

/// Close the browser, clearing the cookies it was seeded with.
#[tauri::command]
pub async fn close_beanfun_browser(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    browser_window::close(&app).await;
    Ok(())
}
