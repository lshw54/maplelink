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
            commands::auth::login,
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
            commands::auth::delete_saved_account,
            commands::auth::save_login_credentials,
            commands::config::get_config,
            commands::config::set_config,
            commands::config::reset_config,
            commands::account::get_game_accounts,
            commands::account::get_game_credentials,
            commands::account::refresh_accounts,
            commands::account::ping_session,
            commands::account::get_remain_point,
            commands::account::auto_paste_otp,
            commands::account::change_account_display_name,
            commands::account::get_auth_email,
            commands::launcher::launch_game,
            commands::launcher::is_game_running,
            commands::launcher::get_process_status,
            commands::launcher::kill_game,
            commands::system::log_frontend_error,
            commands::system::resize_window,
            commands::system::open_file_dialog,
            commands::system::get_app_version,
            commands::system::detect_game_path,
            commands::system::toggle_debug_window,
            commands::system::open_log_folder,
            commands::system::get_recent_logs,
            commands::system::open_web_popup,
            commands::system::open_gash_popup,
            commands::system::resize_gash_popup,
            commands::system::open_member_popup,
            commands::system::open_customer_service,
            commands::system::get_web_token,
            commands::system::cleanup_game_cache,
            commands::auth::open_gamepass_login,
            commands::auth::gamepass_webview_done,
            commands::update::check_update,
            commands::update::apply_update,
            commands::update::test_github_access,
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

            // Clean up old exe from self-replace update
            if let Ok(exe) = std::env::current_exe() {
                let old = exe.with_extension("exe.old");
                if old.exists() {
                    let _ = std::fs::remove_file(&old);
                    tracing::info!("cleaned up old exe: {}", old.display());
                }
            }
            tracing::info!("log directory: {}", log_dir.display());

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

            // 4. Initialise AppState with loaded config.
            let auto_update_enabled = config.auto_update;
            let update_channel = config.update_channel.clone();
            let cookie_jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
            let http_client = reqwest::Client::builder()
                .cookie_provider(cookie_jar.clone())
                // Accept invalid SSL certificates for compatibility with game
                // accelerators (e.g. UU) that proxy beanfun/gamania traffic.
                .danger_accept_invalid_certs(true)
                .build()
                .expect("failed to build HTTP client");
            let update_client = http_client.clone();

            let state = AppState {
                session: tokio::sync::RwLock::new(None),
                config: tokio::sync::RwLock::new(config),
                game_accounts: tokio::sync::RwLock::new(Vec::new()),
                active_processes: tokio::sync::RwLock::new(HashMap::new()),
                http_client,
                cookie_jar,
                config_path,
                saved_accounts: tokio::sync::RwLock::new(saved_accounts),
                accounts_path,
                bf_client_lock: tokio::sync::Mutex::new(()),
            };

            app.manage(state);

            // 4. Auto-update check (non-blocking background task).
            //    Respects the auto_update toggle — skips if disabled.
            //    Failures are logged and swallowed so the app always starts.
            if update_service::should_check_on_startup(auto_update_enabled) {
                let app_handle_for_update = app.handle().clone();
                tauri::async_runtime::spawn(async move {
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

            // 6. Backend ping loop — runs every 60 seconds independently of frontend.
            //    Frontend setInterval can be throttled by Windows when the app is
            //    minimized or idle. This tokio loop is not affected by that.
            //    Matches the original PingWorker behavior.
            let ping_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait for user to login before starting ping
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    if let Some(state) = ping_app.try_state::<AppState>() {
                        if state.session.read().await.is_some() {
                            break;
                        }
                    }
                }
                tracing::info!("backend ping loop started");

                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    if let Some(state) = ping_app.try_state::<AppState>() {
                        let region = {
                            let session = state.session.read().await;
                            session.as_ref().map(|s| s.region.clone())
                        };
                        if let Some(region) = region {
                            // Non-blocking: skip if another operation holds the lock
                            if let Ok(_guard) = state.bf_client_lock.try_lock() {
                                services::beanfun_service::ping(&state.http_client, &region).await;
                            }
                        } else {
                            // Session cleared (logged out) — stop pinging
                            tracing::info!("backend ping loop stopped (no session)");
                            break;
                        }
                    }
                }
            });

            // Window starts at login page size (340×520) per tauri.conf.json5.
            tracing::info!("startup complete — showing login page");
            Ok(())
        })
        // -- Window lifecycle -----------------------------------------------
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let label = window.label().to_string();
                let app_handle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if label == "main" {
                        // Main window closed — clear credentials
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            state.clear_credentials().await;
                            tracing::info!("credentials cleared on window close");
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
                        // GamePass popup closed — if not authenticated, notify frontend
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            let session = state.session.read().await;
                            if session.is_none() {
                                tracing::info!("GamePass popup closed without completing login");
                                let _ = app_handle.emit("gamepass-login-cancelled", ());
                            }
                        }
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run MapleLink");
}
