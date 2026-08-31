//! Native WebView2 navigation control for the Beanfun browser's content view.
//!
//! Tauri exposes `navigate` and `url`, but nothing for the rest of a browser's
//! chrome: there is no back, no forward, and — the part that actually matters —
//! no way to ask whether either is possible. A toolbar whose arrows are always
//! enabled is worse than no arrows, so the state comes from WebView2 itself via
//! `ICoreWebView2::CanGoBack` / `CanGoForward`.
//!
//! Everything here takes a `&tauri::Webview` rather than a `&WebviewWindow`:
//! the content view is a child webview inside a window it shares with the
//! toolbar, so it has no window of its own. `WebviewWindow` is
//! `AsRef<Webview>`, so ordinary popups can still call in with `.as_ref()`.

/// What the toolbar needs to draw itself: where we are, and where we can go.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavState {
    pub url: String,
    pub title: String,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

/// Run `f` against the webview's `ICoreWebView2` and hand back what it returns.
///
/// `with_webview` hops to the main thread and offers no return channel, so the
/// value comes back over a one-shot channel. Callers must therefore not be on
/// the main thread themselves — every caller here is an async command handler,
/// which is not.
#[cfg(target_os = "windows")]
fn with_core<T, F>(webview: &tauri::Webview, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2) -> T + Send + 'static,
{
    use std::sync::{Arc, Mutex};

    let (tx, rx) = std::sync::mpsc::channel::<Result<T, String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let send = tx.clone();
    let posted = webview.with_webview(move |wv| {
        let outcome = unsafe {
            match wv.controller().CoreWebView2() {
                Ok(core) => Ok(f(&core)),
                Err(e) => Err(format!("CoreWebView2 unavailable: {e}")),
            }
        };
        if let Some(sender) = send.lock().unwrap().take() {
            let _ = sender.send(outcome);
        }
    });

    if posted.is_err() {
        return Err("with_webview failed".to_string());
    }

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(inner) => inner,
        Err(_) => Err("webview call timed out".to_string()),
    }
}

/// Take ownership of a COM-allocated string and free the original.
#[cfg(target_os = "windows")]
unsafe fn take_pwstr(raw: windows_core::PWSTR) -> String {
    if raw.is_null() {
        return String::new();
    }
    let out = unsafe { raw.to_string() }.unwrap_or_default();
    unsafe { windows_core::imp::CoTaskMemFree(raw.as_ptr() as _) };
    out
}

/// Read the current URL, title and history availability in one hop.
#[cfg(target_os = "windows")]
pub fn nav_state(webview: &tauri::Webview) -> Result<NavState, String> {
    with_core(webview, |core| unsafe {
        let mut source = windows_core::PWSTR::null();
        let _ = core.Source(&mut source);
        let mut title = windows_core::PWSTR::null();
        let _ = core.DocumentTitle(&mut title);

        let mut back = windows_core::BOOL(0);
        let _ = core.CanGoBack(&mut back);
        let mut forward = windows_core::BOOL(0);
        let _ = core.CanGoForward(&mut forward);

        NavState {
            url: take_pwstr(source),
            title: take_pwstr(title),
            can_go_back: back.as_bool(),
            can_go_forward: forward.as_bool(),
        }
    })
}

/// Go back one entry. A no-op when there is nothing behind us.
#[cfg(target_os = "windows")]
pub fn go_back(webview: &tauri::Webview) -> Result<(), String> {
    with_core(webview, |core| unsafe {
        let _ = core.GoBack();
    })
}

/// Go forward one entry. A no-op when there is nothing ahead.
#[cfg(target_os = "windows")]
pub fn go_forward(webview: &tauri::Webview) -> Result<(), String> {
    with_core(webview, |core| unsafe {
        let _ = core.GoForward();
    })
}

/// Reload the current page.
///
/// Deliberately not `location.reload()`: a page whose script has already thrown
/// still reloads through the native call.
#[cfg(target_os = "windows")]
pub fn reload(webview: &tauri::Webview) -> Result<(), String> {
    with_core(webview, |core| unsafe {
        let _ = core.Reload();
    })
}

/// Navigate to `url`.
///
/// Also native rather than assigning `window.location`, because an injected
/// script is subject to the page's CSP while `ICoreWebView2::Navigate` is not —
/// and beanfun's event pages ship one.
#[cfg(target_os = "windows")]
pub fn navigate(webview: &tauri::Webview, url: &str) -> Result<(), String> {
    // Held for the duration of the call: `PCWSTR` only borrows the buffer.
    let target = windows_core::HSTRING::from(url);
    with_core(webview, move |core| unsafe {
        let _ = core.Navigate(windows_core::PCWSTR(target.as_ptr()));
    })
}

#[cfg(not(target_os = "windows"))]
pub fn nav_state(_webview: &tauri::Webview) -> Result<NavState, String> {
    Ok(NavState::default())
}

#[cfg(not(target_os = "windows"))]
pub fn go_back(_webview: &tauri::Webview) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn go_forward(_webview: &tauri::Webview) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn reload(_webview: &tauri::Webview) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn navigate(_webview: &tauri::Webview, _url: &str) -> Result<(), String> {
    Ok(())
}
