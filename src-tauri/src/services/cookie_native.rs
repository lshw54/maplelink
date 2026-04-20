//! Native WebView2 COM interop for cookie seeding and popup handling.
//!
//! This module provides two key capabilities that bypass wry/Tauri limitations:
//!
//! 1. **Cookie seeding** — Injects cookies from a reqwest cookie jar into
//!    WebView2 via the native `ICoreWebView2CookieManager` COM API.
//!    This is necessary because wry's `set_cookie` strips the leading dot
//!    from `Domain=.beanfun.com`, causing WebView2's `CreateCookie` to only
//!    match the exact host instead of all subdomains.
//!
//! 2. **NewWindowRequested handler** — Registers a native COM event handler
//!    that intercepts `target="_blank"` / `window.open()` popups and redirects
//!    them to navigate the same WebView2 window. WebView2 blocks popup windows
//!    by default; this handler mirrors the WPF `WebBrowser.xaml.cs` approach.

use std::sync::Arc;

/// A cookie to seed into WebView2: (name, value, domain, path).
pub type SeedCookie = (String, String, String, String);

/// Seed cookies into a WebView2 window using the native CookieManager COM API.
///
/// Unlike `document.cookie` JS injection, this can set HttpOnly cookies and
/// correctly handles `Domain=.beanfun.com` (with leading dot) for subdomain
/// matching.
#[cfg(target_os = "windows")]
pub fn seed_cookies_native(
    webview_window: &tauri::WebviewWindow,
    cookies: &[SeedCookie],
) -> Result<(), String> {
    use std::sync::Mutex;

    if cookies.is_empty() {
        return Ok(());
    }

    let cookies_owned: Vec<SeedCookie> = cookies.to_vec();
    let (tx, rx) = std::sync::mpsc::channel::<Result<usize, String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let result = webview_window.with_webview(move |wv| {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use windows_core::Interface;

        unsafe {
            let controller = wv.controller();
            let core: ICoreWebView2 = controller
                .CoreWebView2()
                .expect("failed to get CoreWebView2");

            let core2: ICoreWebView2_2 = core.cast().expect("failed to cast to ICoreWebView2_2");
            let cookie_manager: ICoreWebView2CookieManager =
                core2.CookieManager().expect("failed to get CookieManager");

            let mut count = 0usize;

            for (name, value, domain, path) in &cookies_owned {
                let h_name = windows_core::HSTRING::from(name.as_str());
                let h_value = windows_core::HSTRING::from(value.as_str());
                let h_domain = windows_core::HSTRING::from(domain.as_str());
                let h_path = windows_core::HSTRING::from(path.as_str());

                match cookie_manager.CreateCookie(&h_name, &h_value, &h_domain, &h_path) {
                    Ok(cookie) => {
                        let _ = cookie_manager.AddOrUpdateCookie(&cookie);
                        count += 1;
                    }
                    Err(e) => {
                        tracing::warn!("CreateCookie failed for {name}: {e}");
                    }
                }
            }

            if let Some(sender) = tx.lock().unwrap().take() {
                let _ = sender.send(Ok(count));
            }
        }
    });

    if result.is_err() {
        return Err("with_webview failed for cookie seeding".to_string());
    }

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(count)) => {
            tracing::info!("seed_cookies_native: seeded {count} cookies");
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("cookie seeding timed out".to_string()),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn seed_cookies_native(
    _webview_window: &tauri::WebviewWindow,
    _cookies: &[SeedCookie],
) -> Result<(), String> {
    Ok(())
}

/// Register a native `NewWindowRequested` event handler on a WebView2 window.
///
/// When the page tries to open a popup (via `target="_blank"`, `window.open()`,
/// etc.), this handler intercepts the request and navigates the current window
/// to the popup URL instead. Completely bypasses wry and mirrors the WPF
/// `WebBrowser.xaml.cs` `NewWindowRequested` handler.
#[cfg(target_os = "windows")]
pub fn register_new_window_handler(webview_window: &tauri::WebviewWindow) -> Result<(), String> {
    use std::sync::Mutex;

    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let result = webview_window.with_webview(move |wv| {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;

        unsafe {
            let controller = wv.controller();
            let core: ICoreWebView2 = controller
                .CoreWebView2()
                .expect("failed to get CoreWebView2");

            // Clone the ICoreWebView2 reference for use inside the closure
            let core_for_nav = core.clone();

            let handler = webview2_com::NewWindowRequestedEventHandler::create(Box::new(
                move |_sender, args| -> windows_core::Result<()> {
                    if let Some(args) = args {
                        let mut uri = windows_core::PWSTR::null();
                        args.Uri(&mut uri)?;
                        let url = uri.to_string().unwrap_or_default();
                        if !uri.is_null() {
                            windows_core::imp::CoTaskMemFree(uri.as_ptr() as _);
                        }

                        if !url.is_empty() && url != "about:blank" {
                            tracing::debug!("NewWindowRequested → navigating to: {url}");
                            let h_url = windows_core::HSTRING::from(url.as_str());
                            let _ = core_for_nav.Navigate(&h_url);
                        }

                        // Mark as handled to prevent WebView2 from opening a popup
                        args.SetHandled(true)?;
                    }
                    Ok(())
                },
            ));

            let mut token: i64 = 0;
            core.add_NewWindowRequested(&handler, &mut token)
                .expect("failed to register NewWindowRequested handler");

            tracing::info!("NewWindowRequested handler registered (token={token})");

            if let Some(sender) = tx.lock().unwrap().take() {
                let _ = sender.send(Ok(()));
            }
        }
    });

    if result.is_err() {
        return Err("with_webview failed for NewWindowRequested handler".to_string());
    }

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("NewWindowRequested handler registration timed out".to_string()),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn register_new_window_handler(_webview_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

/// Register a native `NavigationCompleted` event handler that signals via
/// a tokio oneshot when the next navigation finishes loading.
///
/// Returns a `tokio::sync::oneshot::Receiver<bool>` — the bool is `true` if
/// the navigation succeeded, `false` if it failed. The handler auto-removes
/// itself after the first fire.
#[cfg(target_os = "windows")]
pub fn on_navigation_completed(
    webview_window: &tauri::WebviewWindow,
) -> Result<tokio::sync::oneshot::Receiver<bool>, String> {
    use std::sync::Mutex;

    let (nav_tx, nav_rx) = tokio::sync::oneshot::channel::<bool>();
    let nav_tx = Arc::new(Mutex::new(Some(nav_tx)));

    let (reg_tx, reg_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let reg_tx = Arc::new(Mutex::new(Some(reg_tx)));

    let result = webview_window.with_webview(move |wv| {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;

        unsafe {
            let controller = wv.controller();
            let core: ICoreWebView2 = controller
                .CoreWebView2()
                .expect("failed to get CoreWebView2");

            let nav_tx_clone = nav_tx.clone();

            let handler = webview2_com::NavigationCompletedEventHandler::create(Box::new(
                move |_sender, args| -> windows_core::Result<()> {
                    let success = args
                        .as_ref()
                        .map(|a| {
                            let mut ok = windows_core::BOOL::default();
                            let _ = a.IsSuccess(&mut ok);
                            ok.as_bool()
                        })
                        .unwrap_or(false);

                    if let Some(sender) = nav_tx_clone.lock().unwrap().take() {
                        let _ = sender.send(success);
                    }
                    Ok(())
                },
            ));

            let mut token: i64 = 0;
            core.add_NavigationCompleted(&handler, &mut token)
                .expect("failed to register NavigationCompleted handler");

            tracing::debug!("NavigationCompleted handler registered (token={token})");

            if let Some(sender) = reg_tx.lock().unwrap().take() {
                let _ = sender.send(Ok(()));
            }
        }
    });

    if result.is_err() {
        return Err("with_webview failed for NavigationCompleted handler".to_string());
    }

    match reg_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(nav_rx),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("NavigationCompleted handler registration timed out".to_string()),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn on_navigation_completed(
    _webview_window: &tauri::WebviewWindow,
) -> Result<tokio::sync::oneshot::Receiver<bool>, String> {
    let (_tx, rx) = tokio::sync::oneshot::channel::<bool>();
    Ok(rx)
}

/// Build a list of `SeedCookie` tuples from a reqwest cookie jar.
///
/// Extracts cookies for the given host URLs and ensures the domain has a
/// leading dot for proper subdomain matching in WebView2.
pub fn cookies_from_jar(cookie_jar: &Arc<reqwest::cookie::Jar>, hosts: &[&str]) -> Vec<SeedCookie> {
    use reqwest::cookie::CookieStore;

    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for host in hosts {
        let jar_url: url::Url = match host.parse() {
            Ok(u) => u,
            Err(_) => continue,
        };

        let cookie_header = match cookie_jar.cookies(&jar_url) {
            Some(h) => h,
            None => continue,
        };

        let header_str = match cookie_header.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => continue,
        };

        // Reconstruct domain from the host URL. reqwest jar doesn't expose
        // the original domain attribute, so we derive it from the URL host.
        // Add leading dot for subdomain matching in WebView2.
        let domain = jar_url
            .host_str()
            .map(|h| {
                let clean = h.trim_start_matches("www.");
                if clean.starts_with('.') {
                    clean.to_string()
                } else {
                    format!(".{clean}")
                }
            })
            .unwrap_or_else(|| ".beanfun.com".to_string());

        for pair in header_str.split(';') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            if let Some((name, value)) = pair.split_once('=') {
                let name = name.trim().to_string();
                let value = value.trim().to_string();
                let key = format!("{}@{}", name, domain);
                if seen.insert(key) {
                    result.push((name, value, domain.clone(), "/".to_string()));
                }
            }
        }
    }

    result
}
