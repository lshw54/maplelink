//! MapleLink — Beanfun launcher built on Tauri v2.
//!
//! Architecture layers:
//!   commands/ → core/ → services/ → models/
//!
//! Commands are thin Tauri IPC handlers.
//! Core contains pure business logic.
//! Services encapsulate all side effects.
//! Models define shared DTOs and domain structs.

pub mod commands;
pub mod core;
pub mod models;
pub mod services;
pub mod utils;

use std::collections::HashMap;

use tauri::Emitter;
use tauri::Manager;

use models::app_state::AppState;
use models::config::AppConfig;
use services::{account_storage, config_service, update_service};

/// Read the Windows Accessibility "Text size" percentage from the registry.
/// Returns 100 when the user has not changed it (the default).
#[cfg(target_os = "windows")]
fn get_text_scale_factor() -> u32 {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey("SOFTWARE\\Microsoft\\Accessibility")
        .and_then(|key| key.get_value::<u32, _>("TextScaleFactor"))
        .unwrap_or(100)
}

/// Return the primary monitor's DPI scale factor (e.g. 1.0, 1.25, 1.5, 2.0).
/// This is the *display* DPI only — it does NOT include the text-size multiplier.
#[cfg(target_os = "windows")]
fn get_dpi_scale() -> f64 {
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() };
    if dpi == 0 {
        1.0
    } else {
        dpi as f64 / 96.0
    }
}

/// Check if the current process is running with admin privileges.
#[cfg(target_os = "windows")]
fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);

        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// Delete cached WebView2 data (the `EBWebView` folders) once after each app
/// update, so stale cached behaviour can't survive an update. Keyed on the
/// executable's modification time, which changes when the installer replaces the
/// exe. Runs at startup BEFORE the WebView2 window is created (otherwise the
/// in-use Local copy is locked). Only the `EBWebView` subfolders are touched —
/// never the parent dirs, which hold accounts / config / logs.
#[cfg(target_os = "windows")]
fn cleanup_webview_data_on_update() {
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return;
    };
    let app_local = std::path::Path::new(&local).join("com.maplelink.app");

    // Build ID = exe mtime (seconds since epoch); changes on every update.
    let build_id = std::env::current_exe()
        .and_then(std::fs::metadata)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    if build_id.is_empty() {
        return;
    }

    // `.webview_build` marker (deleting it via the reset command forces a clean).
    let marker = app_local.join(".webview_build");
    if std::fs::read_to_string(&marker).unwrap_or_default().trim() == build_id {
        return; // already cleaned for this build
    }

    let mut targets = vec![app_local.join("EBWebView")];
    if let Ok(roaming) = std::env::var("APPDATA") {
        targets.push(
            std::path::Path::new(&roaming)
                .join("com.maplelink.app")
                .join("EBWebView"),
        );
    }
    for t in targets {
        if t.exists() {
            match std::fs::remove_dir_all(&t) {
                Ok(()) => tracing::info!("cleaned WebView2 data: {}", t.display()),
                Err(e) => tracing::warn!("could not clean WebView2 data {}: {e}", t.display()),
            }
        }
    }

    let _ = std::fs::create_dir_all(&app_local);
    let _ = std::fs::write(&marker, &build_id);
}

/// Initialise and run the Tauri application.
///
/// Startup sequence:
/// 1. Register plugins (dialog, fs, shell)
/// 2. Register all Tauri command handlers
/// 3. `.setup()`:
///    a. Load config from disk (create default if missing)
///    b. Initialise structured file + console logging
///    c. Initialise `AppState` with loaded config
///    d. Check for auto-update (non-blocking, respects config toggle)
/// 4. Window starts at login size (340×520) — defined in `tauri.conf.json5`
pub fn run() {
    // Web-login game-launch interception. If beanfun invoked us as the "game"
    // (HKCU\SOFTWARE\Gamania\MapleStory\PATH → MapleLink), handle it headlessly
    // and exit — never start the UI or self-elevate. See core::game_intercept.
    {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        // The helper .bat invokes us as `--web-launch <beanfun args>`; strip the
        // tag (and remember we came from the .bat, so we stay quiet).
        let (params, quiet) = match raw.split_first() {
            Some((first, rest)) if first == "--web-launch" => (rest.to_vec(), true),
            _ => (raw, false),
        };
        if let Some(creds) = core::game_intercept::parse_intercept_args(&params) {
            // Best-effort file logging for this headless path.
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let log_dir = std::path::Path::new(&local)
                    .join("com.maplelink.app")
                    .join("logs");
                let _ = services::log_service::init_logging(&log_dir);
            }
            services::web_launch::run_intercept(creds, quiet);
            return;
        }
    }

    // Wipe stale WebView2 caches once per update (before the webview starts).
    #[cfg(target_os = "windows")]
    cleanup_webview_data_on_update();

    // Neutralise Windows Accessibility "Text size" setting.
    //
    // Windows has two independent scaling knobs:
    //   1. Display scale / DPI  (e.g. 150%) — scales everything uniformly.
    //   2. Accessibility → Text size (e.g. 120%) — scales *only* text.
    //
    // WebView2 honours both.  (2) breaks our fixed layout because only text
    // grows while containers stay the same size.
    //
    // Fix: force WebView2's device-scale-factor to the pure DPI value,
    // which excludes the text-size multiplier.  Combined with PhysicalSize
    // window sizing (DPI × design size), the app renders identically
    // regardless of the text-size slider.
    #[cfg(target_os = "windows")]
    {
        // Only force the device scale factor when the user has changed the
        // Windows Accessibility text-size slider (> 100%).  When text scale
        // is at the default 100%, Tauri + WebView2 handle DPI natively and
        // we must not interfere.
        let text_scale = get_text_scale_factor();
        if text_scale != 100 {
            // Must declare DPI awareness BEFORE reading DPI, otherwise Windows
            // may virtualise the value to 96.
            unsafe {
                windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
                    windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                );
            }

            let dpi_scale = get_dpi_scale();
            let arg = format!("--force-device-scale-factor={dpi_scale}");
            match std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS") {
                Ok(existing) if !existing.contains("--force-device-scale-factor") => {
                    std::env::set_var(
                        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                        format!("{existing} {arg}"),
                    );
                }
                Err(_) => {
                    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", &arg);
                }
                _ => {}
            }
        }
    }

    // Self-elevate to admin if not already elevated.
    // Required for auto-paste (PostMessage to game window) and LR DLL injection.
    #[cfg(target_os = "windows")]
    {
        if !is_elevated() {
            let exe = std::env::current_exe().expect("failed to get current exe path");
            let exe_str = exe.to_string_lossy();

            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            fn to_wide(s: &str) -> Vec<u16> {
                OsStr::new(s)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect()
            }

            let verb = to_wide("runas");
            let file = to_wide(&exe_str);
            let result = unsafe {
                windows_sys::Win32::UI::Shell::ShellExecuteW(
                    std::ptr::null_mut(),
                    verb.as_ptr(),
                    file.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                )
            };
            if result as usize > 32 {
                // Successfully launched elevated instance, exit this one
                std::process::exit(0);
            }
            // If ShellExecute failed (user cancelled UAC), continue without admin
        }
    }

    tauri::Builder::default()
        // -- Plugins --------------------------------------------------------
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        // -- Command handlers -----------------------------------------------
        .invoke_handler(tauri::generate_handler![
            commands::auth::create_session,
            commands::auth::list_sessions,
            commands::auth::login,
            commands::auth::tw_login_check,
            commands::auth::tw_login_submit,
            commands::auth::qr_login_start,
            commands::auth::qr_login_poll,
            commands::auth::totp_verify,
            commands::auth::get_advance_check,
            commands::auth::submit_advance_check,
            commands::auth::refresh_advance_check_captcha,
            commands::auth::logout,
            commands::auth::refresh_session,
            commands::auth::get_saved_accounts,
            commands::auth::get_all_saved_accounts,
            commands::auth::get_last_saved_account,
            commands::auth::get_saved_account_detail,
            commands::auth::save_verify_info,
            commands::auth::delete_saved_account,
            commands::auth::save_login_credentials,
            commands::auth::session_key_webview_done,
            commands::config::get_config,
            commands::config::set_config,
            commands::config::reset_config,
            commands::account::get_game_accounts,
            commands::account::get_account_create_time,
            commands::account::get_game_credentials,
            commands::account::refresh_accounts,
            commands::account::ping_session,
            commands::account::get_remain_point,
            commands::account::auto_paste_otp,
            commands::account::change_account_display_name,
            commands::account::set_display_override,
            commands::account::set_account_order,
            commands::account::get_display_overrides,
            commands::account::get_auth_email,
            commands::launcher::launch_game,
            commands::launcher::launch_game_direct,
            commands::launcher::is_game_running,
            commands::launcher::get_game_pid,
            commands::launcher::get_process_status,
            commands::launcher::kill_game,
            commands::system::log_frontend_error,
            commands::system::set_web_launch_intercept,
            commands::system::get_web_launch_intercept_status,
            commands::system::get_web_launch_status,
            commands::system::web_launch_test_game,
            commands::system::web_launch_test_gamania,
            commands::system::reset_webview_data,
            commands::system::get_dns_status,
            commands::system::test_dns,
            commands::system::set_recommended_dns,
            commands::system::reset_dns_auto,
            commands::system::resize_window,
            commands::system::open_file_dialog,
            commands::system::get_app_version,
            commands::system::get_text_scale_factor,
            commands::system::get_platform_info,
            commands::system::detect_game_path,
            commands::system::toggle_debug_window,
            commands::system::open_log_folder,
            commands::system::open_external,
            commands::system::get_recent_logs,
            commands::system::open_web_popup,
            commands::system::open_gash_popup,
            commands::system::resize_gash_popup,
            commands::system::open_member_popup,
            commands::system::open_customer_service,
            commands::system::open_auth_popup,
            commands::system::get_web_token,
            commands::system::cleanup_game_cache,
            commands::system::check_beanfun_rename,
            commands::system::apply_beanfun_rename,
            commands::system::get_game_download_list,
            commands::system::announcement_is_seen,
            commands::system::announcement_mark_seen,
            commands::system::resolve_app_close,
            commands::system::export_data,
            commands::system::open_import_dialog,
            commands::system::import_data,
            commands::auth::open_gamepass_login,
            commands::auth::gamepass_webview_done,
            commands::auth::open_regular_web_login,
            commands::auth::regular_web_login_done,
            commands::auth::open_recaptcha_window,
            commands::auth::submit_login_token,
            commands::auth::close_recaptcha_window,
            commands::update::check_update,
            commands::update::apply_update,
            commands::update::test_github_access,
            commands::update::restart_app,
        ])
        // -- Setup (startup sequence) ---------------------------------------
        .setup(|app| {
            // 1. Initialise structured file + console logging.
            let log_dir = app
                .path()
                .app_log_dir()
                .expect("failed to resolve app log directory");
            if let Err(e) = services::log_service::init_logging(&log_dir) {
                eprintln!("WARNING: failed to initialise file logging: {e}");
                // Fall back to a basic console-only subscriber so tracing
                // macros still work.
                tracing_subscriber::fmt()
                    .with_env_filter(
                        tracing_subscriber::EnvFilter::try_from_default_env()
                            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                    )
                    .init();
            }

            tracing::info!("Starting MapleLink v{}", env!("CARGO_PKG_VERSION"));
            // Diagnostic: log the raw launch args. When beanfun web-launches us
            // as the game but the args don't match core::game_intercept, we land
            // here (normal UI) instead of intercepting — this shows their real
            // format so the parser can be aligned.
            let startup_args: Vec<String> = std::env::args().skip(1).collect();
            if !startup_args.is_empty() {
                tracing::info!("startup args (not intercepted): {startup_args:?}");
            }

            // Clean up old exe from self-replace update
            if let Ok(exe) = std::env::current_exe() {
                let old = exe.with_extension("exe.old");
                if old.exists() {
                    let _ = std::fs::remove_file(&old);
                    tracing::info!("cleaned up old exe: {}", old.display());
                }
            }
            tracing::info!("log directory: {}", log_dir.display());
            // Elevated means every process we spawn inherits the admin token —
            // external links must go through the shell, never straight from us.
            #[cfg(target_os = "windows")]
            tracing::info!("running elevated: {}", is_elevated());

            // 2. Load config from disk (create default if missing).
            let config_dir = app
                .path()
                .app_config_dir()
                .expect("failed to resolve app config directory");
            let config_path = config_dir.join("config.ini");

            let config = tauri::async_runtime::block_on(async {
                config_service::ensure_default_config(&config_path)
                    .await
                    .ok();
                config_service::load_config(&config_path)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!("failed to load config, using defaults: {e}");
                        AppConfig::default()
                    })
            });

            tracing::info!("config loaded from {}", config_path.display());

            // 3. Load saved accounts from disk.
            let accounts_path = config_dir.join("accounts.json");
            let saved_accounts = tauri::async_runtime::block_on(async {
                account_storage::load_accounts(&accounts_path).await
            });
            tracing::info!(
                "loaded {} saved accounts from {}",
                saved_accounts.len(),
                accounts_path.display()
            );

            let overrides_path = config_dir.join("display_overrides.json");
            let display_overrides = tauri::async_runtime::block_on(async {
                account_storage::load_display_overrides(&overrides_path).await
            });
            tracing::info!(
                "loaded {} display overrides from {}",
                display_overrides.names.len(),
                overrides_path.display()
            );

            // 4. Initialise AppState with loaded config.
            let auto_update_enabled = config.auto_update;
            let update_channel = config.update_channel.clone();
            let http_client = reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .expect("failed to build HTTP client");
            let update_client = http_client.clone();

            let state = AppState {
                sessions: tokio::sync::RwLock::new(HashMap::new()),
                config: tokio::sync::RwLock::new(config),
                config_path,
                saved_accounts: tokio::sync::RwLock::new(saved_accounts),
                accounts_path,
                overrides_path,
                display_overrides: tokio::sync::RwLock::new(display_overrides),
                http_client,
            };

            app.manage(state);

            // 4a. Auto-detect game path on first launch (if not set).
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let needs_detect = state.config.read().await.game_path.is_empty();
                    if !needs_detect {
                        return;
                    }
                    match commands::system::detect_game_path_inner(&state).await {
                        Ok(Some(path)) => {
                            let mut config = state.config.write().await;
                            config.game_path = path.clone();
                            let _ = config_service::save_config(&state.config_path, &config).await;
                            tracing::info!("auto-detected game path on startup: {path}");
                        }
                        Ok(None) => {
                            tracing::debug!("no game path detected on startup");
                        }
                        Err(e) => {
                            tracing::warn!("game path detection failed on startup: {:?}", e);
                        }
                    }
                });
            }

            // 4b. Auto-update check (non-blocking background task).
            if update_service::should_check(false, auto_update_enabled) {
                let app_handle_for_update = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Delay to ensure WebView2 is fully rendered before
                    // making network requests that may contend with the
                    // TLS/network stack during initial page load.
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    let version = update_service::current_version();
                    let include_prerelease =
                        update_channel == models::config::UpdateChannel::PreRelease;
                    match update_service::check_for_update(
                        &update_client,
                        version,
                        include_prerelease,
                    )
                    .await
                    {
                        Ok(Some(info)) => {
                            tracing::info!(
                                "update available: v{} — user will be notified",
                                info.version
                            );
                            // Emit event so frontend can show update dialog
                            if let Some(window) = app_handle_for_update.get_webview_window("main") {
                                let _ = window.emit("update-available", &info);
                            }
                        }
                        Ok(None) => {
                            tracing::info!("no update available, app is up-to-date");
                        }
                        Err(e) => {
                            tracing::warn!("startup update check failed (non-fatal): {e}");
                        }
                    }
                });
            } else {
                tracing::info!("auto-update disabled, skipping startup check");
            }

            // 5. Pre-extract LR files so they're ready for game launch.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match services::lr_service::ensure_lr_files(&app_handle).await {
                    Ok(path) => tracing::info!("LR files ready at {}", path.display()),
                    Err(e) => tracing::warn!("failed to extract LR files: {e}"),
                }
            });

            // 6. Backend ping loop — keeps every logged-in session alive (~60s).
            let ping_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait for at least one session to exist
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    if let Some(state) = ping_app.try_state::<AppState>() {
                        if !state.sessions.read().await.is_empty() {
                            break;
                        }
                    }
                }
                tracing::info!("backend ping loop started");

                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    if let Some(state) = ping_app.try_state::<AppState>() {
                        let sessions = state.sessions.read().await;
                        if sessions.is_empty() {
                            tracing::info!("backend ping loop stopped (no sessions)");
                            break;
                        }
                        for ss in sessions.values() {
                            let region = {
                                let session = ss.session.read().await;
                                session.as_ref().map(|s| s.region.clone())
                            };
                            if let Some(region) = region {
                                if let Ok(_guard) = ss.bf_client_lock.try_lock() {
                                    services::beanfun_service::ping(&ss.http_client, &region).await;
                                }
                            }
                        }
                    }
                }
            });

            // Only resize the initial window when text-scale compensation
            // is active (force-device-scale-factor was set).  Otherwise
            // tauri.conf.json5's logical 350×620 is correct as-is.
            #[cfg(target_os = "windows")]
            {
                let text_scale = get_text_scale_factor();
                if text_scale != 100 {
                    let dpi = get_dpi_scale();
                    let pw = (350.0 * dpi).round() as u32;
                    let ph = (620.0 * dpi).round() as u32;
                    if let Some(win) = app.get_webview_window("main") {
                        let _ =
                            win.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(pw, ph)));
                        let _ = win.center();
                        tracing::info!(
                            "initial window {pw}×{ph} physical (dpi={dpi}, text_scale={text_scale}%)"
                        );
                    }
                }
            }

            // System-tray icon (lets "minimize to tray" on close work).
            if let Err(e) = setup_tray(app.handle()) {
                tracing::warn!("failed to create system tray: {e}");
            }

            tracing::info!("startup complete — showing login page");
            Ok(())
        })
        // -- Window lifecycle -----------------------------------------------
        .on_window_event(|window, event| {
            // Strip the Windows 11 DWM frame border (the accent-coloured top
            // hairline) + round the corners. Windows RESTORES the border after a
            // resize/move (e.g. resize_window on page navigation), and those
            // don't fire Focused, so re-apply on those events too — otherwise the
            // top border reappears and stays until the next focus change.
            #[cfg(target_os = "windows")]
            if matches!(
                event,
                tauri::WindowEvent::Focused(true)
                    | tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::Moved(_)
            ) {
                apply_borderless_dwm(window);
            }

            // Intercept the main window close (titlebar X or Alt+F4) and honour
            // the configured behaviour: quit, minimize to tray, or ask.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main"
                    && !QUIT_REQUESTED.load(std::sync::atomic::Ordering::SeqCst)
                {
                    let state_opt = window.app_handle().try_state::<AppState>();
                    // Café / shared-PC mode overrides the close behaviour entirely:
                    // wipe every local trace and quit (never minimise to tray, which
                    // would keep the session resident).
                    let cafe = state_opt
                        .as_ref()
                        .and_then(|s| s.config.try_read().ok().map(|c| c.cafe_mode))
                        .unwrap_or(false);
                    if cafe {
                        api.prevent_close();
                        if let Some(state) = state_opt.as_ref() {
                            services::cafe_service::wipe_local_data(
                                window.app_handle(),
                                state.inner(),
                            );
                        }
                        request_quit(window.app_handle());
                        return;
                    }

                    let behavior = state_opt
                        .and_then(|s| s.config.try_read().ok().map(|c| c.close_behavior))
                        .unwrap_or(models::config::CloseBehavior::Ask);
                    match behavior {
                        models::config::CloseBehavior::Quit => {
                            api.prevent_close();
                            request_quit(window.app_handle());
                        }
                        models::config::CloseBehavior::Tray => {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                        models::config::CloseBehavior::Ask => {
                            api.prevent_close();
                            let _ = window.emit("app-close-requested", ());
                        }
                    }
                }
            }

            if let tauri::WindowEvent::Destroyed = event {
                let label = window.label().to_string();
                let app_handle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if label == "main" {
                        // Main window closed — clear all sessions
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            state.clear_all_sessions().await;
                            tracing::info!("all sessions cleared on window close");
                        }
                        // Also close debug console if open
                        if let Some(debug_win) = app_handle.get_webview_window("debug-console") {
                            let _ = debug_win.destroy();
                        }
                    } else if label == "debug-console" {
                        // Debug console closed — sync config toggle to false
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            let mut config = state.config.write().await;
                            if config.debug_logging {
                                config.debug_logging = false;
                                let _ =
                                    config_service::save_config(&state.config_path, &config).await;
                                tracing::info!("debug_logging disabled (console closed)");
                            }
                            // Emit event so frontend can sync the toggle
                            let _ = app_handle.emit("debug-window-closed", ());
                        }
                    } else if label == "gamepass-login" {
                        // GamePass popup closed — notify frontend
                        let _ = app_handle.emit("gamepass-login-cancelled", ());
                    } else if label == "recaptcha_window" {
                        // Only a real user/abort close signals cancellation — not
                        // when we closed the window ourselves after capturing a
                        // token (that would abort the next login phase).
                        if !services::recaptcha_window::recaptcha_take_delivered() {
                            let _ = app_handle.emit("recaptcha-cancelled", ());
                        }
                    } else if label == "web-login" {
                        // Regular web-login window closed before completion
                        let _ = app_handle.emit("regular-login-cancelled", ());
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run MapleLink");
}

/// Strip the Windows 11 DWM frame border (accent-coloured top hairline) and
/// round the corners on the borderless transparent window. Must be re-applied
/// whenever Windows might restore the frame (focus gain, resize, move).
#[cfg(target_os = "windows")]
fn apply_borderless_dwm(window: &tauri::Window) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
        const DWMWA_BORDER_COLOR: u32 = 34;
        const DWMWCP_ROUND: u32 = 2;
        const DWM_COLOR_NONE: u32 = 0xFFFFFFFE;
        // Round the corners so the DWM shadow follows the CSS border-radius.
        let round = DWMWCP_ROUND;
        let _ = windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd.0,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &round as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        // No border colour → no accent/black hairline around the window.
        let color: u32 = DWM_COLOR_NONE;
        let _ = windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd.0,
            DWMWA_BORDER_COLOR,
            &color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Set once a real quit is in progress, so the close interceptor stops
/// intercepting and lets the windows close.
static QUIT_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Quit gracefully: mark quitting and close every window so Tauri exits through
/// its normal teardown. This avoids `app.exit(0)`'s abrupt process exit, which on
/// Windows logs a benign "Failed to unregister class Chrome_WidgetWin_0" error
/// from WebView2's Chromium during shutdown.
pub fn request_quit(app: &tauri::AppHandle) {
    QUIT_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
    for (_, win) in app.webview_windows() {
        let _ = win.close();
    }
}

/// Build the system-tray icon + menu (Show / Quit). Enables "minimize to tray".
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    // Best-effort localisation of the two labels to the configured language.
    let lang = app
        .try_state::<AppState>()
        .and_then(|s| s.config.try_read().ok().map(|c| c.language.clone()))
        .unwrap_or(models::config::Language::ZhTW);
    let (show_label, quit_label) = match lang {
        models::config::Language::EnUS => ("Show MapleLink", "Quit"),
        models::config::Language::ZhCN => ("显示主窗口", "退出"),
        models::config::Language::ZhTW => ("顯示主視窗", "結束"),
    };

    let show_i = MenuItem::with_id(app, "tray_show", show_label, true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "tray_quit", quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("MapleLink")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray_show" => show_main_window(app),
            "tray_quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Show, unminimize and focus the main window (from the tray).
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}
