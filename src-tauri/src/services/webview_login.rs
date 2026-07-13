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
    let incognito = config.gamepass_incognito;
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

    // Navigate to bflogin/default.aspx — the server will redirect to
    // login.beanfun.com/Login/Index?pSKey={skey} automatically.
    // This way the WebView2 has the same session cookies as the skey request.
    let start_url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";

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
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .initialization_script(&init_script)
    .devtools(true);

    // In normal mode, persist WebView2 data so "remember me" works.
    // In incognito mode, use a unique temp directory per session.
    let webview_data_dir = if incognito {
        let temp = std::env::temp_dir()
            .join("MapleLink")
            .join("gamepass-incognito")
            .join(format!("{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        temp
    } else {
        data_dir
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
    if let Err(e) = crate::services::cookie_native::register_native_popup_handler(&window) {
        tracing::warn!("GamePass: failed to register native popup handler: {e}");
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
                return; // window closed (finalized elsewhere or user cancelled)
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
        refresh_token: None,
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
        crate::services::beanfun_service::parse_tw_account_list_html(account_html)
    });

    tracing::info!("GamePass: got {} accounts", accounts.len());

    let dto = SessionDto::from_session(&session, session_id);
    *ss.session.write().await = Some(session);
    *ss.game_accounts.write().await = accounts;

    let _ = app.emit("gamepass-login-complete", dto);

    if let Some(win) = app.get_webview_window("gamepass-login") {
        let _ = win.destroy();
    }

    tracing::info!("=== GamePass finalized (session installed) ===");
    Ok(true)
}

// ---------------------------------------------------------------------------
// Regular (帳密) web login — full login in a webview, then harvest cookies
// ---------------------------------------------------------------------------
//
// Same robust pattern as GamePass: the user completes the entire login inside a
// real beanfun page (account + password + reCAPTCHA + any advance check), then
// we harvest ALL cookies from WebView2 and build the reqwest session — instead
// of the fragile "grab a reCAPTCHA token and replay it" approach. The only
// difference from GamePass is the login page prefills the saved credentials.

/// Injected into the regular web-login window. Reads `window.__ML_*` globals
/// (set by a prelude) — a plain raw string so the JS braces need no escaping.
const REGULAR_LOGIN_SCRIPT: &str = r#"
(() => {
  const SESSION_ID = window.__ML_SESSION_ID__ || "";
  const ACCOUNT = window.__ML_ACCOUNT__ || "";
  const PASSWORD = window.__ML_PASSWORD__ || "";

  const url = window.location.href;
  const onBeanfun = url.includes('beanfun.com');
  const onLoginPage = url.includes('Login/Index');
  const onGamania = url.includes('accounts.gamania.com');
  const onDefaultAspx = url.includes('bflogin/default.aspx');

  try { Object.defineProperty(navigator, 'webdriver', { get: () => false }); } catch (e) {}

  // Tauri hijacks window.alert to its dialog plugin, which isn't permitted for
  // this remote origin (CSP/ACL) and throws. Route beanfun's alerts to the
  // console so they never crash the page — real errors also show inline.
  try { window.alert = function (m) { console.log('[beanfun alert]', m); }; } catch (e) {}

  if (onDefaultAspx || onGamania || !onBeanfun) return;

  const _origFetch = window.fetch;
  const hasSession = () => document.cookie.indexOf('bfWebToken=') !== -1;
  console.log('[WebLogin] page', url, '| login?', onLoginPage, '| session?', hasSession());

  // Only treat as the login form when we're on Login/Index AND not yet logged
  // in — a post-login redirect back to a Login/Index URL should fall through to
  // the harvest below.
  if (onLoginPage && !hasSession()) {
    // Self-heal reCAPTCHA: WebView2 Tracking Prevention is only turned off a
    // moment AFTER this window is created, so the very first load inits reCAPTCHA
    // with google/gstatic storage blocked and it fails to render. Wait ~4s (long
    // enough for prevention to be applied), and if no reCAPTCHA widget appeared,
    // reload once — the reload runs with prevention off and it renders. Same
    // proven approach as the standalone reCAPTCHA helper window.
    try {
      const store = window.sessionStorage;
      const RK = '__wl_reloads__';
      setTimeout(() => {
        if (document.querySelector('iframe[src*="recaptcha"]')) return; // rendered OK
        let done = 0;
        try { done = parseInt(store.getItem(RK) || '0', 10) || 0; } catch (e) {}
        if (done >= 1) {
          console.warn('[WebLogin] reCAPTCHA still missing after reload');
          return;
        }
        try { store.setItem(RK, String(done + 1)); } catch (e) {}
        console.warn('[WebLogin] reCAPTCHA not rendered — reloading once');
        location.reload();
      }, 2500);
    } catch (e) {}

    // beanfun's login is a Vue app with a TWO-STEP flow: enter account →
    // CheckAccountType (reveals the password field) → enter password →
    // AccountLogin. The inputs use deliberately-obfuscated names — account is
    // name="aaa", password is name="inputName" (password only shows after
    // step 1). We fill each once when it appears (empty), dispatching a native
    // 'input' event so Vue's v-model picks up the value; the user just solves
    // the reCAPTCHA and clicks through.
    const setVal = (el, v) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
    };
    let accFilled = false;
    let pwFilled = false;
    const prefill = () => {
      const acc = document.querySelector(
        'input[name="aaa"], input[type="email"], input[type="text"]:not([type="hidden"])'
      );
      const pw = document.querySelector('input[name="inputName"], input[type="password"]');
      if (acc && ACCOUNT && !accFilled && !acc.value) { setVal(acc, ACCOUNT); accFilled = true; }
      if (pw && PASSWORD && !pwFilled && !pw.value) { setVal(pw, PASSWORD); pwFilled = true; }
      return accFilled && pwFilled;
    };
    prefill();
    // Keep watching for up to ~60s — the password field only appears after the
    // account step (which needs the reCAPTCHA + a click).
    let n = 0;
    const timer = setInterval(() => {
      n++;
      if (prefill() || n > 120) clearInterval(timer);
    }, 500);

    // Auto-click the login buttons once the user solves the reCAPTCHA, so they
    // only ever have to solve the challenge. Step 1 button is "登入帳號"
    // (submitAccountName), step 2 is "繼續" (submitPassword); both require the
    // reCAPTCHA token, and it resets between steps (so a fresh token = a fresh
    // click). We click the first visible, enabled one that matches.
    const recaptchaToken = () => {
      try {
        const g = window.grecaptcha;
        if (g && g.enterprise && g.enterprise.getResponse) return g.enterprise.getResponse() || '';
        if (g && g.getResponse) return g.getResponse() || '';
      } catch (e) {}
      return '';
    };
    let lastToken = '';
    setInterval(() => {
      const token = recaptchaToken();
      if (!token || token === lastToken) return;
      const btns = document.querySelectorAll('a.ui-btn');
      for (const b of btns) {
        const txt = (b.textContent || '').trim();
        const visible = b.offsetParent !== null;
        const disabled = b.classList.contains('disabled');
        if (visible && !disabled && (txt.indexOf('登入帳號') !== -1 || txt.indexOf('繼續') !== -1)) {
          lastToken = token;
          console.log('[WebLogin] reCAPTCHA solved — auto-clicking', txt);
          b.click();
          break;
        }
      }
    }, 400);
    return;
  }

  // Post-login: nothing to do here. beanfun's page CSP blocks Tauri IPC on
  // these pages, so the backend watches this window's cookies for bfWebToken and
  // harvests them itself (see open_regular_web_login).
  console.log('[WebLogin] post-login page — backend will harvest cookies');
})();
"#;

/// Open the regular (帳密) web-login window: the user completes the whole login
/// on the official page (credentials prefilled), then cookies are harvested by
/// a backend watcher once `bfWebToken` appears.
pub async fn open_regular_web_login_window(
    app: tauri::AppHandle,
    session_id: String,
    account: String,
    password: String,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let label = "web-login";
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Fresh incognito profile each time so the login form (for prefill + the
    // user's reCAPTCHA) always shows instead of auto-logging-in from a cookie.
    let data_dir = std::env::temp_dir()
        .join("MapleLink")
        .join("web-login")
        .join(uuid::Uuid::new_v4().to_string());
    let _ = std::fs::create_dir_all(&data_dir);
    let cleanup_dir = data_dir.clone();

    let start_url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";

    let prelude = format!(
        "window.__ML_SESSION_ID__={};window.__ML_ACCOUNT__={};window.__ML_PASSWORD__={};\n",
        serde_json::to_string(&session_id).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&account).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&password).unwrap_or_else(|_| "\"\"".into()),
    );
    let init_script = format!("{prelude}{REGULAR_LOGIN_SCRIPT}");

    let window = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External(start_url.parse().expect("static login URL is valid")),
    )
    .title("Beanfun 登入")
    // Wide enough for beanfun's desktop login layout (promo panel + the form
    // with the account/password fields + reCAPTCHA on the right). At ~420px only
    // the left promo shows and the form is scrolled off.
    .inner_size(1000.0, 680.0)
    .min_inner_size(720.0, 560.0)
    .center()
    .visible(true)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .data_directory(data_dir)
    .initialization_script(&init_script)
    .devtools(true)
    .build()
    .map_err(|e| ErrorDto {
        code: "AUTH_WEB_LOGIN_WINDOW_FAILED".to_string(),
        message: format!("Failed to open login window: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // The login page renders Google reCAPTCHA; WebView2 Tracking Prevention
    // blocks its google.com/gstatic storage and makes it unclickable. Disable it
    // for this window (same as the standalone reCAPTCHA helper).
    disable_tracking_prevention(&window);

    // Clean up the incognito profile after the window closes.
    let cleanup_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if cleanup_app.get_webview_window("web-login").is_none() {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = std::fs::remove_dir_all(&cleanup_dir);
                break;
            }
        }
    });

    // Watch the window's cookies for bfWebToken (set once login completes) and
    // harvest them ourselves — beanfun's page CSP blocks Tauri IPC, so the page
    // can't signal us. Fully URL-agnostic; no IPC/capability needed.
    let watch_app = app.clone();
    let watch_session = session_id.clone();
    let watch_account = account.clone();
    tauri::async_runtime::spawn(async move {
        use tauri::Emitter;
        // ~5 minutes at 1.5s.
        for _ in 0..200 {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            let Some(_win) = watch_app.get_webview_window("web-login") else {
                return; // window closed / cancelled
            };
            let cookies = extract_webview2_cookies(&watch_app, "web-login").await;
            if !cookies.iter().any(|(name, _, _, _)| name == "bfWebToken") {
                continue; // not logged in yet
            }
            tracing::info!("web-login: bfWebToken detected — harvesting session");
            let Some(state) = watch_app.try_state::<AppState>() else {
                return;
            };
            let Some(ss) = state.get_session(&watch_session).await else {
                return;
            };
            match finalize_webview_login(
                &watch_app,
                &ss,
                &watch_session,
                "web-login",
                &watch_account,
                "cookie_auth",
                "",
            )
            .await
            {
                Ok(dto) => {
                    tracing::info!("regular web-login complete: {}", dto.account_name);
                    let _ = watch_app.emit("regular-login-complete", dto);
                }
                Err(e) => {
                    tracing::error!("regular web-login harvest failed: {e}");
                    let _ = watch_app.emit("regular-login-error", format!("登入失敗: {e}"));
                }
            }
            if let Some(win) = watch_app.get_webview_window("web-login") {
                let _ = win.destroy();
            }
            return;
        }
        tracing::warn!("web-login: no login detected within timeout");
    });

    tracing::info!("regular web-login window opened");
    Ok(())
}

/// Shared finalization for a webview-completed beanfun login: harvest ALL
/// WebView2 cookies from `window_label`, inject them into the session's reqwest
/// jar, build the [`crate::models::session::Session`] with `account_name`,
/// fetch game accounts, and store both. Returns the [`SessionDto`] or an error
/// message.
pub async fn finalize_webview_login(
    app: &tauri::AppHandle,
    ss: &std::sync::Arc<crate::models::session_state::SessionState>,
    session_id: &str,
    window_label: &str,
    account_name: &str,
    web_token_fallback: &str,
    account_html: &str,
) -> Result<SessionDto, String> {
    let all_cookies = extract_webview2_cookies(app, window_label).await;
    tracing::info!(
        "web-login: extracted {} cookies from WebView2",
        all_cookies.len()
    );

    let real_web_token = all_cookies
        .iter()
        .find(|(name, _, _, _)| name == "bfWebToken")
        .map(|(_, value, _, _)| value.clone())
        .unwrap_or_else(|| {
            if web_token_fallback != "cookie_auth" && !web_token_fallback.is_empty() {
                web_token_fallback.to_string()
            } else {
                String::new()
            }
        });

    if real_web_token.is_empty() {
        return Err("no bfWebToken found in webview cookies".to_string());
    }

    inject_cookies_into_jar(&ss.cookie_jar, &all_cookies);

    let session = crate::models::session::Session {
        token: real_web_token,
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: crate::models::session::Region::TW,
        account_name: account_name.to_string(),
        session_key: None,
        totp_state: None,
    };

    let accounts = crate::services::beanfun_service::get_game_accounts(
        &ss.http_client,
        &session,
        &ss.cookie_jar,
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("web-login: reqwest get_game_accounts failed: {e}, using webview HTML");
        crate::services::beanfun_service::parse_tw_account_list_html(account_html)
    });
    tracing::info!("web-login: got {} accounts", accounts.len());

    let dto = SessionDto::from_session(&session, session_id);
    *ss.session.write().await = Some(session);
    *ss.game_accounts.write().await = accounts;
    Ok(dto)
}

/// Seed a reqwest cookie jar with harvested WebView2 cookies, routing each to
/// the beanfun origin matching its domain.
fn inject_cookies_into_jar(
    jar: &std::sync::Arc<reqwest::cookie::Jar>,
    cookies: &[crate::services::webview_util::CookieTuple],
) {
    let tw_url: url::Url = "https://tw.beanfun.com/".parse().unwrap();
    let login_url: url::Url = "https://login.beanfun.com/".parse().unwrap();
    let newlogin_url: url::Url = "https://tw.newlogin.beanfun.com/".parse().unwrap();

    for (name, value, domain, path) in cookies {
        let clean_domain = domain.trim_start_matches('.');
        let jar_url =
            if clean_domain.contains("login.beanfun.com") && !clean_domain.contains("newlogin") {
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
