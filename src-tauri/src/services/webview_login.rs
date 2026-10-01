//! Webview-based beanfun logins (TW region): GamePass OAuth and the regular
//! (帳密) login completed on the official page, followed by a full WebView2
//! cookie harvest into the session's reqwest jar.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Manager;

use crate::models::app_state::AppState;
use crate::models::error::ErrorDto;
use crate::models::session::SessionDto;
use crate::services::webview_util::{
    disable_tracking_prevention, extract_webview2_cookies, WEBVIEW_USER_AGENT,
};

/// Set once a GamePass flow has been finalized, so the backend cookie poll and
/// the JS `gamepass_webview_done` IPC (whichever wins the race) only complete the
/// login once.
static GAMEPASS_DONE: AtomicBool = AtomicBool::new(false);

/// Open the GamePass login popup (TW region) and start the completion poll.
///
/// Creates a new session for this GamePass login flow. The entire login
/// flow happens inside the WebView2:
/// 1. Navigate to `bflogin/default.aspx` → server redirects to `Login/Index?pSKey={skey}`
/// 2. Init script auto-clicks `a.use-gama-pass` on the login page
/// 3. User completes GamePass OAuth in the webview
/// 4. Init script polls `echo_token.ashx` until session is ready
/// 5. Fetches account list HTML inside the webview, then signals backend
pub async fn open_gamepass_login_window(
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<String, ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let label = "gamepass-login";

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let config = state.config.read().await;
    if config.region != crate::models::session::Region::TW {
        return Err(ErrorDto {
            code: "AUTH_GAMEPASS_UNSUPPORTED".to_string(),
            message: "GamePass login is only available for TW region".to_string(),
            category: crate::models::error::ErrorCategory::Authentication,
            details: None,
        });
    }
    let mut incognito = config.gamepass_incognito;
    drop(config);

    // Create a new session for this GamePass login flow
    let (session_id, _) = state.create_session().await;
    tracing::info!("GamePass: created session {session_id}");

    // Fresh flow — clear the "already finalized" guard.
    GAMEPASS_DONE.store(false, Ordering::SeqCst);

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // A password manager answering the passkey needs its extension loaded,
    // which needs the persistent, extension-enabled profile: the manager's
    // vault state lives in it.
    let manager = crate::services::passkey_manager::PasskeySource::parse(
        crate::services::prefs::get(&data_dir, crate::services::passkey_manager::PREF_SOURCE)
            .as_deref(),
    )
    .manager()
    .filter(|m| {
        let installed = crate::services::passkey_manager::status(&data_dir, *m).is_some();
        if !installed {
            tracing::warn!(
                "GamePass: {} is selected but not installed; using the Windows prompt",
                m.display_name()
            );
        }
        installed
    });
    if manager.is_some() {
        incognito = false;
    }

    // Navigate to bflogin/default.aspx — the server will redirect to
    // login.beanfun.com/Login/Index?pSKey={skey} automatically.
    // This way the WebView2 has the same session cookies as the skey request.
    let start_url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";
    tracing::info!(
        passkey_source = manager.map_or("windows", |m| m.key()),
        incognito,
        "GamePass: window setup"
    );

    // Pass session_id to the init script so it can invoke gamepass_webview_done with it
    let init_script = format!(
        r#"
    (() => {{
        const SESSION_ID = "{}";
        const url = window.location.href;
        const onBeanfun = url.includes('beanfun.com');
        const onLoginPage = url.includes('Login/Index');
        const onGamania = url.includes('accounts.gamania.com');
        const onDefaultAspx = url.includes('bflogin/default.aspx');

        Object.defineProperty(navigator, 'webdriver', {{get: () => false}});

        // Skip: initial redirect page, gamania OAuth, non-beanfun pages
        if (onDefaultAspx || onGamania || !onBeanfun) return;

        const _origFetch = window.fetch;

        // On login page: auto-click GamePass button
        if (onLoginPage) {{
            function tryClick() {{
                const btn = document.querySelector('a.use-gama-pass');
                if (btn) {{ btn.click(); console.log('[GamePass] clicked use-gama-pass'); return true; }}
                return false;
            }}
            // Try immediately
            if (!tryClick()) {{
                // Retry with MutationObserver + polling fallback
                const obs = new MutationObserver(() => {{ if (tryClick()) obs.disconnect(); }});
                if (document.body) {{
                    obs.observe(document.body, {{ childList: true, subtree: true }});
                }}
                // Also poll every 200ms for up to 10s as fallback
                let attempts = 0;
                const poller = setInterval(() => {{
                    attempts++;
                    if (tryClick() || attempts > 50) {{
                        clearInterval(poller);
                        obs.disconnect();
                    }}
                }}, 200);
            }}
            return;
        }}

        // On beanfun.com post-login pages (return.aspx, index.aspx, etc)
        // Poll echo_token, then fetch accounts, then signal backend
        console.log('[GamePass] post-login page detected:', url);

        (async function() {{
            // Poll echo_token until session is confirmed
            let ready = false;
            for (let i = 0; i < 60; i++) {{
                try {{
                    const r = await _origFetch(
                        'https://tw.beanfun.com/beanfun_block/generic_handlers/echo_token.ashx?webtoken=1',
                        {{ credentials: 'include' }}
                    );
                    const t = await r.text();
                    if (t.includes('ResultCode:1')) {{
                        console.log('[GamePass] session ready at attempt', i);
                        ready = true;
                        break;
                    }}
                }} catch(e) {{}}
                await new Promise(r => setTimeout(r, 500));
            }}

            if (!ready) {{
                console.log('[GamePass] session not ready, skipping (might be pre-login page)');
                return;
            }}

            // Session is ready — fetch account list
            console.log('[GamePass] fetching account list...');
            let accountHtml = '';
            try {{
                const sc = '610074', sr = 'T9';
                await _origFetch(
                    'https://tw.beanfun.com/beanfun_block/auth.aspx?channel=game_zone'
                    + '&page_and_query=game_start.aspx%3Fservice_code_and_region%3D' + sc + '_' + sr
                    + '&web_token=1',
                    {{ credentials: 'include' }}
                );
                const listResp = await _origFetch(
                    'https://tw.beanfun.com/beanfun_block/game_zone/game_server_account_list.aspx'
                    + '?sc=' + sc + '&sr=' + sr + '&dt=' + Date.now(),
                    {{ credentials: 'include' }}
                );
                accountHtml = await listResp.text();
                console.log('[GamePass] account list length:', accountHtml.length);
            }} catch(e) {{
                console.error('[GamePass] account list fetch failed:', e);
            }}

            const cookies = document.cookie;
            let webToken = 'cookie_auth';
            cookies.split(';').forEach(c => {{
                const t = c.trim();
                if (t.startsWith('bfWebToken=')) webToken = t.substring(11);
            }});

            if (window.__TAURI_INTERNALS__) {{
                console.log('[GamePass] invoking backend...');
                window.__TAURI_INTERNALS__.invoke('gamepass_webview_done', {{
                    sessionId: SESSION_ID,
                    webToken: webToken,
                    cookies: cookies,
                    accountHtml: accountHtml
                }}).then(() => console.log('[GamePass] SUCCESS'))
                  .catch(e => console.error('[GamePass] FAILED', e));
            }}
        }})();
    }})();
    "#,
        session_id
    );

    let browser_args = crate::services::webview_util::browser_args(&app).await;
    let mut builder = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External(start_url.parse().unwrap()),
    )
    .title("Beanfun GamePass Login")
    .inner_size(420.0, 580.0)
    .min_inner_size(380.0, 500.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(true)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args(&browser_args)
    .initialization_script(&init_script)
    .devtools(true);
    // Extensions are added by `gamepass_extensions::ensure_loaded` below,
    // never through wry's `extensions_path`: that re-adds on every window
    // and broke the extension's pages and service worker.
    if manager.is_some() {
        builder = builder.browser_extensions_enabled(true);
    }

    // Three profiles: a throwaway one per login in incognito mode, the plain
    // persistent one so "remember me" works, and the extension-enabled one
    // when a password manager answers the passkey (its own folder, because
    // WebView2 environment options are fixed per folder per process).
    let webview_data_dir = if incognito {
        let temp = std::env::temp_dir()
            .join("MapleLink")
            .join("gamepass-incognito")
            .join(format!("{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        temp
    } else if manager.is_some() {
        crate::services::gamepass_extensions::profile_dir(&data_dir)
    } else {
        data_dir.clone()
    };
    builder = builder.data_directory(webview_data_dir.clone());

    let window = builder.build().map_err(|e| ErrorDto {
        code: "AUTH_GAMEPASS_WINDOW_FAILED".to_string(),
        message: format!("Failed to open GamePass login window: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // Google / Facebook / Apple sign-in need their third-party storage and an
    // OAuth popup. WebView2 Tracking Prevention blocks the SDK storage (see the
    // "Tracking Prevention blocked ... connect.facebook.net / apis.google.com"
    // errors) and the popup is blocked (POPUP_MAYBE_BLOCKED_OAUTH). Disable
    // tracking prevention and route window.open through the main window so the
    // provider login page loads instead of being blocked.
    disable_tracking_prevention(&window);
    // Let OAuth popups open as real popup windows (with window.opener) so
    // Google/Facebook/Apple sign-in can postMessage + close back to the opener.
    // Navigating the main window instead breaks that final step.
    let host = manager.map(|_| {
        crate::services::gamepass_extensions::ExtensionHost::new(
            app.clone(),
            webview_data_dir.clone(),
            browser_args.clone(),
            label,
        )
    });
    if let Err(e) = crate::services::cookie_native::register_native_popup_handler(
        &window,
        host.as_ref().map(|h| h.opener()),
    ) {
        tracing::warn!("GamePass: failed to register native popup handler: {e}");
    }
    if let Some(m) = manager {
        let mut dirs = vec![m.install_dir(&data_dir)];
        match crate::services::passkey_manager::helper::ensure_written(&data_dir) {
            Ok(helper) => dirs.push(helper),
            Err(e) => tracing::warn!("GamePass extensions: helper: {e}"),
        }
        if let Err(e) =
            crate::services::gamepass_extensions::ensure_loaded(&window, &webview_data_dir, dirs)
        {
            tracing::warn!("GamePass extensions: ensure_loaded failed: {e}");
        }
        crate::services::gamepass_extensions::log_credentials_override(&window);
        // Keep the manager awake for the whole login: a hidden page of its
        // own is the only thing WebView2 honours for that.
        let keepalive_app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = trigger_helper(keepalive_app, m, true, "gamepass-login").await {
                tracing::warn!("GamePass extensions: keepalive: {e}");
            }
        });
    }

    // Backend completion poll (the robust path). The injected JS tries to reach
    // us over IPC, but beanfun's page CSP blocks ipc.localhost, so that invoke
    // can silently fail and the login stalls on the beanfun portal without ever
    // reaching the account list. Instead, poll the WebView2 cookie store from the
    // backend and finalize as soon as the HttpOnly `bfWebToken` appears — exactly
    // when the OAuth redirect lands back on the beanfun portal.
    let poll_app = app.clone();
    let poll_sid = session_id.clone();
    tauri::async_runtime::spawn(async move {
        // ~5 minutes at 500ms; the window-gone check ends it early on close.
        for _ in 0..600 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if poll_app.get_webview_window("gamepass-login").is_none() {
                // Window closed (finalized elsewhere or user cancelled): the
                // extension's own windows have nothing left to do.
                crate::services::gamepass_extensions::close_all(&poll_app);
                return;
            }
            let st = poll_app.state::<AppState>();
            match try_finalize_gamepass(&poll_app, st.inner(), &poll_sid, "", "").await {
                Ok(true) => return, // finalized
                Ok(false) => {}     // token not there yet — keep polling
                Err(e) => tracing::debug!("GamePass poll: {}", e.message),
            }
        }
        tracing::warn!("GamePass: completion poll timed out after ~5 min");
    });

    // Clean up incognito temp dir when window closes (best-effort)
    if incognito {
        let app_clone = app.clone();
        let temp_dir = webview_data_dir;
        tauri::async_runtime::spawn(async move {
            // Wait for window to be destroyed
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if app_clone.get_webview_window("gamepass-login").is_none() {
                    // Small delay to let WebView2 release file locks
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    tracing::debug!("GamePass: cleaned up incognito temp dir");
                    break;
                }
            }
        });
    }

    tracing::info!("GamePass login window opened");
    // Return the session_id so the frontend can track this GamePass session
    Ok(session_id)
}

/// Core GamePass completion, shared by the JS `gamepass_webview_done` IPC and the
/// backend cookie poll. Pulls ALL WebView2 cookies (incl. HttpOnly `bfWebToken`),
/// seeds the session jar, fetches accounts, emits `gamepass-login-complete`, and
/// closes the window.
///
/// Returns `Ok(true)` once `bfWebToken` exists and the session is installed, or
/// `Ok(false)` while it isn't there yet (the caller should keep waiting). The
/// `GAMEPASS_DONE` guard ensures only the first caller to see the token finalizes.
pub async fn try_finalize_gamepass(
    app: &tauri::AppHandle,
    state: &AppState,
    session_id: &str,
    account_html: &str,
    js_web_token: &str,
) -> Result<bool, ErrorDto> {
    use tauri::Emitter;

    let ss = state.require_session(session_id).await?;

    // Extract ALL cookies from WebView2 via CookieManager (including HttpOnly).
    let all_cookies = extract_webview2_cookies(app, "gamepass-login").await;

    // Find bfWebToken from the extracted cookies (fall back to the JS value only
    // when it's a real token, not the "cookie_auth" placeholder).
    let real_web_token = all_cookies
        .iter()
        .find(|(name, _, _, _)| name == "bfWebToken")
        .map(|(_, value, _, _)| value.clone())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            if js_web_token != "cookie_auth" && !js_web_token.is_empty() {
                Some(js_web_token.to_string())
            } else {
                None
            }
        });

    let Some(real_web_token) = real_web_token else {
        // Not logged in yet — no token on any origin. Keep waiting.
        return Ok(false);
    };

    // Only the first caller (poll vs JS IPC) that sees the token finalizes.
    if GAMEPASS_DONE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(true);
    }

    tracing::info!(
        "GamePass: finalizing — {} cookies, bfWebToken = {}...",
        all_cookies.len(),
        &real_web_token[..real_web_token.len().min(20)]
    );

    // Inject ALL cookies into the session's reqwest jar
    inject_cookies_into_jar(&ss.cookie_jar, &all_cookies);

    tracing::info!("GamePass: injected all cookies into session's reqwest jar");

    // Step 3: Build session with real bfWebToken
    let session = crate::models::session::Session {
        token: real_web_token,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: crate::models::session::Region::TW,
        account_name: "GamePass".to_string(),
        session_key: None,
        totp_state: None,
    };

    // Step 4: Fetch game accounts via reqwest (now has full cookies)
    let accounts = crate::services::beanfun_service::get_game_accounts(
        &ss.http_client,
        &session,
        &ss.cookie_jar,
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("GamePass: reqwest get_game_accounts failed: {e}, trying webview HTML");
        crate::models::game_account::AccountList {
            accounts: crate::services::beanfun_service::parse_tw_account_list_html(account_html),
            limit_notice: crate::services::beanfun_service::parse_account_limit_notice(
                account_html,
            ),
        }
    });

    tracing::info!("GamePass: got {} accounts", accounts.accounts.len());

    let dto = SessionDto::from_session(&session, session_id);
    *ss.session.write().await = Some(session);
    ss.store_account_list(accounts).await;

    let _ = app.emit("gamepass-login-complete", dto);

    if let Some(win) = app.get_webview_window("gamepass-login") {
        let _ = win.destroy();
    }
    crate::services::gamepass_extensions::close_all(app);

    tracing::info!("=== GamePass finalized (session installed) ===");
    Ok(true)
}

/// Seed a reqwest cookie jar with harvested WebView2 cookies, routing each to
/// the beanfun origin matching its domain.
fn inject_cookies_into_jar(
    jar: &std::sync::Arc<reqwest::cookie::Jar>,
    cookies: &[crate::services::cookie_native::SeedCookie],
) {
    let tw_url: url::Url = "https://tw.beanfun.com/".parse().unwrap();
    let login_url: url::Url = "https://login.beanfun.com/".parse().unwrap();
    let newlogin_url: url::Url = "https://tw.newlogin.beanfun.com/".parse().unwrap();
    let openid_url: url::Url = "https://openid.beanfun.com/".parse().unwrap();

    for (name, value, domain, path) in cookies {
        let clean_domain = domain.trim_start_matches('.');
        // openid is checked first: it does not contain "login.beanfun.com", so
        // it would otherwise land on the tw origin and never be sent back.
        let jar_url = if clean_domain.contains("openid.beanfun.com") {
            &openid_url
        } else if clean_domain.contains("login.beanfun.com") && !clean_domain.contains("newlogin") {
            &login_url
        } else if clean_domain.contains("newlogin") {
            &newlogin_url
        } else {
            &tw_url
        };
        let path_str = if path.is_empty() { "/" } else { path.as_str() };
        let cookie_str = format!("{}={}; Domain={}; Path={}", name, value, domain, path_str);
        jar.add_cookie_str(&cookie_str, jar_url);
    }
}

/// Ask MapleLink's helper extension to open `manager`'s vault page, visible
/// (for signing in) or hidden (to keep the manager's service worker alive
/// while GamaPass is open).
///
/// WebView2 refuses to render an extension page the host navigates to, so a
/// hidden loader window on the extension-enabled profile loads the manager
/// and the helper, then navigates to the helper's sentinel URL; the helper
/// opens the page as a popup, which lands in a hosted MapleLink window like
/// any other extension page.
async fn trigger_helper(
    app: tauri::AppHandle,
    manager: crate::services::passkey_manager::Manager,
    hidden: bool,
    owner_label: &str,
) -> Result<(), String> {
    use crate::services::{gamepass_extensions, passkey_manager};

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    if passkey_manager::status(&data_dir, manager).is_none() {
        return Err(format!("{} is not installed", manager.display_name()));
    }
    let helper_dir = passkey_manager::helper::ensure_written(&data_dir)?;
    let profile = gamepass_extensions::profile_dir(&data_dir);
    let browser_args = crate::services::webview_util::browser_args(&app).await;

    let label = if hidden {
        "gamepass-keepalive-loader"
    } else {
        "gamepass-manager-loader"
    };
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    let loader = tauri::WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External("about:blank".parse().expect("about:blank parses")),
    )
    .title(manager.display_name())
    .inner_size(10.0, 10.0)
    .visible(false)
    .skip_taskbar(true)
    .data_directory(profile.clone())
    .browser_extensions_enabled(true)
    .additional_browser_args(&browser_args)
    .build()
    .map_err(|e| format!("open loader: {e}"))?;

    // The loader goes away after the hand-off, so the hosted window's owner
    // is the caller's window (or none), never the loader.
    let host = gamepass_extensions::ExtensionHost::new(
        app.clone(),
        profile.clone(),
        browser_args,
        owner_label,
    );
    if let Err(e) =
        crate::services::cookie_native::register_native_popup_handler(&loader, Some(host.opener()))
    {
        tracing::warn!("GamePass extensions: popup handler on {label}: {e}");
    }
    gamepass_extensions::ensure_loaded(
        &loader,
        &profile,
        vec![manager.install_dir(&data_dir), helper_dir],
    )?;
    let sentinel = if hidden {
        passkey_manager::helper::keepalive_url(manager)
    } else {
        passkey_manager::helper::sentinel_url(manager)
    };
    let url: url::Url = sentinel.parse().map_err(|e| format!("sentinel url: {e}"))?;
    loader.navigate(url).map_err(|e| format!("navigate: {e}"))?;
    tracing::info!(
        "GamePass extensions: asked the helper to open {} ({})",
        manager.display_name(),
        if hidden { "keepalive" } else { "vault" }
    );
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        let _ = loader.destroy();
    });
    Ok(())
}

/// Open `manager`'s vault page so the player can sign in or unlock.
pub async fn open_passkey_manager_window(
    app: tauri::AppHandle,
    manager: crate::services::passkey_manager::Manager,
) -> Result<(), String> {
    trigger_helper(app, manager, false, "").await
}
