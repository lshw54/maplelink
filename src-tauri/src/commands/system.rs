//! Tauri commands for system-level operations.
//!
//! Includes frontend log forwarding, window resizing on page transitions,
//! native file dialog, and app version retrieval.

use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::models::error::{ErrorCategory, ErrorDto};
use crate::services::web_popup_service;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

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

/// Enable or disable web-login game-launch interception.
///
/// When enabled, registers MapleLink as the beanfun MapleStory launch target
/// so users who can only log in via the website still open the game (with
/// auto-paste) through MapleLink. Disabling restores beanfun's original value.
#[tauri::command]
pub fn set_web_launch_intercept(enabled: bool) -> Result<(), ErrorDto> {
    let result = if enabled {
        crate::services::web_launch::register()
    } else {
        crate::services::web_launch::unregister()
    };
    result.map_err(|e| ErrorDto {
        code: "SYS_WEB_LAUNCH_TOGGLE_FAILED".to_string(),
        message: format!("Failed to update web-launch interception: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })
}

/// Whether web-login game-launch interception is currently active.
#[tauri::command]
pub fn get_web_launch_intercept_status() -> Result<bool, ErrorDto> {
    Ok(crate::services::web_launch::is_registered())
}

/// Self-check snapshot for the web-launch tool UI.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLaunchStatus {
    /// MapleLink is currently registered as beanfun's launch target.
    pub registered: bool,
    /// Configured game path (may be empty).
    pub game_path: String,
    /// The configured game path points at an existing `maplestory.exe`.
    pub game_path_ok: bool,
    /// The Locale Remulator (`LRProc.exe`) is extracted and ready.
    pub lr_ready: bool,
    /// Gamania's official launcher is installed.
    pub gamania_installed: bool,
    /// This app's own exe file name (e.g. `maplelink.exe`).
    pub exe_name: String,
    /// The exe is named one of the expected values (not renamed to something odd).
    pub exe_name_ok: bool,
}

/// Report the readiness of every prerequisite the one-click web-launch tool
/// depends on, so the UI can show the user exactly which step is missing.
#[tauri::command]
pub async fn get_web_launch_status(
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<WebLaunchStatus, ErrorDto> {
    let game_path = state.config.read().await.game_path.clone();
    let game_path_ok = !game_path.trim().is_empty() && std::path::Path::new(&game_path).exists();

    Ok(WebLaunchStatus {
        registered: crate::services::web_launch::is_registered(),
        game_path,
        game_path_ok,
        lr_ready: crate::services::web_launch::lr_ready(),
        gamania_installed: crate::services::web_launch::gamania_installed(),
        exe_name: crate::services::web_launch::exe_name(),
        exe_name_ok: crate::services::web_launch::exe_name_ok(),
    })
}

/// Live launch test — game only: starts the game (via LR), confirms it really
/// opens, then kills it immediately. Skipped if a game is already running.
/// Returns a stable code the UI maps to a message.
#[tauri::command]
pub async fn web_launch_test_game(
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<String, ErrorDto> {
    let game_running = state.is_any_game_running().await;
    Ok(crate::services::web_launch::test_game(game_running).await)
}

/// Live launch test — Gamania launcher only: starts it, confirms it opens, then
/// kills the spawned tree. Returns a stable code the UI maps to a message.
#[tauri::command]
pub async fn web_launch_test_gamania() -> Result<String, ErrorDto> {
    Ok(crate::services::web_launch::test_gamania().await)
}

/// Queue a WebView2 data reset for the next launch by clearing the build marker.
/// The `EBWebView` folders can't be deleted now (the running app's own webview
/// holds the Local copy open), so the actual wipe happens at the next startup in
/// `cleanup_webview_data_on_update` — the caller should restart the app.
#[tauri::command]
pub fn reset_webview_data() -> Result<(), ErrorDto> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let marker = std::path::Path::new(&local)
                .join("com.maplelink.app")
                .join(".webview_build");
            let _ = std::fs::remove_file(marker);
        }
    }
    Ok(())
}

/// Network / DNS diagnostics: public IP + geo, and the active adapter's DNS.
#[tauri::command]
pub async fn get_dns_status(
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<crate::services::network_service::DnsStatus, ErrorDto> {
    let (public_ip, country_code) =
        crate::services::network_service::geo_lookup(&state.http_client).await;
    let current_dns = crate::services::network_service::current_dns();
    let using_recommended = current_dns.iter().any(|d| d == "223.5.5.5");
    Ok(crate::services::network_service::DnsStatus {
        is_china: country_code == "CN",
        public_ip,
        country_code,
        current_dns,
        using_recommended,
    })
}

/// Resolve login.beanfun.com + www.google.com via the current DNS.
#[tauri::command]
pub async fn test_dns() -> Result<crate::services::network_service::DnsTestResult, ErrorDto> {
    Ok(crate::services::network_service::test_resolution().await)
}

/// Switch the active adapter to Alibaba DNS (needs admin → UAC prompt).
#[tauri::command]
pub async fn set_recommended_dns() -> Result<(), ErrorDto> {
    run_dns_change(crate::services::network_service::set_recommended_dns).await
}

/// Revert the active adapter to automatic DNS (needs admin → UAC prompt).
#[tauri::command]
pub async fn reset_dns_auto() -> Result<(), ErrorDto> {
    run_dns_change(crate::services::network_service::reset_dns).await
}

/// Run a blocking, elevation-prompting DNS change off the async runtime and map
/// its outcome (including a declined UAC prompt) to an `ErrorDto`.
async fn run_dns_change(op: fn() -> Result<(), String>) -> Result<(), ErrorDto> {
    let result = tokio::task::spawn_blocking(op)
        .await
        .map_err(|e| ErrorDto {
            code: "SYS_DNS_TASK_FAILED".to_string(),
            message: format!("DNS task failed to run: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })?;
    result.map_err(|e| {
        let cancelled = e == "cancelled";
        ErrorDto {
            code: if cancelled {
                "SYS_DNS_CANCELLED".to_string()
            } else {
                "SYS_DNS_FAILED".to_string()
            },
            message: e,
            category: ErrorCategory::Process,
            details: None,
        }
    })
}

/// Resize the application window for a page transition.
#[tauri::command]
pub async fn resize_window(page: String, window: tauri::Window) -> Result<(), ErrorDto> {
    // The announcement banner is permanent chrome (always shown), so its height
    // is baked into every page's base size — that way it never fights the update
    // banner's dynamic ±height adjustment in the frontend.
    const ANNOUNCEMENT_BAR: f64 = 28.0;
    let (width, height): (f64, f64) = match page.as_str() {
        "login" => (350.0, 620.0 + ANNOUNCEMENT_BAR),
        "login-enlarged" => (540.0, 780.0 + ANNOUNCEMENT_BAR),
        "main" => (760.0, 530.0 + ANNOUNCEMENT_BAR),
        "toolbox" => (750.0, 490.0 + ANNOUNCEMENT_BAR),
        "web_launch" => (560.0, 640.0 + ANNOUNCEMENT_BAR),
        // Temporarily enlarged while the announcement overlay is open so the
        // wide notice card has room (restored to the page size on close).
        "announcement" => (640.0, 700.0),
        _ => {
            return Err(ErrorDto {
                code: "SYS_INVALID_PAGE".to_string(),
                message: format!("Unknown page: {page}"),
                category: ErrorCategory::Configuration,
                details: Some(page),
            });
        }
    };

    // When text-size scaling is active we forced WebView2's scale to the
    // pure DPI value, so we must size the window in physical pixels.
    // Otherwise let Tauri handle it natively with LogicalSize.
    #[cfg(target_os = "windows")]
    {
        let text_scale = crate::get_text_scale_factor();
        if text_scale != 100 {
            let dpi = crate::get_dpi_scale();
            let pw = (width * dpi).round() as u32;
            let ph = (height * dpi).round() as u32;
            window
                .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(pw, ph)))
                .map_err(|e| ErrorDto {
                    code: "SYS_RESIZE_FAILED".to_string(),
                    message: format!("Failed to resize window: {e}"),
                    category: ErrorCategory::Process,
                    details: None,
                })?;
            tracing::debug!("window {pw}×{ph} (physical, dpi={dpi}) page='{page}'");
        } else {
            window
                .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
                .map_err(|e| ErrorDto {
                    code: "SYS_RESIZE_FAILED".to_string(),
                    message: format!("Failed to resize window: {e}"),
                    category: ErrorCategory::Process,
                    details: None,
                })?;
            tracing::debug!("window {width}×{height} page='{page}'");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
            .map_err(|e| ErrorDto {
                code: "SYS_RESIZE_FAILED".to_string(),
                message: format!("Failed to resize window: {e}"),
                category: ErrorCategory::Process,
                details: None,
            })?;
        tracing::debug!("window {width}×{height} page='{page}'");
    }

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

/// Export saved accounts + display overrides to a portable file the user picks.
/// `passphrase = None` writes plaintext JSON; `Some(p)` writes an AES-256-GCM
/// encrypted envelope. Returns `false` if the user cancelled the save dialog.
#[tauri::command]
pub async fn export_data(
    passphrase: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<bool, ErrorDto> {
    use crate::services::data_transfer::{build_export, ExportPayload};

    let payload = ExportPayload {
        accounts: state.saved_accounts.read().await.clone(),
        display_overrides: state.display_overrides.read().await.clone(),
    };
    let contents = build_export(&payload, passphrase.as_deref()).map_err(|e| ErrorDto {
        code: "SYS_EXPORT_FAILED".to_string(),
        message: e,
        category: ErrorCategory::Process,
        details: None,
    })?;

    let default_name = format!(
        "maplelink-backup-{}.json",
        chrono::Local::now().format("%Y%m%d")
    );
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_title("Export MapleLink data")
        .set_file_name(&default_name)
        .save_file(move |path| {
            let _ = tx.send(path.map(|p| p.to_string()));
        });
    let path = rx.await.map_err(|_| ErrorDto {
        code: "SYS_DIALOG_FAILED".to_string(),
        message: "Save dialog was cancelled unexpectedly".to_string(),
        category: ErrorCategory::Process,
        details: None,
    })?;
    let Some(path) = path else {
        return Ok(false);
    };

    tokio::fs::write(&path, contents)
        .await
        .map_err(|e| ErrorDto {
            code: "SYS_EXPORT_WRITE_FAILED".to_string(),
            message: format!("failed to write export file: {e}"),
            category: ErrorCategory::FileSystem,
            details: None,
        })?;
    tracing::info!("exported {} accounts to {path}", payload.accounts.len());
    Ok(true)
}

/// Open a file picker to choose a backup to import. Returns the path, or `None`
/// if cancelled.
#[tauri::command]
pub async fn open_import_dialog(app: tauri::AppHandle) -> Result<Option<String>, ErrorDto> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_title("Import MapleLink data")
        .pick_file(move |path| {
            let _ = tx.send(path.map(|p| p.to_string()));
        });
    rx.await.map_err(|_| ErrorDto {
        code: "SYS_DIALOG_FAILED".to_string(),
        message: "File dialog was cancelled unexpectedly".to_string(),
        category: ErrorCategory::Process,
        details: None,
    })
}

/// Import accounts + display overrides from a backup file at `path`. Merges into
/// existing data (imported entries upsert by region+account). Returns the number
/// of accounts imported. Error code `IMPORT_PASSPHRASE_REQUIRED` when the file is
/// encrypted and no passphrase was given; `IMPORT_WRONG_PASSPHRASE` on a bad one.
#[tauri::command]
pub async fn import_data(
    path: String,
    passphrase: Option<String>,
    disposal: String,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<usize, ErrorDto> {
    let result = do_import(&path, passphrase.as_deref(), state.inner()).await;

    // Handle the source backup file once the import concludes (it can contain
    // plaintext passwords). Keep it ONLY while a passphrase retry may still need
    // to re-read it (encrypted file, missing / wrong passphrase); otherwise apply
    // the user's chosen disposal: "delete" (permanent), "recycle" (OS trash), or
    // "keep".
    let retryable = matches!(
        result.as_ref().err().map(|e| e.code.as_str()),
        Some("IMPORT_PASSPHRASE_REQUIRED") | Some("IMPORT_WRONG_PASSPHRASE")
    );
    if !retryable {
        dispose_import_file(&path, &disposal).await;
    }
    result
}

/// Apply the user's chosen disposal to the imported backup file.
async fn dispose_import_file(path: &str, disposal: &str) {
    match disposal {
        "keep" => {}
        "recycle" => {
            let p = path.to_string();
            let res = tokio::task::spawn_blocking(move || trash::delete(&p)).await;
            if let Ok(Err(e)) = res {
                tracing::warn!("could not move import file {path} to recycle bin: {e}");
            }
        }
        // "delete" (and any unexpected value) → permanent delete.
        _ => {
            if let Err(e) = tokio::fs::remove_file(path).await {
                tracing::warn!("could not delete import file {path}: {e}");
            }
        }
    }
}

async fn do_import(
    path: &str,
    passphrase: Option<&str>,
    state: &crate::models::app_state::AppState,
) -> Result<usize, ErrorDto> {
    let contents = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| ErrorDto {
            code: "SYS_IMPORT_READ_FAILED".to_string(),
            message: format!("failed to read backup file: {e}"),
            category: ErrorCategory::FileSystem,
            details: None,
        })?;

    let payload =
        crate::services::data_transfer::parse_import(&contents, passphrase).map_err(|e| {
            let code = match e.as_str() {
                "PASSPHRASE_REQUIRED" => "IMPORT_PASSPHRASE_REQUIRED",
                "WRONG_PASSPHRASE" => "IMPORT_WRONG_PASSPHRASE",
                _ => "SYS_IMPORT_PARSE_FAILED",
            };
            ErrorDto {
                code: code.to_string(),
                message: e,
                category: ErrorCategory::Configuration,
                details: None,
            }
        })?;

    // Merge saved accounts (imported upsert by region + account).
    {
        let mut accounts = state.saved_accounts.write().await;
        for imported in &payload.accounts {
            accounts.retain(|a| !(a.region == imported.region && a.account == imported.account));
            accounts.push(imported.clone());
        }
        crate::services::account_storage::save_accounts(&state.accounts_path, &accounts)
            .await
            .map_err(|e| ErrorDto {
                code: "SYS_IMPORT_SAVE_FAILED".to_string(),
                message: e,
                category: ErrorCategory::FileSystem,
                details: None,
            })?;
    }

    // Merge display overrides (names override; order replaced if the import has one).
    {
        let mut ov = state.display_overrides.write().await;
        for (k, v) in &payload.display_overrides.names {
            ov.names.insert(k.clone(), v.clone());
        }
        if !payload.display_overrides.order.is_empty() {
            ov.order = payload.display_overrides.order.clone();
        }
        crate::services::account_storage::save_display_overrides(&state.overrides_path, &ov)
            .await
            .map_err(|e| ErrorDto {
                code: "SYS_IMPORT_SAVE_FAILED".to_string(),
                message: e,
                category: ErrorCategory::FileSystem,
                details: None,
            })?;
    }

    tracing::info!("imported {} accounts from {path}", payload.accounts.len());
    Ok(payload.accounts.len())
}

/// Return the application version from `Cargo.toml`.
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Return the Windows Accessibility "Text size" percentage (default 100).
/// The frontend uses this to apply an inverse CSS zoom so layout is not
/// broken by text-size scaling.
#[tauri::command]
pub fn get_text_scale_factor() -> u32 {
    #[cfg(target_os = "windows")]
    {
        crate::get_text_scale_factor()
    }
    #[cfg(not(target_os = "windows"))]
    {
        100
    }
}

/// Return a human-readable platform string, e.g. "Windows 11 (x64)".
/// Reads the actual OS build from the registry for accurate Win10/11 detection.
#[tauri::command]
pub fn get_platform_info() -> String {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm
            .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
            .ok();

        let build: u32 = key
            .as_ref()
            .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber").ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let win_ver = if build >= 22000 {
            "Windows 11"
        } else {
            "Windows 10"
        };

        let arch = if std::mem::size_of::<usize>() == 8 {
            "x64"
        } else {
            "x86"
        };

        format!("{win_ver} ({arch})")
    }
    #[cfg(not(target_os = "windows"))]
    {
        "Unknown".to_string()
    }
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

/// Open the Beanfun gash (top-up / buy points) popup.
#[tauri::command]
pub async fn open_gash_popup(
    session_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<(), ErrorDto> {
    web_popup_service::open_gash_popup(session_id, app, state.inner()).await
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
#[tauri::command]
pub async fn open_member_popup(
    session_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<(), ErrorDto> {
    web_popup_service::open_member_popup(session_id, app, state.inner()).await
}

/// Open the region-appropriate customer-service page in a public popup.
#[tauri::command]
pub async fn open_customer_service(
    session_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<(), ErrorDto> {
    web_popup_service::open_customer_service(session_id, app, state.inner()).await
}

/// Open an authenticated WebView popup with cookie seeding.
///
/// Used for pages that require beanfun login cookies (e.g. report pages).
#[tauri::command]
pub async fn open_auth_popup(
    session_id: String,
    url: String,
    title: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<(), ErrorDto> {
    web_popup_service::open_auth_popup(session_id, url, title, app, state.inner()).await
}

/// Open a simple WebView popup window for a given URL (no auth needed).
#[tauri::command]
pub async fn open_web_popup(
    url: String,
    title: String,
    app: tauri::AppHandle,
) -> Result<(), ErrorDto> {
    web_popup_service::open_web_popup(url, title, app).await
}

/// Get the bfWebToken from the cookie jar for constructing authenticated URLs.
#[tauri::command]
pub async fn get_web_token(
    session_id: String,
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<String, ErrorDto> {
    let ss = state.require_session(&session_id).await?;
    web_popup_service::web_token_from_jar(&ss.cookie_jar, state.inner()).await
}

/// Act on the user's window-close choice from the "quit vs. minimize to tray"
/// dialog. `action` is "quit" or "tray". Remembering the choice is done on the
/// frontend via `set_config` (close_behavior) before calling this.
#[tauri::command]
pub async fn resolve_app_close(action: String, app: tauri::AppHandle) -> Result<(), ErrorDto> {
    if action == "tray" {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    } else {
        crate::request_quit(&app);
    }
    Ok(())
}

/// Whether the given announcement id has already been read-and-dismissed.
#[tauri::command]
pub async fn announcement_is_seen(id: String, app: tauri::AppHandle) -> Result<bool, ErrorDto> {
    let dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;
    Ok(crate::services::announcement_service::is_seen(&dir, &id))
}

/// Persist that the given announcement id has been read-and-dismissed.
#[tauri::command]
pub async fn announcement_mark_seen(id: String, app: tauri::AppHandle) -> Result<(), ErrorDto> {
    let dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;
    crate::services::announcement_service::mark_seen(&dir, &id).map_err(|e| ErrorDto {
        code: "SYS_ANNOUNCEMENT_SAVE_FAILED".to_string(),
        message: e,
        category: ErrorCategory::FileSystem,
        details: None,
    })
}

/// Fetch the official MapleStory TW client download list (full client + update
/// patches). Read-only: returns official links only — MapleLink never downloads
/// or replaces client files itself (issue #21).
#[tauri::command]
pub async fn get_game_download_list(
) -> Result<Vec<crate::services::game_download::GameDownloadItem>, ErrorDto> {
    crate::services::game_download::fetch_download_list()
        .await
        .map_err(|e| ErrorDto {
            code: "SYS_DOWNLOAD_LIST_FAILED".to_string(),
            message: e,
            category: ErrorCategory::Network,
            details: None,
        })
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
