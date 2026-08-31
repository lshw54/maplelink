//! The Beanfun browser — one window holding a toolbar webview above a content
//! webview, both children of the same `Window` (Tauri's `unstable` multiwebview
//! API).
//!
//! ## Why this exists
//!
//! Logging in once used to mean the official site was logged in too, so a player
//! could walk from the launcher into an event page without signing in again.
//! The bare popups we opened for the member centre broke that: they have no
//! address bar, so the only way out of the page they land on was the browser's
//! developer console — which is exactly what people started using, and exactly
//! what we do not want to ship.
//!
//! A toolbar gives back the walk without giving back the console.
//!
//! ## Why a child webview and not an injected bar
//!
//! An injected toolbar lives inside the page: it covers content, fights the
//! site's own fixed elements, and can only guess at history state. A child
//! webview is real chrome — it owns its strip of the window, and the arrows know
//! whether they can go anywhere because WebView2 is asked directly (see
//! `services::webview_nav`).
//!
//! ## Where this window will go
//!
//! beanfun and gamania, and nowhere else. Anything further out — typed, clicked
//! or redirected to — is handed to the user's own browser instead.
//!
//! Cookies alone would not have needed that: they are scoped by domain, so the
//! session never reaches a third party. The renderer is the reason. Every
//! WebView2 this app opens runs with `--no-sandbox`, and the app self-elevates,
//! so a renderer here is an unsandboxed one holding an administrator token. That
//! was a contained risk while these windows only ever showed beanfun. An address
//! bar would have turned it into "any site the user can be talked into typing",
//! which is a different thing entirely.
//!
//! Downloads are refused for the same reason: anything this process launches
//! inherits its token, and this window exists for reading event pages.

use std::sync::Mutex;

use tauri::{LogicalPosition, LogicalSize, Manager};

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::models::session::Region;
use crate::services::web_popup_service::{
    auth_url, member_landing, region_web_host, web_token_from_jar,
};
use crate::services::webview_util::WEBVIEW_USER_AGENT;
use crate::services::{cookie_native, webview_nav};

/// The window, and the two webviews inside it.
pub const WINDOW: &str = "bf-browser";
pub const BAR: &str = "bf-browser-bar";
pub const VIEW: &str = "bf-browser-view";

/// Toolbar height in logical pixels. Mirrored by `BAR_HEIGHT` in the toolbar
/// page; change both together or the strip and its contents disagree.
const BAR_HEIGHT: f64 = 46.0;

/// How much of the window the toolbar currently occupies.
///
/// Normally `BAR_HEIGHT`. The padlock panel raises it while it is open, because
/// this webview cannot paint outside its own bounds — a panel hanging below the
/// bar is drawn where there are no pixels. Growing the strip and pushing the
/// page down is not how a real browser overlays a panel, but it needs no
/// fighting over the z-order of two sibling child windows, and it cannot end up
/// invisible.
static CHROME_HEIGHT: Mutex<f64> = Mutex::new(BAR_HEIGHT);

/// The toolbar's current height, clamped to something the window can hold.
fn chrome_height() -> f64 {
    CHROME_HEIGHT.lock().map(|h| *h).unwrap_or(BAR_HEIGHT)
}

/// Set the toolbar's height and restack. Called when the padlock panel opens
/// and closes.
pub fn set_chrome_height(app: &tauri::AppHandle, height: f64) {
    if let Ok(mut current) = CHROME_HEIGHT.lock() {
        *current = height.max(BAR_HEIGHT);
    }
    if let Some(window) = app.get_window(WINDOW) {
        relayout(&window);
    }
}

/// Event carrying the content view's state to the toolbar.
const NAV_EVENT: &str = "browser:nav";

/// Event carrying a translation key for something the toolbar should say.
const NOTICE_EVENT: &str = "browser:notice";

/// The session the open window was seeded from.
///
/// A browser holding one account's cookies must not be reused for another, so
/// `open` tears the window down when the session it is asked for differs.
static SESSION: Mutex<Option<String>> = Mutex::new(None);

/// Set when the toolbar reports for duty.
///
/// `Window::add_child` hands back a handle before the webview actually exists,
/// so a creation failure reaches us as nothing at all. This is how we notice.
static TOOLBAR_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Called by the toolbar once it has mounted.
pub fn toolbar_ready() {
    TOOLBAR_READY.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Origins we seed session cookies into.
///
/// The jar's cookies land on `.beanfun.com`, which already covers every event
/// subdomain the launcher is asked to reach. The region host is listed so its
/// host-only cookies come along too.
fn seed_hosts(host: &str) -> Vec<String> {
    vec![
        format!("https://{host}/"),
        "https://beanfun.com/".to_string(),
        "https://m.beanfun.com/".to_string(),
        "https://event.beanfun.com/".to_string(),
        "https://login.beanfun.com/".to_string(),
    ]
}

/// What the toolbar draws: where the content view is, and where it can go.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NavPayload {
    url: String,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
    loading: bool,
    /// True for the first, cheap emit of a navigation, whose history flags are
    /// not filled in yet. The toolbar takes only the URL from those, so its
    /// arrows do not blink off and on again a moment later.
    partial: bool,
}

/// A toolbar shortcut. `key` is a translation key, resolved by the toolbar.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub key: String,
    pub url: String,
}

/// Strip `web_token` from a URL before it is shown or handed anywhere.
///
/// The `auth.aspx` hop carries the session's `bfWebToken` in its query string.
/// The old popups had no address bar, so it stayed out of sight; this one would
/// put it on screen, one screenshot away from being someone else's session. The
/// token has done its job by the time the page loads — beanfun answers on
/// cookies from then on — so the bar can lose it without losing the page.
fn redact(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    if !parsed.query_pairs().any(|(k, _)| k == "web_token") {
        return url.to_string();
    }

    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| k != "web_token")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    let mut out = parsed.clone();
    match kept.is_empty() {
        true => out.set_query(None),
        false => {
            out.query_pairs_mut().clear().extend_pairs(kept);
        }
    }
    out.to_string()
}

/// Domains this window navigates to itself. Everything else goes to the user's
/// own browser — see the module docs for why the line is drawn here.
const IN_SCOPE: [&str; 2] = ["beanfun.com", "gamania.com"];

/// Whether the window may follow `url` itself.
pub fn in_scope(url: &url::Url) -> bool {
    // The window is built on about:blank before its first real navigation.
    if url.scheme() == "about" {
        return true;
    }
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    url.host_str().is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        IN_SCOPE
            .iter()
            // The dot matters: without it `notbeanfun.com` is in scope.
            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    })
}

/// Hand a URL to the user's own browser and tell the toolbar it happened.
///
/// Silence here would read as the window being broken — the user clicks, and
/// nothing anywhere says why nothing moved.
fn hand_off(app: &tauri::AppHandle, url: &str, notice: &str) {
    use tauri::Emitter;

    // Through the desktop shell, never `open::that`: this process is elevated,
    // and anything it spawns inherits that token — a browser started this way
    // writes its profile as Administrator and cannot read it back afterwards.
    // See `utils::shell_open`.
    if let Err(e) = crate::utils::shell_open::open_external_url(url) {
        tracing::warn!("browser: could not hand {url} to the system browser: {e}");
    }
    let _ = app.emit_to(BAR, NOTICE_EVENT, notice);
}

/// Reject anything an address bar should not accept.
///
/// `javascript:` is the one that matters — typed into a bar it runs against
/// whatever origin is loaded, which is the console we are declining to ship.
/// `file:` and `data:` are refused on the same grounds.
pub fn sanitize_url(input: &str) -> Result<String, ErrorDto> {
    let trimmed = input.trim();
    let candidate = match trimmed.contains("://") {
        true => trimmed.to_string(),
        // A bare host is what people actually type.
        false => format!("https://{trimmed}"),
    };

    let parsed: url::Url = candidate.parse().map_err(|_| ErrorDto {
        code: "BROWSER_INVALID_URL".to_string(),
        message: format!("Not a URL: {trimmed}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ErrorDto {
            code: "BROWSER_BAD_SCHEME".to_string(),
            message: format!("Unsupported scheme: {}", parsed.scheme()),
            category: ErrorCategory::Process,
            details: None,
        });
    }

    Ok(parsed.to_string())
}

/// The shortcuts offered for a region.
///
/// Every entry is a URL the launcher already relies on elsewhere — this is not
/// the place to guess at addresses that may not exist.
pub fn bookmarks(region: &Region, token: &str) -> Vec<Bookmark> {
    let (host, region_path) = region_web_host(region);
    let mark = |key: &str, url: String| Bookmark {
        key: key.to_string(),
        url,
    };

    // No top-up here on purpose: paying leaves for a payment provider, which
    // this window will not follow. It keeps its own dedicated popup — see
    // `web_popup_service::open_gash_popup` — which is not scope-limited.
    let mut out = vec![mark(
        "browser.link.member",
        auth_url(host, region_path, "member", member_landing(region), token),
    )];

    match region {
        Region::TW => {
            out.push(mark(
                "browser.link.maplestory",
                "https://maplestory.beanfun.com/".to_string(),
            ));
            out.push(mark(
                "browser.link.exchange",
                "https://m.beanfun.com/Deposite".to_string(),
            ));
            out.push(mark(
                "browser.link.support",
                "https://tw.beanfun.com/customerservice/www/main.aspx".to_string(),
            ));
        }
        Region::HK => {
            out.push(mark(
                "browser.link.home",
                "https://bfweb.hk.beanfun.com/".to_string(),
            ));
            out.push(mark(
                "browser.link.support",
                "https://bfweb.hk.beanfun.com/newfaq/service_newBF.aspx".to_string(),
            ));
        }
    }

    out
}

/// The content webview, if the browser is open.
pub fn content_view(app: &tauri::AppHandle) -> Option<tauri::Webview> {
    app.get_webview(VIEW)
}

/// The session the open browser was seeded from, if any.
pub fn current_session() -> Option<String> {
    SESSION.lock().ok().and_then(|s| s.clone())
}

/// Send the content view's current state to the toolbar.
///
/// The URL and loading flag go out immediately — they are already in hand, and
/// the address bar should not lag the page. History availability has to be asked
/// of WebView2, which blocks, so it follows on a blocking-pool task.
fn push_nav(app: &tauri::AppHandle, webview: &tauri::Webview, url: String, loading: bool) {
    use tauri::Emitter;

    let _ = app.emit_to(
        BAR,
        NAV_EVENT,
        NavPayload {
            url: redact(&url),
            title: String::new(),
            can_go_back: false,
            can_go_forward: false,
            loading,
            partial: true,
        },
    );

    let app = app.clone();
    let webview = webview.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Ok(state) = webview_nav::nav_state(&webview) else {
            return;
        };
        let _ = app.emit_to(
            BAR,
            NAV_EVENT,
            NavPayload {
                url: redact(&state.url),
                title: state.title,
                can_go_back: state.can_go_back,
                can_go_forward: state.can_go_forward,
                loading,
                partial: false,
            },
        );
    });
}

/// Open the browser for `session_id`, landing on `target` (member centre when
/// none is given).
pub async fn open(
    session_id: String,
    target: Option<String>,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), ErrorDto> {
    let ss = state.require_session(&session_id).await?;
    let region = crate::services::web_popup_service::session_region(&ss, state).await;
    let (host, region_path) = region_web_host(&region);
    let token = web_token_from_jar(&ss.cookie_jar, state).await?;

    let landing = match target {
        Some(raw) => sanitize_url(&raw)?,
        None => auth_url(host, region_path, "member", member_landing(&region), &token),
    };

    // An open window already holds one account's cookies. Reuse it only for the
    // account it was seeded from; anything else gets a fresh one.
    let reusable = SESSION
        .lock()
        .map(|s| s.as_deref() == Some(session_id.as_str()))
        .unwrap_or(false);

    if let Some(window) = app.get_window(WINDOW) {
        if reusable {
            if let Some(view) = content_view(&app) {
                let _ = webview_nav::navigate(&view, &landing);
            }
            let _ = window.show();
            let _ = window.set_focus();
            return Ok(());
        }
        let _ = window.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;
    let browser_args = crate::services::webview_util::browser_args(&app).await;

    // Both set before the toolbar can exist. It loads in about 200ms and asks
    // for its shortcut list straight away, so anything recorded after the
    // webviews are built is recorded too late: the check-in gets wiped, and the
    // shortcut list is looked up against a session we have not stored yet.
    TOOLBAR_READY.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut held) = SESSION.lock() {
        *held = Some(session_id.clone());
    }
    if let Ok(mut height) = CHROME_HEIGHT.lock() {
        *height = BAR_HEIGHT;
    }

    let width = 1100.0;
    let height = 780.0;

    let window = tauri::window::WindowBuilder::new(&app, WINDOW)
        .title("Beanfun")
        .inner_size(width, height)
        .min_inner_size(680.0, 420.0)
        .resizable(true)
        .center()
        .visible(false)
        .build()
        .map_err(|e| ErrorDto {
            code: "BROWSER_WINDOW_FAILED".to_string(),
            message: format!("Failed to create browser window: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })?;

    // A profile of its own, like the debug console has. Two WebView2 instances
    // in one process cannot share a user data folder, and the toolbar would
    // otherwise land on the main window's — which fails with 0x8007139F, and
    // fails *silently*: `add_child` reports success and only the runtime log
    // says the webview was never created. Hence the watchdog below.
    let bar_data_dir = data_dir.join("browser-bar");

    window
        .add_child(
            tauri::webview::WebviewBuilder::new(BAR, tauri::WebviewUrl::App("browser.html".into()))
                .data_directory(bar_data_dir),
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(width, BAR_HEIGHT),
        )
        .map_err(|e| ErrorDto {
            code: "BROWSER_WINDOW_FAILED".to_string(),
            message: format!("Failed to create browser toolbar: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })?;

    let app_for_load = app.clone();
    let view = window
        .add_child(
            tauri::webview::WebviewBuilder::new(
                VIEW,
                tauri::WebviewUrl::External("about:blank".parse().unwrap()),
            )
            .user_agent(WEBVIEW_USER_AGENT)
            .data_directory(data_dir)
            .additional_browser_args(&browser_args)
            .initialization_script(crate::services::web_popup_service::KEEP_LINKS_IN_WINDOW)
            // The toolbar is the answer to what people were opening DevTools for,
            // so the console stays shut.
            .devtools(false)
            // Covers every way out of beanfun: a clicked link, a redirect, and
            // the `NewWindowRequested` handler's own `Navigate` call, which
            // WebView2 reports here like any other navigation.
            .on_navigation({
                let app = app.clone();
                move |url| {
                    if in_scope(url) {
                        return true;
                    }
                    tracing::info!("browser: {url} is out of scope; handing it over");
                    hand_off(&app, url.as_str(), "browser.notice.opened_externally");
                    false
                }
            })
            // This process runs elevated, so whatever it saves is written — and
            // whatever the user then opens is launched — with that token.
            .on_download({
                let app = app.clone();
                move |_, event| {
                    if let tauri::webview::DownloadEvent::Requested { url, .. } = event {
                        tracing::info!("browser: refused a download of {url}");
                        hand_off(&app, url.as_str(), "browser.notice.download_blocked");
                    }
                    false
                }
            })
            .on_page_load(move |webview, payload| {
                let loading = matches!(payload.event(), tauri::webview::PageLoadEvent::Started);
                push_nav(&app_for_load, &webview, payload.url().to_string(), loading);
            }),
            LogicalPosition::new(0.0, BAR_HEIGHT),
            LogicalSize::new(width, height - BAR_HEIGHT),
        )
        .map_err(|e| ErrorDto {
            code: "BROWSER_WINDOW_FAILED".to_string(),
            message: format!("Failed to create browser content view: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })?;

    // If the toolbar never checks in, the window is a bare popup again and the
    // user is stuck on whatever page it landed on — worth saying so out loud,
    // since nothing else will.
    let app_for_watchdog = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        if TOOLBAR_READY.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        // Ask the webview what it is showing, since `add_child` reports success
        // for a webview that was never created: a URL means the page is there
        // and only the check-in went missing, an error means there is no webview
        // under the label at all.
        let found = match app_for_watchdog.get_webview(BAR) {
            Some(bar) => tauri::async_runtime::spawn_blocking(move || webview_nav::nav_state(&bar))
                .await
                .unwrap_or_else(|e| Err(format!("join failed: {e}"))),
            None => Err("no webview registered under this label".to_string()),
        };
        tracing::error!("browser: toolbar never checked in; webview reports {found:?}");
    });

    // Keep the two webviews stacked as the window changes size. `auto_resize`
    // would have each of them fill the window, which is not the layout.
    let layout_window = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(_) = event {
            relayout(&layout_window);
        }
    });

    if let Err(e) = cookie_native::register_new_window_handler(&view) {
        tracing::warn!("browser: NewWindowRequested handler failed: {e}");
    }
    let seed = cookie_native::cookies_from_jar(
        &ss.cookie_jar,
        &seed_hosts(host)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    if let Err(e) = cookie_native::seed_cookies_native(&view, &seed) {
        tracing::warn!("browser: native cookie seeding failed: {e}");
    }

    // beanfun only honours the seeded token once `auth.aspx` has turned it into
    // a server-side session, so that hop comes first even when the user asked
    // for somewhere else. Landing straight on an event page without it shows the
    // logged-out version — the very thing this window exists to avoid.
    let bootstrap = auth_url(host, region_path, "member", member_landing(&region), &token);
    let needs_bootstrap = landing != bootstrap;
    let nav_rx = cookie_native::on_navigation_completed(&view).ok();
    let _ = webview_nav::navigate(&view, &bootstrap);

    let window_for_show = window.clone();
    let view_for_show = view.clone();
    tauri::async_runtime::spawn(async move {
        match nav_rx {
            Some(rx) => {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(8), rx).await;
            }
            None => tokio::time::sleep(std::time::Duration::from_millis(1500)).await,
        }

        if needs_bootstrap {
            let _ = webview_nav::navigate(&view_for_show, &landing);
        }

        let _ = window_for_show.show();
        let _ = window_for_show.set_focus();
        tracing::info!("beanfun browser opened: {landing}");
    });

    Ok(())
}

/// Restack the toolbar and content view over the window's current size.
fn relayout(window: &tauri::Window) {
    let Ok(size) = window.inner_size() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let logical = size.to_logical::<f64>(scale);
    // Never let the toolbar eat the whole window, however tall the panel is.
    let bar_height = chrome_height().min((logical.height - 80.0).max(BAR_HEIGHT));
    let content_height = (logical.height - bar_height).max(0.0);

    if let Some(bar) = window.app_handle().get_webview(BAR) {
        let _ = bar.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = bar.set_size(LogicalSize::new(logical.width, bar_height));
    }
    if let Some(view) = window.app_handle().get_webview(VIEW) {
        let _ = view.set_position(LogicalPosition::new(0.0, bar_height));
        let _ = view.set_size(LogicalSize::new(logical.width, content_height));
    }
}

/// Close the browser and forget the session it belonged to.
///
/// Called on logout: a window still showing a signed-in member centre after the
/// user has signed out is the kind of thing that gets reported as a security
/// bug, and rightly.
pub async fn close(app: &tauri::AppHandle) {
    // The WebView2 profile on disk is shared by every window the launcher opens
    // and only ever holds one beanfun session at a time, so clearing it wholesale
    // is the right scope — the next window seeds itself from its own session.
    if let Some(view) = content_view(app) {
        let cleared = tauri::async_runtime::spawn_blocking(move || {
            cookie_native::clear_cookies_native(&view)
        })
        .await;
        if let Ok(Err(e)) = cleared {
            tracing::warn!("browser: could not clear cookies on close: {e}");
        }
    }
    if let Some(window) = app.get_window(WINDOW) {
        let _ = window.destroy();
    }
    if let Ok(mut held) = SESSION.lock() {
        *held = None;
    }
    if let Ok(mut height) = CHROME_HEIGHT.lock() {
        *height = BAR_HEIGHT;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_host_becomes_https() {
        assert_eq!(
            sanitize_url("tw.beanfun.com").unwrap(),
            "https://tw.beanfun.com/"
        );
    }

    #[test]
    fn keeps_path_and_query() {
        let url = sanitize_url("https://maplestory-event.beanfun.com/Event/E1/Index?a=1").unwrap();
        assert_eq!(
            url,
            "https://maplestory-event.beanfun.com/Event/E1/Index?a=1"
        );
    }

    #[test]
    fn refuses_script_and_file_schemes() {
        assert!(sanitize_url("javascript:alert(document.cookie)").is_err());
        assert!(sanitize_url("file:///C:/Windows/win.ini").is_err());
        assert!(sanitize_url("data:text/html,<script>1</script>").is_err());
    }

    #[test]
    fn web_token_never_reaches_the_address_bar() {
        let shown = redact(
            "https://tw.beanfun.com/TW/auth.aspx?channel=member&page_and_query=index_new.aspx&web_token=SECRET",
        );
        assert!(!shown.contains("SECRET"));
        assert!(shown.contains("channel=member"));
        assert!(shown.contains("page_and_query=index_new.aspx"));
    }

    #[test]
    fn redaction_leaves_ordinary_urls_alone() {
        let url = "https://maplestory-event.beanfun.com/Event/E1/Index?a=1";
        assert_eq!(redact(url), url);
    }

    #[test]
    fn scope_covers_beanfun_and_gamania_subdomains() {
        for url in [
            "https://tw.beanfun.com/TW/index.aspx",
            "https://maplestory-event.beanfun.com/Event/E1/Index",
            "https://beanfun.com/",
            "https://galaxy.games.gamania.com/webapi",
        ] {
            assert!(in_scope(&url.parse().unwrap()), "{url} should be in scope");
        }
    }

    #[test]
    fn scope_is_not_fooled_by_a_lookalike_domain() {
        for url in [
            "https://notbeanfun.com/",
            "https://beanfun.com.evil.example/",
            "https://example.com/beanfun.com",
        ] {
            assert!(!in_scope(&url.parse().unwrap()), "{url} should be out");
        }
    }

    #[test]
    fn bookmarks_are_region_specific() {
        let tw: Vec<String> = bookmarks(&Region::TW, "t")
            .into_iter()
            .map(|b| b.key)
            .collect();
        let hk: Vec<String> = bookmarks(&Region::HK, "t")
            .into_iter()
            .map(|b| b.key)
            .collect();
        assert!(tw.contains(&"browser.link.exchange".to_string()));
        assert!(!hk.contains(&"browser.link.exchange".to_string()));
        assert!(hk.contains(&"browser.link.support".to_string()));
        // Paying leaves the allowlist, so it is not offered here.
        assert!(!tw.contains(&"browser.link.topup".to_string()));
    }
}
