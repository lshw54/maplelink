//! Tauri commands for system-level operations.
//!
//! Includes frontend log forwarding, window resizing on page transitions,
//! native file dialog, and app version retrieval.

use reqwest::cookie::CookieStore;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::models::error::{ErrorCategory, ErrorDto};

/// User-Agent for WebView2 windows and HTTP requests.
const WEBVIEW_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

/// Forward a frontend log entry to the backend tracing system.
#[tauri::command]
pub fn log_frontend_error(level: String, module: String, message: String) -> Result<(), ErrorDto> {
    let level_lower = level.to_lowercase();

    match level_lower.as_str() {
        "trace" => tracing::trace!(frontend_module = %module, "{message}"),
        "debug" => tracing::debug!(frontend_module = %module, "{message}"),
        "info" => tracing::info!(frontend_module = %module, "{message}"),
        "warn" => tracing::warn!(frontend_module = %module, "{message}"),
        "error" => tracing::error!(frontend_module = %module, "{message}"),
        _ => {
            return Err(ErrorDto {
                code: "SYS_INVALID_LOG_LEVEL".to_string(),
                message: format!(
                    "Invalid log level: {level}. Expected one of: trace, debug, info, warn, error"
                ),
                category: ErrorCategory::Configuration,
                details: Some(level),
            });
        }
    }

    Ok(())
}

/// Resize the application window for a page transition.
#[tauri::command]
pub async fn resize_window(page: String, window: tauri::Window) -> Result<(), ErrorDto> {
    let (width, height): (f64, f64) = match page.as_str() {
        "login" => (350.0, 580.0),
        "qr-viewer" => (500.0, 560.0),
        "main" => (760.0, 530.0),
        "toolbox" => (750.0, 490.0),
        _ => {
            return Err(ErrorDto {
                code: "SYS_INVALID_PAGE".to_string(),
                message: format!("Unknown page: {page}"),
                category: ErrorCategory::Configuration,
                details: Some(page),
            });
        }
    };

    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e| ErrorDto {
            code: "SYS_RESIZE_FAILED".to_string(),
            message: format!("Failed to resize window: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })?;

    tracing::debug!("window {width}×{height} page='{page}'");
    Ok(())
}

/// Open a native file dialog for selecting a game executable.
#[tauri::command]
pub async fn open_file_dialog(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<Option<String>, ErrorDto> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let current_path = state.config.read().await.game_path.clone();
    let default_dir = if !current_path.is_empty() {
        std::path::Path::new(&current_path)
            .parent()
            .map(|p| p.to_path_buf())
    } else {
        None
    };

    let mut dialog = app.dialog().file();
    dialog = dialog
        .add_filter("Executable", &["exe"])
        .set_title("Select Game Executable");

    if let Some(dir) = default_dir {
        dialog = dialog.set_directory(dir);
    }

    dialog.pick_file(move |path| {
        let _ = tx.send(path.map(|p| p.to_string()));
    });

    rx.await.map_err(|_| ErrorDto {
        code: "SYS_DIALOG_FAILED".to_string(),
        message: "File dialog was cancelled unexpectedly".to_string(),
        category: ErrorCategory::Process,
        details: None,
    })
}

/// Return the application version from `Cargo.toml`.
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Auto-detect the MapleStory game path from the Windows Registry.
/// Inner function for detect_game_path, callable from both the command and startup.
pub async fn detect_game_path_inner(
    state: &crate::models::app_state::AppState,
) -> Result<Option<String>, ErrorDto> {
    detect_game_path_impl(state).await
}

#[tauri::command]
pub async fn detect_game_path(
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<Option<String>, ErrorDto> {
    detect_game_path_impl(&state).await
}

async fn detect_game_path_impl(
    state: &crate::models::app_state::AppState,
) -> Result<Option<String>, ErrorDto> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        let region = state.config.read().await.region.clone();
        let host = match region {
            crate::models::session::Region::HK => "bfweb.hk",
            crate::models::session::Region::TW => "tw",
        };
        let ini_url = format!(
            "https://{host}.beanfun.com/beanfun_block/generic_handlers/get_service_ini.ashx"
        );

        if let Ok(ini_text) = state
            .http_client
            .get(&ini_url)
            .header("User-Agent", WEBVIEW_USER_AGENT)
            .send()
            .await
        {
            if let Ok(body) = ini_text.text().await {
                let game_code = "610074_T9";
                let dir_reg = extract_ini_value(&body, game_code, "dir_reg");
                let dir_value_name = extract_ini_value(&body, game_code, "dir_value_name");
                let exe_field = extract_ini_value(&body, game_code, "exe");

                let exe_name = exe_field
                    .as_deref()
                    .and_then(|e| {
                        let name = e.split_whitespace().next().unwrap_or("");
                        if name.to_lowercase().ends_with(".exe") {
                            Some(name.to_string())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "MapleStory.exe".to_string());

                if let (Some(reg_path), Some(val_name)) = (dir_reg, dir_value_name) {
                    let reg_path = reg_path.replace("HKEY_LOCAL_MACHINE\\", "");
                    tracing::info!("INI dir_reg={reg_path}, dir_value_name={val_name}");

                    if let Ok(key) = hkcu.open_subkey(&reg_path) {
                        if let Ok(dir) = key.get_value::<String, _>(&val_name) {
                            if !dir.is_empty() {
                                let full_str = if dir.to_lowercase().ends_with(".exe") {
                                    dir
                                } else {
                                    std::path::Path::new(&dir)
                                        .join(&exe_name)
                                        .to_string_lossy()
                                        .to_string()
                                };
                                tracing::info!("detected game path from HKCU: {full_str}");
                                return Ok(Some(full_str));
                            }
                        }
                    }

                    let hklm = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
                    if let Ok(key) = hklm.open_subkey(&reg_path) {
                        if let Ok(dir) = key.get_value::<String, _>(&val_name) {
                            if !dir.is_empty() {
                                let full_str = if dir.to_lowercase().ends_with(".exe") {
                                    dir
                                } else {
                                    std::path::Path::new(&dir)
                                        .join(&exe_name)
                                        .to_string_lossy()
                                        .to_string()
                                };
                                tracing::info!("detected game path from HKLM: {full_str}");
                                return Ok(Some(full_str));
                            }
                        }
                    }
                }
            }
        }

        let candidates: &[(&str, &str)] = &[
            (r"Software\Gamania\MapleStory", "Path"),
            (r"Software\Wizet\MapleStory", "ExecPath"),
            (r"Software\Gamania\MapleStory HK", "Path"),
        ];

        for &(subkey, value_name) in candidates {
            if let Ok(key) = hkcu.open_subkey(subkey) {
                if let Ok(path) = key.get_value::<String, _>(value_name) {
                    if !path.is_empty() {
                        tracing::info!("detected game path from fallback registry {subkey}\\{value_name}: {path}");
                        return Ok(Some(path));
                    }
                }
            }
        }

        tracing::debug!("no game path found in registry");
        Ok(None)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        tracing::debug!("game path detection is only supported on Windows");
        Ok(None)
    }
}

/// Extract a value from a simple INI-style string for a given section and key.
fn extract_ini_value(ini: &str, section: &str, key: &str) -> Option<String> {
    let section_header = format!("[{section}]");
    let mut in_section = false;

    for line in ini.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == section_header;
            continue;
        }
        if in_section {
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Open the app's log directory in the system file explorer.
#[tauri::command]
pub async fn open_log_folder(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    let log_dir = app.path().app_log_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get log dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    open::that(&log_dir).map_err(|e| ErrorDto {
        code: "SYS_OPEN_FOLDER_FAILED".to_string(),
        message: format!("Failed to open folder: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    tracing::info!("opened log folder: {}", log_dir.display());
    Ok(())
}

/// Read the last N lines from the log file for clipboard copy.
#[tauri::command]
pub async fn get_recent_logs(app: tauri::AppHandle) -> Result<String, ErrorDto> {
    let log_dir = app.path().app_log_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get log dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    let log_file = log_dir.join("maplelink.log");
    let content = tokio::fs::read_to_string(&log_file)
        .await
        .unwrap_or_default();

    // Return last 100 lines
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(100);
    Ok(lines[start..].join("\n"))
}

/// Open or close the debug console window.
#[tauri::command]
pub async fn toggle_debug_window(enable: bool, app: tauri::AppHandle) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let label = "debug-console";

    if enable {
        if app.get_webview_window(label).is_some() {
            tracing::debug!("debug window already open");
            return Ok(());
        }

        // Use a separate data directory to avoid WebView2 lock conflicts
        // with the main window (fixes 0x8007139F in elevated processes).
        let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
            code: "SYS_PATH_ERROR".to_string(),
            message: format!("Failed to get app data dir: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })?;
        let debug_data_dir = data_dir.join("debug-webview");

        WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::App("debug.html".into()))
            .title("Debug Console")
            .inner_size(1000.0, 520.0)
            .decorations(false)
            .resizable(true)
            .shadow(true)
            .always_on_top(true)
            .data_directory(debug_data_dir)
            .build()
            .map_err(|e| ErrorDto {
                code: "SYS_DEBUG_WINDOW_FAILED".to_string(),
                message: format!("Failed to open debug window: {e}"),
                category: ErrorCategory::Process,
                details: None,
            })?;

        tracing::info!("debug console window opened");
    } else {
        if let Some(win) = app.get_webview_window(label) {
            let _ = win.destroy();
            tracing::info!("debug console window closed");
        }
    }

    Ok(())
}

/// Open Beanfun Gash Top-up popup with FULL cookie injection.
///
/// Flow:
/// 1. Call auth.aspx via reqwest (set server-side session)
/// 2. Open seed page HIDDEN — establishes .beanfun.com domain context in WebView2
/// 3. Inject cookies from reqwest jar into WebView2
/// 4. Navigate to gash page, show window
/// 5. After page loads, auto-resize window to fit the content area (right pane only)
#[tauri::command]
pub async fn open_gash_popup(
    session_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
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

    let config = state.config.read().await;
    let (host, region_path) = match config.region {
        crate::models::session::Region::HK => ("bfweb.hk.beanfun.com", "HK"),
        crate::models::session::Region::TW => ("tw.beanfun.com", "TW"),
    };
    drop(config);

    // Step 1: Call auth.aspx to establish server-side session via reqwest
    let auth_url = format!("https://{host}/{region_path}/auth.aspx");
    let _ = ss
        .http_client
        .get(&auth_url)
        .header(header::USER_AGENT, WEBVIEW_USER_AGENT)
        .send()
        .await;

    // Step 2: Collect cookies + build gash URL
    let jar_url: url::Url = format!("https://{host}/").parse().unwrap();
    let cookies: String = ss
        .cookie_jar
        .cookies(&jar_url)
        .and_then(|h: reqwest::header::HeaderValue| h.to_str().ok().map(|s: &str| s.to_string()))
        .unwrap_or_default();

    let token = get_web_token_from_jar(&ss.cookie_jar, &state).await?;
    let gash_url = format!(
        "https://{host}/{region_path}/auth.aspx?channel=gash&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={}",
        urlencoding::encode(&token)
    );

    tracing::debug!("gash_url={gash_url}");
    tracing::debug!("cookies to inject: {cookies}");

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    // Init script: TSPD bypass + window.open intercept (runs on every page load)
    let init_script = r#"
    (() => {
        // --- TSPD bypass ---
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

        // --- window.open → navigate top-level window ---
        // WebView2 blocks popup windows. Redirect to top so we stay inside
        // our managed WebviewWindow and don't break out of iframe context.
        const _open = window.open;
        window.open = function(url, target, features) {
            if (url && url !== '' && url !== 'about:blank') {
                console.log('[MapleLink] window.open →', url);
                try { window.top.location.href = url; } catch(e) { window.location.href = url; }
                return { closed: false, close(){}, focus(){}, location: { href: url } };
            }
            return _open.apply(this, arguments);
        };

        // --- <a target="_blank"> → same top-level navigation ---
        document.addEventListener('click', function(e) {
            const a = e.target.closest('a[target="_blank"]');
            if (a && a.href && !a.href.startsWith('javascript')) {
                e.preventDefault();
                try { window.top.location.href = a.href; } catch(e) { window.location.href = a.href; }
            }
        }, true);

        console.log('[MapleLink] init_script active');
    })();
    "#;

    // Step 3: Open seed page HIDDEN — do NOT touch innerHTML, just let it load silently.
    // This establishes the .beanfun.com domain context so document.cookie works correctly.
    let seed_url = format!("https://{host}/");
    let window = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External(seed_url.parse().unwrap()),
    )
    .title("Beanfun 儲值與購點")
    .inner_size(840.0, 520.0) // initial size — will be auto-adjusted after load
    .min_inner_size(600.0, 400.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(false) // hidden until we navigate to the real gash page
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

    // Step 4: Wait for seed page to load, then inject cookies from reqwest jar.
    // Do NOT call window.show() or eval innerHTML — keep it fully hidden.
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    if !cookies.is_empty() {
        let inject_js = format!(
            r#"(function() {{
                `{cookies}`.split(';').forEach(c => {{
                    const t = c.trim();
                    if (t) document.cookie = t + '; path=/; domain=.beanfun.com; SameSite=None; Secure';
                }});
                console.log('[MapleLink] cookies injected');
            }})();"#,
            cookies = cookies
        );
        let _ = window.eval(&inject_js);
    }

    // Step 5: Navigate to gash page (still hidden)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let _ = window.eval(format!("window.location.href = '{gash_url}';"));

    // Step 6: Wait for gash page to fully render, then:
    //   a) measure the RIGHT-side content pane (not the sidebar) for width
    //   b) measure full scrollHeight for height
    //   c) resize + show
    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

        // Measure the content area, not the whole page.
        // The beanfun gash page has a right-side content pane; we want the
        // sidebar + content pane total width, but using scrollWidth of the
        // eval() is fire-and-forget in Tauri v2 — use IPC callback instead
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

        // Small delay for resize to complete, then show
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let _ = win_clone.show();
    });

    tracing::info!("gash popup: seed(hidden) → {gash_url}");
    Ok(())
}

/// Resize the gash popup window (called from JS inside the popup).
#[tauri::command]
pub async fn resize_gash_popup(
    width: f64,
    height: f64,
    app: tauri::AppHandle,
) -> Result<(), ErrorDto> {
    if let Some(win) = app.get_webview_window("gash-popup") {
        let size = tauri::LogicalSize::new(width, height);
        let _ = win.set_size(tauri::Size::Logical(size));
        let _ = win.center();
        tracing::info!("gash popup resized to {width}x{height}");
    }
    Ok(())
}

/// Open the member center in a popup WebviewWindow.
///
/// Uses the same shared data_directory + cookie injection flow as gash popup.
#[tauri::command]
pub async fn open_member_popup(
    session_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<(), ErrorDto> {
    use reqwest::cookie::CookieStore;
    use tauri::WebviewWindowBuilder;

    let ss = state.require_session(&session_id).await?;

    let label = "member-popup";

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let config = state.config.read().await;
    let (host, region_path, page_query) = match config.region {
        crate::models::session::Region::HK => (
            "bfweb.hk.beanfun.com",
            "HK",
            "default.aspx%3Fservice_code%3D999999%26service_region%3DT0",
        ),
        crate::models::session::Region::TW => ("tw.beanfun.com", "TW", "index_new.aspx"),
    };
    drop(config);

    let token = get_web_token_from_jar(&ss.cookie_jar, &state).await?;
    let url = format!(
        "https://{host}/{region_path}/auth.aspx?channel=member&page_and_query={page_query}&web_token={}",
        urlencoding::encode(&token)
    );

    // Inject cookies from session's reqwest jar
    let jar_url: url::Url = format!("https://{host}/").parse().unwrap();
    let cookies: String = ss
        .cookie_jar
        .cookies(&jar_url)
        .and_then(|h: reqwest::header::HeaderValue| h.to_str().ok().map(|s: &str| s.to_string()))
        .unwrap_or_default();

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    let seed_url = format!("https://{host}/");
    let win = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External(seed_url.parse().unwrap()),
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

    // Inject cookies then navigate
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    if !cookies.is_empty() {
        for cookie in cookies.split(';') {
            let c = cookie.trim();
            if c.is_empty() {
                continue;
            }
            let js = format!(
                "document.cookie = '{}; domain=.beanfun.com; path=/; secure';",
                c.replace('\'', "\\'")
            );
            let _ = win.eval(&js);
        }
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let _ = win.eval(format!("window.location.href = '{}';", url));
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let _ = win.show();

    tracing::info!("member popup opened: {url}");
    Ok(())
}

/// Open customer service page in system browser.
///
/// Customer service pages don't require auth — just open the URL directly.
#[tauri::command]
pub async fn open_customer_service(
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<(), ErrorDto> {
    let config = state.config.read().await;
    let url = match config.region {
        crate::models::session::Region::HK => {
            "https://bfweb.hk.beanfun.com/newfaq/service_newBF.aspx"
        }
        crate::models::session::Region::TW => {
            "https://tw.beanfun.com/customerservice/www/main.aspx"
        }
    };
    drop(config);

    open::that(url).map_err(|e| ErrorDto {
        code: "SYS_OPEN_FAILED".to_string(),
        message: format!("Failed to open: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    tracing::info!("customer service opened: {url}");
    Ok(())
}

/// Open a simple WebView popup window for a given URL (no auth needed).
///
/// Used for public pages like forgot password, customer service, etc.
#[tauri::command]
pub async fn open_web_popup(
    url: String,
    title: String,
    app: tauri::AppHandle,
    _state: tauri::State<'_, crate::models::app_state::AppState>,
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

/// Get the bfWebToken from the cookie jar for constructing authenticated URLs.
#[tauri::command]
pub async fn get_web_token(
    session_id: String,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<String, ErrorDto> {
    let ss = state.require_session(&session_id).await?;
    get_web_token_from_jar(&ss.cookie_jar, &state).await
}

/// Internal helper: extract bfWebToken from a cookie jar.
async fn get_web_token_from_jar(
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    state: &tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<String, ErrorDto> {
    let config = state.config.read().await;
    let host = match config.region {
        crate::models::session::Region::HK => "bfweb.hk.beanfun.com",
        crate::models::session::Region::TW => "tw.beanfun.com",
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

/// Clean up game cache directories, failed update leftovers, crash dumps,
/// and stale DLL files from the game directory.
///
/// Matches the reference Beanfun `btn_Recycling_Click` logic.
#[tauri::command]
pub async fn cleanup_game_cache(
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<String, ErrorDto> {
    let game_path = state.config.read().await.game_path.clone();

    if game_path.is_empty() {
        return Err(ErrorDto {
            code: "SYS_NO_GAME_PATH".to_string(),
            message: "Game path is not configured".into(),
            category: ErrorCategory::Configuration,
            details: None,
        });
    }

    let game_dir = std::path::Path::new(&game_path)
        .parent()
        .ok_or_else(|| ErrorDto {
            code: "SYS_INVALID_PATH".to_string(),
            message: "Cannot determine game directory".into(),
            category: ErrorCategory::Configuration,
            details: Some(game_path.clone()),
        })?
        .to_path_buf();

    if !game_dir.exists() {
        return Err(ErrorDto {
            code: "SYS_DIR_NOT_FOUND".to_string(),
            message: format!("Game directory not found: {}", game_dir.display()),
            category: ErrorCategory::FileSystem,
            details: Some(game_dir.display().to_string()),
        });
    }

    let mut cleaned = Vec::new();

    // 1. Remove known cache directories
    let cache_dirs = ["blob_storage", "GPUCache", "VideoDecodeStats", "XignCode"];
    for dir_name in &cache_dirs {
        let dir = game_dir.join(dir_name);
        if dir.exists() && std::fs::remove_dir_all(&dir).is_ok() {
            cleaned.push(format!("dir: {dir_name}"));
        }
    }

    // 2. Remove failed update cache (directories ending with .$$$)
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".$$$") && std::fs::remove_dir_all(&path).is_ok() {
                        cleaned.push(format!("dir: {name}"));
                    }
                }
            }
        }
    }

    // 3. Remove crash dumps (.dmp) and stale DLLs
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let lower = name.to_lowercase();
                    if (lower.ends_with(".dmp")
                        || lower == "localeemulator.dll"
                        || lower == "loaderdll.dll")
                        && std::fs::remove_file(&path).is_ok()
                    {
                        cleaned.push(format!("file: {name}"));
                    }
                }
            }
        }
    }

    let summary = if cleaned.is_empty() {
        "nothing to clean".to_string()
    } else {
        format!("cleaned {} items", cleaned.len())
    };

    tracing::info!("game cache cleanup: {summary} ({:?})", cleaned);
    Ok(summary)
}
