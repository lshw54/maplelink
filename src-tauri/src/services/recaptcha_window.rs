//! TW Regular (帳密) login — external reCAPTCHA helper window.
//!
//! The TW "Regular" (username + password) login on `login.beanfun.com` is
//! guarded by Google reCAPTCHA. Inside a bare WebView2 the widget frequently
//! fails to render — usually because `www.google.com` is unreachable for the
//! user. This helper opens the *official* login page in a small, frameless
//! window so the human can solve their own challenge; the injected script only
//! (a) swaps the reCAPTCHA origin to the Google-supported `recaptcha.net`
//! mirror so the widget loads, (b) trims the surrounding page chrome, and
//! (c) hands the token the human produced back to the backend. The challenge
//! is solved by a person — nothing here auto-solves or bypasses it.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Manager;

use crate::models::error::ErrorDto;
use crate::services::webview_util::{disable_tracking_prevention, WEBVIEW_USER_AGENT};

/// Window label for the external reCAPTCHA helper window.
pub const RECAPTCHA_WINDOW_LABEL: &str = "recaptcha_window";

/// Set right before the backend destroys the helper window after a SUCCESSFUL
/// token capture, so the window-destroyed handler can tell that apart from a
/// user closing the window. Without this, the destroy after phase 1 emits a
/// spurious `recaptcha-cancelled` that can abort phase 2.
static RECAPTCHA_DELIVERED: AtomicBool = AtomicBool::new(false);

/// Consume the "token delivered" flag. `true` → the window closed because we
/// captured a token (suppress cancel); `false` → user/close abort (emit cancel).
pub fn recaptcha_take_delivered() -> bool {
    RECAPTCHA_DELIVERED.swap(false, Ordering::SeqCst)
}

/// Injected at document-start into the reCAPTCHA helper window.
///
/// No `format!` placeholders here on purpose — it's a plain raw string so the
/// JS braces/backticks need no escaping.
const RECAPTCHA_INIT_SCRIPT: &str = r#"
(() => {
  'use strict';
  // The reCAPTCHA token is origin-locked to login.beanfun.com, so the widget is
  // solved on beanfun's own Login/Index page. Do NOT swap www.google.com for
  // recaptcha.net: the Enterprise endpoint 404s on the mirror, leaving
  // grecaptcha.render undefined. (Mainland support would need that wired up and
  // verified separately.)

  // Parity with the rest of the app's webviews.
  try { Object.defineProperty(navigator, 'webdriver', { get: () => false }); } catch (e) {}

  const OVERLAY_Z = 999999; // MUST stay below reCAPTCHA's challenge bframe (~2e9)

  const onReady = () => {
    // The helper window first passes through redirect pages (default.aspx →
    // checkin → Login/Index). Only act on the actual login form.
    if (location.href.indexOf('Login/Index') === -1) return;
    console.log('[reCAPTCHA] on Login/Index, watching for token');

    if (document.getElementById('__ov')) return;

    // (1) Lay an opaque overlay + spinner from first paint so the user never
    //     sees beanfun's raw login page flash by.
    const style = document.createElement('style');
    style.textContent =
      '#__ov{position:fixed;inset:0;z-index:' + OVERLAY_Z + ';background:#1c1712;' +
      'display:flex;flex-direction:column;align-items:center;justify-content:center;gap:18px;' +
      'color:#f4ede4;font-family:system-ui,sans-serif;font-size:14px}' +
      '#__sp{width:34px;height:34px;border-radius:50%;border:3px solid rgba(244,237,228,.25);' +
      'border-top-color:#ff8201;animation:__r .8s linear infinite}' +
      '@keyframes __r{to{transform:rotate(360deg)}}' +
      '.grecaptcha-badge{z-index:' + (OVERLAY_Z + 1) + ' !important}';
    const ov = document.createElement('div'); ov.id = '__ov';
    const sp = document.createElement('div'); sp.id = '__sp';
    const lb = document.createElement('div'); lb.textContent = '驗證載入中，請稍候…';
    ov.appendChild(sp); ov.appendChild(lb);
    (document.head || document.documentElement).appendChild(style);
    (document.body || document.documentElement).appendChild(ov);

    // (2) Detect the ANCHOR iframe (the "I'm not a robot" checkbox, not the
    //     nine-grid bframe challenge).
    const anchor = () =>
      document.querySelector("iframe[src*='recaptcha'][src*='anchor']") ||
      document.querySelector("iframe[title='reCAPTCHA']") ||
      document.querySelector("iframe[src*='recaptcha']");

    const findWidget = () => {
      const a = anchor(); if (!a) return null;
      let w = a.closest('.g-recaptcha');
      if (!w) {
        w = a;
        for (let i = 0; i < 5 && w.parentElement && w.parentElement !== document.body; i++) {
          w = w.parentElement;
          if (w.offsetWidth >= 280 && w.offsetWidth <= 400) break;
        }
      }
      return w;
    };

    let store = null;
    try { store = window.sessionStorage; } catch (e) {}
    const RKEY = '__rc_reloads__';

    // (3) Once the widget exists, REPARENT it into the overlay. This is the only
    //     reliable presentation: a pure z-index lift leaves the cross-origin
    //     (out-of-process) iframe visible but unclickable (trapped in its
    //     stacking context). Moving the iframe reloads it → the checkbox resets
    //     to unchecked, but the user ticks the fresh one and getResponse() still
    //     returns a fully valid token. If no widget ever renders, reveal the real
    //     page (or, once the reload budget is spent, close so the frontend isn't
    //     stuck on a blank helper — closing fires `recaptcha-cancelled`).
    let placed = false;
    // Reveal the real reCAPTCHA page (remove the opaque overlay). This is the
    // graceful fallback for slow accelerator/VPN connections where the widget
    // loads late or our selectors miss it: the user is NEVER left stuck on a
    // spinner, and never depends on an IPC close that beanfun's CSP can block —
    // they just solve the reCAPTCHA on the visible page (the token harvest below
    // still works). Less pretty than the reparent, but always usable.
    const reveal = () => {
      if (placed) return;
      placed = true; clearInterval(rt);
      try { if (store) store.removeItem(RKEY); } catch (e) {}
      ov.remove();
    };
    const place = () => {
      if (placed) return;
      const w = findWidget();
      if (w && w !== document.body && w !== document.documentElement) {
        placed = true; clearInterval(rt);
        try { if (store) store.removeItem(RKEY); } catch (e) {}
        w.style.position = 'relative';
        w.style.zIndex = String(OVERLAY_Z + 2);
        sp.remove();
        lb.textContent = '請完成「我不是機器人」驗證';
        ov.appendChild(w); // reparent → iframe reload → fresh checkbox
      }
    };
    const rt = setInterval(() => { if (anchor()) place(); }, 200);

    // (3b) Self-heal beanfun's first-load render race: ONLY if there is no
    //      reCAPTCHA iframe at all after 4s (genuine render fail), reload once.
    //      Slow connections keep the iframe present, so they skip this and just
    //      wait for the widget → they don't get a disruptive reload.
    setTimeout(() => {
      if (!placed && !document.querySelector("iframe[src*='recaptcha']") && store && !store.getItem(RKEY)) {
        store.setItem(RKEY, '1');
        console.warn('[reCAPTCHA] no widget — reloading once');
        location.reload();
      }
    }, 4000);

    // Final fallback: if the widget still isn't reparented, reveal the real page
    // rather than hide it behind the spinner forever.
    setTimeout(reveal, 6500);

    // (4) Harvest the Enterprise token once the human ticks the checkbox.
    //     Primary source is grecaptcha.enterprise.getResponse() — beanfun is an
    //     Enterprise checkbox and reads it straight into its XHR rather than into
    //     a stable hidden input. Fall back to the response <textarea> / beanfun's
    //     own field. We never fire beanfun's submit, so the token stays
    //     unconsumed and can be replayed into AccountLogin's `Captcha` field.
    const readToken = () => {
      try {
        const g = window.grecaptcha;
        if (g && g.enterprise && typeof g.enterprise.getResponse === 'function') {
          const t = g.enterprise.getResponse();
          if (t) return t;
        }
        if (g && typeof g.getResponse === 'function') {
          const t = g.getResponse();
          if (t) return t;
        }
      } catch (e) {}
      const el = document.getElementById('recaptcha-token') ||
        document.querySelector('#g-recaptcha-response, textarea[name="g-recaptcha-response"]');
      return el ? el.value : '';
    };

    const initial = readToken();
    let done = false;
    const timer = setInterval(() => {
      if (done) return;
      const val = readToken();
      if (val && val !== initial && val.length > 50) {
        done = true;
        clearInterval(timer);
        if (store) { try { store.removeItem(RKEY); } catch (e) {} }
        const step = window.__RECAPTCHA_STEP__ || 'login';
        // beanfun's page CSP blocks Tauri IPC (ipc.localhost → connect-src), so
        // hand the token to the backend through a URL fragment it polls.
        // Changing the hash isn't gated by connect-src and doesn't reload.
        // reCAPTCHA tokens are URL-safe base64url, so '~' is a safe separator.
        try { window.location.hash = 'mltoken=' + step + '~' + val; } catch (e) {}
        // Best-effort IPC too, in case CSP allows it on some pages.
        try {
          if (window.__TAURI_INTERNALS__) {
            window.__TAURI_INTERNALS__
              .invoke('submit_login_token', { token: val, step })
              .catch(() => {});
          }
        } catch (e) {}
        console.log('[reCAPTCHA] token captured, handed to backend via fragment');
      }
    }, 500);
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', onReady, { once: true });
  } else {
    onReady();
  }
})();
"#;

/// Open the external reCAPTCHA helper window for TW Regular (帳密) login.
///
/// Points a small, frameless, always-on-top WebView2 at the official login
/// page and injects [`RECAPTCHA_INIT_SCRIPT`]. The human solves the challenge;
/// the script hands the token back via a URL fragment the backend polls (with
/// the `submit_login_token` IPC as fallback).
///
/// `step` distinguishes the two challenge points (`"check"` for
/// `CheckAccountType`, `"login"` for `AccountLogin`); it is echoed back in the
/// `recaptcha-token` event so the frontend can route each token.
pub async fn open_recaptcha_helper_window(
    app: tauri::AppHandle,
    step: String,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    // Replace any existing helper window so we never stack two. Mark it as a
    // backend close so its destroy doesn't emit a cancel that aborts this solve.
    if let Some(existing) = app.get_webview_window(RECAPTCHA_WINDOW_LABEL) {
        RECAPTCHA_DELIVERED.store(true, Ordering::SeqCst);
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Tag the window with its step (consumed by RECAPTCHA_INIT_SCRIPT). The
    // body's braces are safe here: it's interpolated as a value, not reparsed
    // as a format string.
    let init_script = format!("window.__RECAPTCHA_STEP__ = {step:?};\n{RECAPTCHA_INIT_SCRIPT}");

    // Use a FRESH (incognito) profile every time. A persistent profile retains
    // beanfun's login cookie, so loading Login/Index auto-completes the pSKey
    // login and redirects to the portal — no reCAPTCHA is ever shown, no token
    // is captured, and the frontend hangs on "登入中". A unique dir guarantees a
    // clean login page with the reCAPTCHA every time.
    let data_dir = std::env::temp_dir()
        .join("MapleLink")
        .join("recaptcha-incognito")
        .join(uuid::Uuid::new_v4().to_string());
    let _ = std::fs::create_dir_all(&data_dir);
    let cleanup_dir = data_dir.clone();

    // Navigate to bflogin/default.aspx — the server redirects to
    // login.beanfun.com/Login/Index?pSKey={skey} (a bare Login/Index with no
    // pSKey renders blank). Same entry point as open_gamepass_login.
    let url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";

    let window = WebviewWindowBuilder::new(
        &app,
        RECAPTCHA_WINDOW_LABEL,
        tauri::WebviewUrl::External(url.parse().expect("static reCAPTCHA URL is valid")),
    )
    .title("Beanfun 驗證")
    // Roomy enough that reCAPTCHA's image-challenge popup (~400px wide, opens
    // beside the centred checkbox) isn't clipped on the right/bottom. Resizable
    // so the user can enlarge further if a taller challenge appears.
    .inner_size(500.0, 680.0)
    .min_inner_size(440.0, 560.0)
    .resizable(true)
    // Keep the native title bar so the user can always close the window — a
    // frameless window that fails to render leaves no way out and hangs the
    // login. Closing fires `recaptcha-cancelled`, which unblocks the frontend.
    .decorations(true)
    .always_on_top(true)
    .center()
    .visible(true)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .data_directory(data_dir)
    .initialization_script(&init_script)
    .build()
    .map_err(|e| ErrorDto {
        code: "AUTH_RECAPTCHA_WINDOW_FAILED".to_string(),
        message: format!("Failed to open reCAPTCHA window: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // WebView2 Tracking Prevention blocks google.com/gstatic third-party
    // storage, which breaks reCAPTCHA's interaction (widget renders but clicks
    // don't verify). Turn it off for this window's profile.
    disable_tracking_prevention(&window);

    // beanfun's page CSP blocks Tauri IPC (ipc.localhost), so the injected
    // script hands the token back via the URL fragment (#mltoken=<step>~<token>).
    // Poll the window URL for it.
    let poll_app = app.clone();
    tauri::async_runtime::spawn(async move {
        // ~3 minutes at 500ms; the window-gone check ends it early on close.
        for _ in 0..360 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let Some(win) = poll_app.get_webview_window(RECAPTCHA_WINDOW_LABEL) else {
                return; // window closed or token already delivered
            };
            let Ok(u) = win.url() else { continue };
            let Some(frag) = u.fragment() else { continue };
            let Some(rest) = frag.strip_prefix("mltoken=") else {
                continue;
            };
            if let Some((step, token)) = rest.split_once('~') {
                if token.len() > 50 {
                    deliver_recaptcha_token(&poll_app, token.to_string(), step.to_string());
                    return;
                }
            }
        }
    });

    // Remove the incognito profile once the window closes (best effort).
    let cleanup_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if cleanup_app
                .get_webview_window(RECAPTCHA_WINDOW_LABEL)
                .is_none()
            {
                // Let WebView2 release its file locks before deleting.
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = std::fs::remove_dir_all(&cleanup_dir);
                break;
            }
        }
    });

    tracing::info!("reCAPTCHA helper window opened");
    Ok(())
}

/// Close the reCAPTCHA helper window. Destroying it fires
/// `recaptcha-cancelled`, which unblocks any pending login.
pub fn close_recaptcha_helper_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window(RECAPTCHA_WINDOW_LABEL) {
        let _ = win.destroy();
    }
}

/// Payload re-emitted to the frontend when a reCAPTCHA token is captured.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecaptchaTokenEvent {
    pub token: String,
    /// `"check"` or `"login"` — which login step this token is for.
    pub step: String,
}

/// Emit the captured reCAPTCHA token to the frontend and close the helper
/// window. Shared by the fragment-poll path (primary) and the
/// `submit_login_token` IPC command (fallback). Guards against double
/// delivery by keying off the helper window still existing.
pub fn deliver_recaptcha_token(app: &tauri::AppHandle, token: String, step: String) {
    use tauri::Emitter;

    // If the helper window is already gone, the token was already delivered.
    let Some(win) = app.get_webview_window(RECAPTCHA_WINDOW_LABEL) else {
        return;
    };

    tracing::info!(token_len = token.len(), step = %step, "reCAPTCHA token delivered");
    if let Err(e) = app.emit("recaptcha-token", RecaptchaTokenEvent { token, step }) {
        tracing::warn!("failed to emit recaptcha-token event: {e}");
    }
    // Mark this close as a successful delivery so on_window_event does NOT emit
    // recaptcha-cancelled (which would abort the next phase).
    RECAPTCHA_DELIVERED.store(true, Ordering::SeqCst);
    let _ = win.destroy();
}
