//! Browser extensions in the GamaPass window (Windows only).
//!
//! A password manager's extension acts as the passkey authenticator for
//! GamaPass (see [`super::passkey_manager`]). WebView2 can load unpacked
//! Chromium extensions into a profile, with rules learned the hard way:
//!
//! - An unpacked extension's id derives from its folder path unless the
//!   manifest carries `key`; every `chrome-extension://<id>/…` URL uses the
//!   store id, which only comes back when the CRX's public key is in the
//!   manifest. [`expected_id`] computes it so we know what is loaded.
//! - wry's `extensions_path` calls `AddBrowserExtension` on every window
//!   creation. Re-adding an installed id counts as a reinstall and left the
//!   extension's pages blocked and its service worker unreachable.
//!   [`ensure_loaded`] lists the profile first and adds only what is missing.
//! - A page the extension opens (`chrome.windows.create`) arrives as a
//!   `NewWindowRequested` on whichever of our webviews the extension last
//!   touched, and wry's default is to block it. [`ExtensionHost`] turns those
//!   into MapleLink windows on the same profile, each carrying the same
//!   handler, owned by the GamaPass window, closed when the page closes
//!   itself and when the login finishes.
//! - `AreBrowserExtensionsEnabled` is an environment option, and an
//!   environment is shared by every window on a user data folder for the
//!   life of the process. The extension-enabled profile therefore has its
//!   own folder ([`profile_dir`]) so switching the setting at runtime never
//!   collides with a window already open on the plain profile.

use std::path::{Path, PathBuf};

/// User data folder for the extension-enabled GamaPass profile.
pub fn profile_dir(app_data: &Path) -> PathBuf {
    app_data.join("passkey-profile")
}

/// Label prefix of windows that host extension pages.
pub const HOST_LABEL_PREFIX: &str = "gamepass-ext-";

/// The id Chromium assigns to an unpacked extension whose manifest carries
/// `key` (base64 SPKI): the first 16 bytes of SHA-256(key), each nibble
/// mapped onto `a`–`p`.
pub fn expected_id(manifest_dir: &Path) -> Option<String> {
    use base64::Engine;
    use sha2::Digest;
    let manifest = std::fs::read_to_string(manifest_dir.join("manifest.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&manifest).ok()?;
    let key = json.get("key")?.as_str()?;
    let spki = base64::engine::general_purpose::STANDARD.decode(key).ok()?;
    let digest = sha2::Sha256::digest(&spki);
    Some(super::passkey_manager::crx::letters(&digest[..16]))
}

/// One extension the profile reports.
#[derive(Debug, Clone)]
pub struct LoadedExtension {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

/// Make sure each folder in `dirs` is loaded in `window`'s profile at the
/// version on disk: a folder whose id is missing is added, one whose manifest
/// version differs from what this profile last loaded is removed and added
/// again (a plain re-add of a present id breaks the extension), and the rest
/// are left alone. Returns the list the profile reported *before* any change.
#[cfg(target_os = "windows")]
pub fn ensure_loaded(
    window: &tauri::WebviewWindow,
    profile_dir: &Path,
    dirs: Vec<PathBuf>,
) -> Result<Vec<LoadedExtension>, String> {
    use std::sync::{Arc, Mutex};
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Profile7;
    use webview2_com::{
        BrowserExtensionRemoveCompletedHandler, ProfileAddBrowserExtensionCompletedHandler,
    };

    let profile_dir = profile_dir.to_path_buf();
    with_profile(window, move |profile, present, done| {
        // Report only once every add has completed: a page that asks for the
        // extension before then is blocked as unknown.
        let pending = Arc::new(Mutex::new((0usize, Some(done))));
        let finish_one = {
            let pending = pending.clone();
            move || {
                let mut p = pending.lock().unwrap();
                p.0 = p.0.saturating_sub(1);
                if p.0 == 0 {
                    if let Some(done) = p.1.take() {
                        done();
                    }
                }
            }
        };
        let add = {
            let pending = pending.clone();
            let finish_one = finish_one.clone();
            let profile_dir = profile_dir.clone();
            move |profile: &ICoreWebView2Profile7, dir: PathBuf, version: String| {
                let path = windows_core::HSTRING::from(dir.as_path());
                pending.lock().unwrap().0 += 1;
                let finish = finish_one.clone();
                let dir_for_log = dir.clone();
                let profile_dir = profile_dir.clone();
                let handler = ProfileAddBrowserExtensionCompletedHandler::create(Box::new(
                    move |hr, ext| -> windows_core::Result<()> {
                        match (hr.is_ok(), ext) {
                            (true, Some(ext)) => {
                                let d = unsafe { describe(&ext) };
                                tracing::info!(
                                    "GamePass extensions: added {} {} as {} ({}, {})",
                                    dir_for_log.display(),
                                    version,
                                    d.id,
                                    d.name,
                                    if d.enabled { "on" } else { "OFF" }
                                );
                                record_loaded(&profile_dir, &d.id, &version);
                            }
                            _ => tracing::warn!(
                                "GamePass extensions: AddBrowserExtension({}) failed: {hr:?}",
                                dir_for_log.display()
                            ),
                        }
                        finish();
                        Ok(())
                    },
                ));
                if let Err(e) = unsafe { profile.AddBrowserExtension(&path, &handler) } {
                    tracing::warn!(
                        "GamePass extensions: AddBrowserExtension({}) call failed: {e}",
                        dir.display()
                    );
                    finish_one();
                }
            }
        };

        let loaded_versions = loaded_versions(&profile_dir);
        for dir in dirs {
            let Some(id) = expected_id(&dir) else {
                tracing::warn!(
                    "GamePass extensions: {} has no manifest `key`; skipped",
                    dir.display()
                );
                continue;
            };
            let version = manifest_version(&dir);
            match present.iter().find(|e| e.id == id) {
                None => add(profile, dir, version),
                Some(ext) if loaded_versions.get(&id) == Some(&version) => {
                    tracing::info!(
                        "GamePass extensions: {} {} already loaded as {}",
                        dir.display(),
                        version,
                        id
                    );
                    let _ = ext;
                }
                Some(ext) => {
                    // Version changed on disk: remove the loaded copy, then add.
                    let Some(handle) = &ext.handle else {
                        add(profile, dir, version);
                        continue;
                    };
                    tracing::info!(
                        "GamePass extensions: {} is loaded at {:?}, disk has {}; reloading",
                        id,
                        loaded_versions.get(&id),
                        version
                    );
                    pending.lock().unwrap().0 += 1;
                    let finish = finish_one.clone();
                    let add = add.clone();
                    let profile_again = profile.clone();
                    let id_for_log = id.clone();
                    let handler = BrowserExtensionRemoveCompletedHandler::create(Box::new(
                        move |hr| -> windows_core::Result<()> {
                            if hr.is_ok() {
                                add(&profile_again, dir, version);
                            } else {
                                tracing::warn!(
                                    "GamePass extensions: Remove({id_for_log}) failed: {hr:?}"
                                );
                            }
                            finish();
                            Ok(())
                        },
                    ));
                    if let Err(e) = unsafe { handle.Remove(&handler) } {
                        tracing::warn!("GamePass extensions: Remove call failed: {e}");
                        finish_one();
                    }
                }
            }
        }
        // Nothing to do (or every call failed to start): report now.
        let mut p = pending.lock().unwrap();
        if p.0 == 0 {
            if let Some(done) = p.1.take() {
                done();
            }
        }
    })
}

const LOADED_FILE: &str = "maplelink-loaded.json";

/// `id → version` of what this profile last loaded, kept beside the profile.
fn loaded_versions(profile_dir: &Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(profile_dir.join(LOADED_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn record_loaded(profile_dir: &Path, id: &str, version: &str) {
    let mut map = loaded_versions(profile_dir);
    map.insert(id.to_string(), version.to_string());
    let _ = std::fs::create_dir_all(profile_dir);
    if let Ok(json) = serde_json::to_string_pretty(&map) {
        if let Err(e) = std::fs::write(profile_dir.join(LOADED_FILE), json) {
            tracing::warn!("GamePass extensions: could not record loaded version: {e}");
        }
    }
}

/// The manifest's `version`, or `?` when unreadable.
fn manifest_version(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("manifest.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|j| j.get("version")?.as_str().map(str::to_string))
        .unwrap_or_else(|| "?".to_string())
}

/// Remove `id` from `window`'s profile if it is loaded, so the browser's own
/// WebAuthn prompt is back on the next login.
#[cfg(target_os = "windows")]
pub fn ensure_removed(window: &tauri::WebviewWindow, id: &str) -> Result<bool, String> {
    use webview2_com::BrowserExtensionRemoveCompletedHandler;
    let id = id.to_string();
    let wanted = id.clone();
    let present = with_profile(window, move |_profile, present, done| {
        let targets: Vec<_> = present.iter().filter(|e| e.id == wanted).collect();
        let pending = std::sync::Arc::new(std::sync::Mutex::new((targets.len(), Some(done))));
        let finish_one = {
            let pending = pending.clone();
            move || {
                let mut p = pending.lock().unwrap();
                p.0 = p.0.saturating_sub(1);
                if p.0 == 0 {
                    if let Some(done) = p.1.take() {
                        done();
                    }
                }
            }
        };
        if targets.is_empty() {
            finish_one();
        }
        for ext in targets {
            let Some(handle) = &ext.handle else {
                finish_one();
                continue;
            };
            let id_for_log = ext.id.clone();
            let finish = finish_one.clone();
            let handler = BrowserExtensionRemoveCompletedHandler::create(Box::new(
                move |hr| -> windows_core::Result<()> {
                    finish();
                    if hr.is_ok() {
                        tracing::info!(
                            "GamePass extensions: removed {id_for_log} from the profile"
                        );
                    } else {
                        tracing::warn!("GamePass extensions: Remove({id_for_log}) failed: {hr:?}");
                    }
                    Ok(())
                },
            ));
            if let Err(e) = unsafe { handle.Remove(&handler) } {
                tracing::warn!("GamePass extensions: Remove call failed: {e}");
                finish_one();
            }
        }
    })?;
    Ok(present.iter().any(|e| e.id == id))
}

/// Everything the profile reports, with the COM handle kept for `Remove`.
#[cfg(target_os = "windows")]
struct Reported {
    id: String,
    name: String,
    enabled: bool,
    handle: Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2BrowserExtension>,
}

/// Signals that `then`'s work is complete; the listing is then returned.
#[cfg(target_os = "windows")]
type Done = Box<dyn FnOnce() + Send>;

/// List the profile's extensions on the WebView2 thread, hand the list to
/// `then` (still on that thread, with the profile), and return the list once
/// `then` has called `done` — right away, or from a completion callback.
#[cfg(target_os = "windows")]
fn with_profile<F>(window: &tauri::WebviewWindow, then: F) -> Result<Vec<LoadedExtension>, String>
where
    F: FnOnce(
            &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Profile7,
            &[Reported],
            Done,
        ) + Send
        + 'static,
{
    use std::sync::{Arc, Mutex};
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use webview2_com::ProfileGetBrowserExtensionsCompletedHandler;
    use windows_core::Interface;

    type Reply = Result<Vec<LoadedExtension>, String>;
    let (tx, rx) = std::sync::mpsc::channel::<Reply>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let deliver = |tx: &Arc<Mutex<Option<std::sync::mpsc::Sender<Reply>>>>, r: Reply| {
        if let Some(s) = tx.lock().unwrap().take() {
            let _ = s.send(r);
        }
    };

    let result = window.with_webview(move |wv| unsafe {
        let send = |r: Reply| deliver(&tx, r);
        let tx_inner = tx.clone();
        let core: ICoreWebView2 = match wv.controller().CoreWebView2() {
            Ok(c) => c,
            Err(e) => return send(Err(format!("CoreWebView2: {e}"))),
        };
        let profile: ICoreWebView2Profile7 = match core
            .cast::<ICoreWebView2_13>()
            .and_then(|c| c.Profile())
            .and_then(|p| p.cast::<ICoreWebView2Profile7>())
        {
            Ok(p) => p,
            Err(e) => {
                return send(Err(format!(
                    "Profile7 unavailable (WebView2 runtime too old?): {e}"
                )))
            }
        };
        let profile_for_then = profile.clone();
        let handler = ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(
            move |hr, list| -> windows_core::Result<()> {
                let mut present: Vec<Reported> = Vec::new();
                if hr.is_ok() {
                    if let Some(list) = list {
                        let mut count = 0u32;
                        let _ = list.Count(&mut count);
                        for i in 0..count {
                            if let Ok(ext) = list.GetValueAtIndex(i) {
                                let d = describe(&ext);
                                present.push(Reported {
                                    id: d.id,
                                    name: d.name,
                                    enabled: d.enabled,
                                    handle: Some(ext),
                                });
                            }
                        }
                    }
                } else {
                    tracing::warn!("GamePass extensions: GetBrowserExtensions failed: {hr:?}");
                }
                tracing::info!(
                    "GamePass extensions: profile has {}: {:?}",
                    present.len(),
                    present
                        .iter()
                        .map(|e| format!(
                            "{} {} {}",
                            e.id,
                            e.name,
                            if e.enabled { "on" } else { "OFF" }
                        ))
                        .collect::<Vec<_>>()
                );
                let listing: Vec<LoadedExtension> = present
                    .iter()
                    .map(|e| LoadedExtension {
                        id: e.id.clone(),
                        name: e.name.clone(),
                        enabled: e.enabled,
                    })
                    .collect();
                let done: Done = Box::new(move || deliver(&tx_inner, Ok(listing)));
                then(&profile_for_then, &present, done);
                Ok(())
            },
        ));
        if let Err(e) = profile.GetBrowserExtensions(&handler) {
            send(Err(format!("GetBrowserExtensions call failed: {e}")));
        }
    });
    if result.is_err() {
        return Err("with_webview failed".to_string());
    }
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(r) => r,
        Err(_) => Err("extension listing timed out".to_string()),
    }
}

#[cfg(target_os = "windows")]
unsafe fn describe(
    ext: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2BrowserExtension,
) -> LoadedExtension {
    let take = |get: &dyn Fn(*mut windows_core::PWSTR) -> windows_core::Result<()>| {
        let mut p = windows_core::PWSTR::null();
        let _ = get(&mut p);
        let s = p.to_string().unwrap_or_default();
        if !p.is_null() {
            windows_core::imp::CoTaskMemFree(p.as_ptr() as _);
        }
        s
    };
    let id = take(&|p| ext.Id(p));
    let name = take(&|p| ext.Name(p));
    let mut enabled = windows_core::BOOL(0);
    let _ = ext.IsEnabled(&mut enabled);
    LoadedExtension {
        id,
        name,
        enabled: enabled.as_bool(),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_loaded(
    _window: &tauri::WebviewWindow,
    _profile_dir: &Path,
    _dirs: Vec<PathBuf>,
) -> Result<Vec<LoadedExtension>, String> {
    Err("browser extensions are Windows-only".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_removed(_window: &tauri::WebviewWindow, _id: &str) -> Result<bool, String> {
    Err("browser extensions are Windows-only".to_string())
}

// ---------------------------------------------------------------------------
// Hosting the extension's own pages
// ---------------------------------------------------------------------------

/// Builds MapleLink windows for pages an extension opens (its unlock prompt,
/// its passkey confirmation) on the extension-enabled GamaPass profile.
#[derive(Clone)]
pub struct ExtensionHost {
    app: tauri::AppHandle,
    data_dir: PathBuf,
    browser_args: String,
    /// Label of the window the hosted ones belong to (the GamaPass window).
    owner_label: String,
}

impl ExtensionHost {
    pub fn new(
        app: tauri::AppHandle,
        data_dir: PathBuf,
        browser_args: String,
        owner_label: &str,
    ) -> Self {
        Self {
            app,
            data_dir,
            browser_args,
            owner_label: owner_label.to_string(),
        }
    }

    /// The callback [`super::cookie_native::register_native_popup_handler`]
    /// takes: open the page off the WebView2 thread.
    pub fn opener(&self) -> super::cookie_native::ExtensionPageOpener {
        let host = self.clone();
        std::sync::Arc::new(move |url: String| {
            let host = host.clone();
            tauri::async_runtime::spawn(async move {
                host.open(url, None).await;
            });
        })
    }

    /// Open `url` in a hosted window. `title` defaults to the owner's. Returns
    /// the new window's label.
    pub async fn open(&self, url: String, title: Option<&str>) -> Option<String> {
        use std::sync::atomic::{AtomicU32, Ordering};
        use tauri::Manager;
        static SEQ: AtomicU32 = AtomicU32::new(0);

        let parsed = match url.parse::<url::Url>() {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("GamePass extensions: bad extension page url {url}: {e}");
                return None;
            }
        };
        let keepalive = url.contains(super::passkey_manager::helper::KEEPALIVE_MARK);
        let label = format!("{HOST_LABEL_PREFIX}{}", SEQ.fetch_add(1, Ordering::SeqCst));
        let mut builder = tauri::WebviewWindowBuilder::new(
            &self.app,
            &label,
            tauri::WebviewUrl::External(parsed),
        )
        .title(title.unwrap_or("GamaPass"))
        .inner_size(420.0, 640.0)
        .resizable(true)
        .center()
        // A keepalive page exists only to keep the manager's service worker
        // running (WebView2 does not wake it for a content script); never
        // shown, never in the taskbar.
        .visible(!keepalive)
        .skip_taskbar(keepalive)
        .data_directory(self.data_dir.clone())
        .browser_extensions_enabled(true)
        .additional_browser_args(&self.browser_args);
        if let Some(owner) = self.app.get_webview_window(&self.owner_label) {
            match builder.owner(&owner) {
                Ok(b) => builder = b,
                Err(e) => {
                    tracing::warn!("GamePass extensions: owner for {label}: {e}");
                    return None;
                }
            }
        }
        let window = match builder.build() {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("GamePass extensions: could not open window for {url}: {e}");
                return None;
            }
        };
        tracing::info!("GamePass extensions: opened {label} for {url}");

        if let Err(e) =
            super::cookie_native::register_native_popup_handler(&window, Some(self.opener()))
        {
            tracing::warn!("GamePass extensions: popup handler on {label}: {e}");
        }
        if let Err(e) = super::cookie_native::close_window_when_page_asks(&window) {
            tracing::warn!("GamePass extensions: close handler on {label}: {e}");
        }

        if !keepalive {
            nudge_paint(&window);
        }
        Some(label)
    }
}

/// A window built off the main thread comes up blank until it is resized; a
/// one-pixel bounce a moment after creation (and again a little later, for a
/// page that navigates in) forces the first layout.
pub fn nudge_paint(window: &tauri::WebviewWindow) {
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        for delay in [600u64, 1200] {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            if let Ok(size) = window.inner_size() {
                let _ = window.set_size(tauri::PhysicalSize::new(size.width + 1, size.height));
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                let _ = window.set_size(size);
            }
        }
    });
}

/// Log whether a password manager's page script has taken over
/// `navigator.credentials` in `window`'s page, a little after it opened.
/// Nothing in the login depends on it; it is how a "the manager did not
/// react" report gets diagnosed from the log.
#[cfg(target_os = "windows")]
pub fn log_credentials_override(window: &tauri::WebviewWindow) {
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        for delay in [8u64, 12] {
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            let label = window.label().to_string();
            let res = window.with_webview(move |wv| unsafe {
                use webview2_com::ExecuteScriptCompletedHandler;
                let Ok(core) = wv.controller().CoreWebView2() else {
                    return;
                };
                let js = windows_core::HSTRING::from(
                    "(function(){try{var g=navigator.credentials&&navigator.credentials.get;return JSON.stringify({href:location.href.slice(0,60),get:g?String(g).slice(0,80):'none',name:g&&g.name})}catch(e){return 'ERR '+e}})()",
                );
                let handler = ExecuteScriptCompletedHandler::create(Box::new(
                    move |_hr, text: String| -> windows_core::Result<()> {
                        tracing::info!("GamePass extensions: credentials probe on {label}: {text}");
                        Ok(())
                    },
                ));
                let _ = core.ExecuteScript(&js, &handler);
            });
            if res.is_err() {
                return;
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub fn log_credentials_override(_window: &tauri::WebviewWindow) {}

/// Close every hosted extension window. Called when the GamaPass login
/// finishes or its window goes away: the pages only make sense during it.
pub fn close_all(app: &tauri::AppHandle) {
    use tauri::Manager;
    for (label, window) in app.webview_windows() {
        if label.starts_with(HOST_LABEL_PREFIX) {
            let _ = window.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_id_matches_chromium_for_a_known_key() {
        let dir = std::env::temp_dir().join(format!("maplelink_ext_id_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"name":"x","key":"AAAA"}"#, // base64 of three zero bytes
        )
        .unwrap();
        let id = expected_id(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(id, "hajoiamiieihkcebbobooenpljpcckig");
    }

    #[test]
    fn expected_id_is_none_without_a_key() {
        let dir = std::env::temp_dir().join(format!("maplelink_ext_nokey_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.json"), r#"{"name":"x"}"#).unwrap();
        assert_eq!(expected_id(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn profile_dir_is_separate_from_the_plain_profile() {
        let d = profile_dir(Path::new("C:/data"));
        assert!(d.ends_with("passkey-profile"));
    }
}
