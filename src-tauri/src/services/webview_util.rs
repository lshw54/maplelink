//! Shared WebView2 helpers used by the webview-based auth flows.

/// User-Agent for WebView2 windows and HTTP requests.
pub const WEBVIEW_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

/// A cookie tuple: (name, value, domain, path).
pub type CookieTuple = (String, String, String, String);

/// Extract all cookies from a WebView2 window using the native CookieManager API.
/// This reads HttpOnly cookies too (including secure/httponly flags).
#[cfg(target_os = "windows")]
pub async fn extract_webview2_cookies(app: &tauri::AppHandle, label: &str) -> Vec<CookieTuple> {
    use std::sync::{Arc, Mutex};
    use tauri::Manager;

    let Some(webview) = app.get_webview_window(label) else {
        tracing::warn!("GamePass: webview '{}' not found", label);
        return Vec::new();
    };

    let cookies: Arc<Mutex<Vec<CookieTuple>>> = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let cookies_clone = cookies.clone();
    let tx_clone = tx.clone();

    let urls = vec![
        "https://tw.beanfun.com".to_string(),
        "https://login.beanfun.com".to_string(),
        "https://tw.newlogin.beanfun.com".to_string(),
    ];
    let total = urls.len();

    let result = webview.with_webview(move |wv| {
        use webview2_com::{GetCookiesCompletedHandler, Microsoft::Web::WebView2::Win32::*};
        use windows_core::Interface;

        unsafe {
            let controller = wv.controller();
            let core: ICoreWebView2 = controller
                .CoreWebView2()
                .expect("failed to get CoreWebView2");

            let core2: ICoreWebView2_2 = core.cast().expect("failed to cast to ICoreWebView2_2");
            let cookie_manager: ICoreWebView2CookieManager =
                core2.CookieManager().expect("failed to get CookieManager");

            let remaining = Arc::new(Mutex::new(total));

            for url_str in urls {
                let cookies_ref = cookies_clone.clone();
                let remaining_ref = remaining.clone();
                let tx_ref = tx_clone.clone();

                let handler =
                    GetCookiesCompletedHandler::create(Box::new(move |hr, cookie_list| {
                        if hr.is_ok() {
                            if let Some(list) = cookie_list {
                                let mut count: u32 = 0;
                                let _ = list.Count(&mut count);
                                for i in 0..count {
                                    if let Ok(cookie) = list.GetValueAtIndex(i) {
                                        let mut name = windows_core::PWSTR::null();
                                        let mut value = windows_core::PWSTR::null();
                                        let mut domain = windows_core::PWSTR::null();
                                        let mut path = windows_core::PWSTR::null();

                                        let _ = cookie.Name(&mut name);
                                        let _ = cookie.Value(&mut value);
                                        let _ = cookie.Domain(&mut domain);
                                        let _ = cookie.Path(&mut path);

                                        let n = name.to_string().unwrap_or_default();
                                        let v = value.to_string().unwrap_or_default();
                                        let d = domain.to_string().unwrap_or_default();
                                        let p = path.to_string().unwrap_or_default();

                                        // Free the PWSTR strings
                                        if !name.is_null() {
                                            windows_core::imp::CoTaskMemFree(name.as_ptr() as _);
                                        }
                                        if !value.is_null() {
                                            windows_core::imp::CoTaskMemFree(value.as_ptr() as _);
                                        }
                                        if !domain.is_null() {
                                            windows_core::imp::CoTaskMemFree(domain.as_ptr() as _);
                                        }
                                        if !path.is_null() {
                                            windows_core::imp::CoTaskMemFree(path.as_ptr() as _);
                                        }

                                        if !n.is_empty() {
                                            if let Ok(mut c) = cookies_ref.lock() {
                                                c.push((n, v, d, p));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let mut rem = remaining_ref.lock().unwrap();
                        *rem -= 1;
                        if *rem == 0 {
                            if let Some(sender) = tx_ref.lock().unwrap().take() {
                                let _ = sender.send(());
                            }
                        }
                        Ok(())
                    }));

                let url_hstring = windows_core::HSTRING::from(&url_str);
                let _ = cookie_manager.GetCookies(&url_hstring, &handler);
            }
        }
    });

    if result.is_err() {
        tracing::warn!("GamePass: with_webview failed");
        return Vec::new();
    }

    // Wait for all cookie callbacks (timeout 10s)
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), rx).await;

    let result = cookies.lock().unwrap().clone();
    tracing::info!("GamePass: extracted {} total cookies", result.len());
    result
}

#[cfg(not(target_os = "windows"))]
pub async fn extract_webview2_cookies(_app: &tauri::AppHandle, _label: &str) -> Vec<CookieTuple> {
    Vec::new()
}

/// Disable WebView2 Tracking Prevention for a window's profile.
///
/// reCAPTCHA needs third-party storage/cookies on `google.com` / `gstatic.com`;
/// Edge's Tracking Prevention blocks those by default, leaving the widget
/// visible but non-functional. This is a profile-level setting (not a Chromium
/// switch), so it must go through the WebView2 COM API.
#[cfg(target_os = "windows")]
pub fn disable_tracking_prevention(window: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile3, ICoreWebView2_13, COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE,
    };
    use windows_core::Interface;

    let result = window.with_webview(|wv| unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return;
        };
        let Ok(core13) = core.cast::<ICoreWebView2_13>() else {
            tracing::warn!(
                "reCAPTCHA: ICoreWebView2_13 unavailable; cannot disable tracking prevention"
            );
            return;
        };
        let Ok(profile) = core13.Profile() else {
            return;
        };
        let Ok(profile3) = profile.cast::<ICoreWebView2Profile3>() else {
            tracing::warn!(
                "reCAPTCHA: ICoreWebView2Profile3 unavailable; cannot disable tracking prevention"
            );
            return;
        };
        // Set on the profile (persisted in data_directory). The window goes
        // through a redirect chain (default.aspx → checkin → Login/Index)
        // before the reCAPTCHA loads, so this takes effect well before the
        // widget initializes — no risky Reload needed.
        match profile3
            .SetPreferredTrackingPreventionLevel(COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE)
        {
            Ok(()) => tracing::info!("reCAPTCHA: tracking prevention disabled for helper window"),
            Err(e) => tracing::warn!("reCAPTCHA: failed to disable tracking prevention: {e}"),
        }
    });

    if result.is_err() {
        tracing::warn!("reCAPTCHA: with_webview failed; tracking prevention left at default");
    }
}

#[cfg(not(target_os = "windows"))]
pub fn disable_tracking_prevention(_window: &tauri::WebviewWindow) {}
