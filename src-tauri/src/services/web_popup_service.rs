//! Beanfun WebView2 popup windows (gash top-up, member center, customer
//! service, authenticated report pages, generic public pages) plus the
//! `bfWebToken` cookie-jar lookup they share.
//!
//! Extracted from `commands/system.rs` so the command layer stays thin and all
//! webview/cookie side effects live in `services/` (Clean Architecture). The
//! command handlers are thin wrappers that delegate here.

use reqwest::cookie::CookieStore;
use tauri::Manager;

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::models::session::Region;
use crate::services::cookie_native;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

/// Extract `bfWebToken` from a cookie jar (region host resolved from config).
pub async fn web_token_from_jar(
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    state: &AppState,
) -> Result<String, ErrorDto> {
    let config = state.config.read().await;
    let host = match config.region {
        Region::HK => "bfweb.hk.beanfun.com",
        Region::TW => "tw.beanfun.com",
    };
    drop(config);

    let jar_url: url::Url = format!("https://{host}/").parse().unwrap();
    let token = cookie_jar
        .cookies(&jar_url)
        .and_then(|h: reqwest::header::HeaderValue| {
            h.to_str().ok().and_then(|s: &str| {
                s.split(';')
                    .find_map(|c: &str| c.trim().strip_prefix("bfWebToken=").map(String::from))
            })
        })
        .unwrap_or_default();

    Ok(token)
}

/// Open the gash (top-up / buy points) popup with native COM cookie seeding.
pub async fn open_gash_popup(
    session_id: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), ErrorDto> {
    use reqwest::header;
    use tauri::WebviewWindowBuilder;

    let ss = state.require_session(&session_id).await?;

    let label = "gash-popup";

    // Close existing popup if open
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    // Use the SESSION's region (the account being viewed), NOT the global config
    // toggle — otherwise a TW account opens HK URLs (seeded with the TW session's
    // cookies → beanfun serves a logged-out page = a fake logout).
    let region = match ss.session.read().await.as_ref() {
        Some(s) => s.region.clone(),
        None => state.config.read().await.region.clone(),
    };
    let (host, region_path) = match region {
        Region::HK => ("bfweb.hk.beanfun.com", "HK"),
        Region::TW => ("tw.beanfun.com", "TW"),
    };

    // Step 1: Call auth.aspx via reqwest to establish server-side session
    let auth_url = format!("https://{host}/{region_path}/auth.aspx");
    let _ = ss
        .http_client
        .get(&auth_url)
        .header(header::USER_AGENT, WEBVIEW_USER_AGENT)
        .send()
        .await;

    // Build gash URL with web_token
    let token = web_token_from_jar(&ss.cookie_jar, state).await?;
    let gash_url = format!(
        "https://{host}/{region_path}/auth.aspx?channel=gash&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={}",
        urlencoding::encode(&token)
    );

    // Collect cookies from reqwest jar for native seeding
    let seed_cookies = cookie_native::cookies_from_jar(
        &ss.cookie_jar,
        &[&format!("https://{host}/"), "https://beanfun.com/"],
    );

    tracing::debug!("gash_url={gash_url}");
    tracing::debug!("cookies to seed natively: {}", seed_cookies.len());

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    // Init script: TSPD bypass only (popup handling is now native COM)
    let init_script = r#"
    (() => {
        Object.defineProperty(navigator, 'webdriver', {get: () => false});
        Object.defineProperty(navigator, 'plugins',   {get: () => [1,2,3,4,5]});
        Object.defineProperty(navigator, 'languages', {get: () => ['zh-TW','zh','en-US']});
        Object.defineProperty(navigator, 'hardwareConcurrency', {get: () => 8});

        const blockList = ['127.0.0.1:8888', 'chrome-extension://invalid', 'burp/favicon'];
        const _fetch = window.fetch;
        window.fetch = async (input, init) => {
            const url = typeof input === 'string' ? input : (input.url || '');
            if (blockList.some(b => url.includes(b))) return new Response('', {status: 404});
            return _fetch(input, init);
        };
        const _xhr = XMLHttpRequest.prototype.open;
        XMLHttpRequest.prototype.open = function(method, url) {
            if (blockList.some(b => (typeof url === 'string' ? url : '').includes(b))) return;
            return _xhr.apply(this, arguments);
        };

        console.log('[MapleLink] gash init_script active (native popup handler)');
    })();
    "#;

    // Step 2: Build window on about:blank (HIDDEN)
    let window = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("Beanfun 儲值與購點")
    .inner_size(840.0, 520.0)
    .min_inner_size(600.0, 400.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(false) // hidden until page loads
    .data_directory(data_dir)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .initialization_script(init_script)
    .devtools(true)
    .build()
    .map_err(|e| ErrorDto {
        code: "SYS_POPUP_FAILED".to_string(),
        message: format!("Failed to open gash popup: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    // Step 3: Register native NewWindowRequested handler (COM event)
    // This intercepts target="_blank" / window.open() at the WebView2 level,
    // redirecting popups to navigate the same window. Bypasses wry completely.
    if let Err(e) = cookie_native::register_new_window_handler(&window) {
        tracing::warn!("NewWindowRequested handler failed: {e}, falling back to JS intercept");
        // Fallback: inject JS-based window.open intercept
        let _ = window.eval(
            r#"
            const _open = window.open;
            window.open = function(url, target, features) {
                if (url && url !== '' && url !== 'about:blank') {
                    try { window.top.location.href = url; } catch(e) { window.location.href = url; }
                    return { closed: false, close(){}, focus(){}, location: { href: url } };
                }
                return _open.apply(this, arguments);
            };
        "#,
        );
    }

    // Step 4: Seed cookies via native COM CookieManager
    // This sets HttpOnly cookies (bfWebToken, ASP.NET_SessionId, etc.) that
    // document.cookie cannot access. Domain=.beanfun.com with leading dot
    // ensures subdomain matching (bfweb.hk.beanfun.com, etc.)
    if let Err(e) = cookie_native::seed_cookies_native(&window, &seed_cookies) {
        tracing::warn!("Native cookie seeding failed: {e}, falling back to JS injection");
        // Fallback: navigate to host first, then inject via document.cookie
        let seed_url = format!("https://{host}/");
        let _ = window.eval(format!("window.location.href = '{}';", seed_url));
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;

        let jar_url: url::Url = format!("https://{host}/").parse().unwrap();
        let cookies: String = ss
            .cookie_jar
            .cookies(&jar_url)
            .and_then(|h: reqwest::header::HeaderValue| {
                h.to_str().ok().map(|s: &str| s.to_string())
            })
            .unwrap_or_default();
        if !cookies.is_empty() {
            let inject_js = format!(
                r#"(function() {{
                    `{cookies}`.split(';').forEach(c => {{
                        const t = c.trim();
                        if (t) document.cookie = t + '; path=/; domain=.beanfun.com; SameSite=None; Secure';
                    }});
                }})();"#,
                cookies = cookies
            );
            let _ = window.eval(&inject_js);
        }
    }

    // Step 5: Register NavigationCompleted handler BEFORE navigating
    let nav_rx = cookie_native::on_navigation_completed(&window).ok();

    // Step 6: Navigate to gash auth page (carries seeded cookies)
    // No flush delay needed — native CookieManager is synchronous.
    let _ = window.eval(format!("window.location.href = '{}';", gash_url));

    // Step 7: Wait for NavigationCompleted event (or 5s safety timeout), then show
    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        // Wait for page to finish loading via native event, with 5s safety timeout
        if let Some(rx) = nav_rx {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
        } else {
            // Fallback if handler registration failed
            tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        }

        // Small yield for DOM to settle after NavigationCompleted fires
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Auto-resize based on content
        let resize_js = r#"
        (function() {
            var container =
                document.querySelector('table[width]') ||
                document.querySelector('#wrapper') ||
                document.querySelector('#container') ||
                document.querySelector('#main') ||
                document.body;
            var contentW = container ? container.offsetWidth : document.body.scrollWidth;
            var contentH = document.body.scrollHeight;
            var w = Math.min(Math.max(contentW + 48, 700), 1200);
            var h = Math.min(Math.max(contentH + 80, 460), 900);
            console.log('[MapleLink] measured:', contentW, contentH, '→', w, h);
            if (window.__TAURI__ && window.__TAURI__.core) {
                window.__TAURI__.core.invoke('resize_gash_popup', { width: w, height: h });
            }
        })();
        "#;
        let _ = win_clone.eval(resize_js);

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let _ = win_clone.show();
        let _ = win_clone.set_focus();
    });

    tracing::info!("gash popup: about:blank → native seed → {gash_url}");
    Ok(())
}

/// Open the member center popup (same native COM cookie seeding as gash).
pub async fn open_member_popup(
    session_id: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let ss = state.require_session(&session_id).await?;

    let label = "member-popup";

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    // Session's region, not the global config toggle (see open_gash_popup).
    let region = match ss.session.read().await.as_ref() {
        Some(s) => s.region.clone(),
        None => state.config.read().await.region.clone(),
    };
    let (host, region_path, page_query) = match region {
        Region::HK => (
            "bfweb.hk.beanfun.com",
            "HK",
            "default.aspx%3Fservice_code%3D999999%26service_region%3DT0",
        ),
        Region::TW => ("tw.beanfun.com", "TW", "index_new.aspx"),
    };

    let token = web_token_from_jar(&ss.cookie_jar, state).await?;
    let url = format!(
        "https://{host}/{region_path}/auth.aspx?channel=member&page_and_query={page_query}&web_token={}",
        urlencoding::encode(&token)
    );

    // Collect cookies for native seeding
    let seed_cookies = cookie_native::cookies_from_jar(
        &ss.cookie_jar,
        &[&format!("https://{host}/"), "https://beanfun.com/"],
    );

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    // Build on about:blank (hidden)
    let win = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("Beanfun 會員中心")
    .inner_size(1024.0, 720.0)
    .min_inner_size(400.0, 300.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(false)
    .data_directory(data_dir)
    .user_agent(WEBVIEW_USER_AGENT)
    .devtools(true)
    .build()
    .map_err(|e| ErrorDto {
        code: "SYS_POPUP_FAILED".to_string(),
        message: format!("Failed to open member popup: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    // Register native NewWindowRequested handler
    if let Err(e) = cookie_native::register_new_window_handler(&win) {
        tracing::warn!("member popup: NewWindowRequested handler failed: {e}");
    }

    // Seed cookies via native COM CookieManager
    if let Err(e) = cookie_native::seed_cookies_native(&win, &seed_cookies) {
        tracing::warn!("member popup: native cookie seeding failed: {e}");
    }

    // Register NavigationCompleted handler BEFORE navigating
    let nav_rx = cookie_native::on_navigation_completed(&win).ok();

    // Navigate (no flush delay — native CookieManager is synchronous)
    let _ = win.eval(format!("window.location.href = '{}';", url));

    // Wait for NavigationCompleted event (or 5s safety timeout), then show
    let win_clone = win.clone();
    let url_log = url.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(rx) = nav_rx {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let _ = win_clone.show();
        let _ = win_clone.set_focus();
        tracing::info!("member popup opened: {url_log}");
    });

    Ok(())
}

/// Open the region-appropriate customer-service page in a public popup.
pub async fn open_customer_service(
    session_id: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), ErrorDto> {
    // Region from the active session, not the global config toggle.
    let region = match state.get_session(&session_id).await {
        Some(ss) => match ss.session.read().await.as_ref() {
            Some(s) => s.region.clone(),
            None => state.config.read().await.region.clone(),
        },
        None => state.config.read().await.region.clone(),
    };
    let url = match region {
        Region::HK => "https://bfweb.hk.beanfun.com/newfaq/service_newBF.aspx",
        Region::TW => "https://tw.beanfun.com/customerservice/www/main.aspx",
    }
    .to_string();

    open_web_popup(url, "客服中心".to_string(), app).await
}

/// Open an authenticated WebView popup with cookie seeding (e.g. report pages).
pub async fn open_auth_popup(
    session_id: String,
    url: String,
    title: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let ss = state.require_session(&session_id).await?;
    let label = "auth-popup";

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let config = state.config.read().await;
    let host = match config.region {
        Region::HK => "bfweb.hk.beanfun.com",
        Region::TW => "tw.beanfun.com",
    };
    drop(config);

    // Seed cookies from the session jar — covers all beanfun domains
    let seed_cookies = cookie_native::cookies_from_jar(
        &ss.cookie_jar,
        &[
            &format!("https://{host}/"),
            "https://beanfun.com/",
            "https://event.beanfun.com/",
            "https://m.beanfun.com/",
            "https://login.beanfun.com/",
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
    .title(&title)
    .inner_size(1024.0, 720.0)
    .min_inner_size(400.0, 300.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(false)
    .data_directory(data_dir)
    .user_agent(WEBVIEW_USER_AGENT)
    .build()
    .map_err(|e| ErrorDto {
        code: "SYS_POPUP_FAILED".to_string(),
        message: format!("Failed to open auth popup: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    if let Err(e) = cookie_native::register_new_window_handler(&win) {
        tracing::warn!("auth popup: NewWindowRequested handler failed: {e}");
    }

    if let Err(e) = cookie_native::seed_cookies_native(&win, &seed_cookies) {
        tracing::warn!("auth popup: native cookie seeding failed: {e}");
    }

    let nav_rx = cookie_native::on_navigation_completed(&win).ok();
    let _ = win.eval(format!("window.location.href = '{}';", url));

    let win_clone = win.clone();
    let url_log = url.clone();
    let title_log = title.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(rx) = nav_rx {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let _ = win_clone.show();
        let _ = win_clone.set_focus();
        tracing::info!("auth popup opened: {url_log} ({title_log})");
    });

    Ok(())
}

/// Open a simple public WebView popup for a given URL (no auth/cookies).
pub async fn open_web_popup(
    url: String,
    title: String,
    app: tauri::AppHandle,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let label = "web-popup";

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    let win = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External(url.parse().map_err(|_| ErrorDto {
            code: "SYS_INVALID_URL".to_string(),
            message: format!("Invalid URL: {url}"),
            category: ErrorCategory::Process,
            details: None,
        })?),
    )
    .title(&title)
    .inner_size(800.0, 600.0)
    .min_inner_size(400.0, 300.0)
    .decorations(true)
    .resizable(true)
    .center()
    .data_directory(data_dir)
    .user_agent(WEBVIEW_USER_AGENT)
    .build()
    .map_err(|e| ErrorDto {
        code: "SYS_POPUP_FAILED".to_string(),
        message: format!("Failed to open popup: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    let _ = win.show();
    tracing::debug!("web popup opened: {title} -> {url}");
    Ok(())
}
