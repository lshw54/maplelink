//! Tauri commands for game launching and process monitoring.
//!
//! Thin wrappers: validate inputs, delegate to core/service, map errors to [`ErrorDto`].

use tauri::State;

use crate::core::auth;
use crate::core::error::{AppError, AuthError, FsError, ProcessError};
use crate::core::game_launcher;
use crate::models::app_state::AppState;
use crate::models::error::ErrorDto;
use crate::services::{beanfun_service, lr_service, process_service};

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Launch the game for a given account (Req 3.6, 4.1, 4.3, 4.4).
///
/// Flow:
/// 1. Validate account_id input
/// 2. Require valid session
/// 3. Read config, validate game path exists on disk
/// 4. Retrieve game credentials from Beanfun
/// 5. Build launch command (executable, working_dir, args)
/// 6. Auto-detect system locale; if not zh-TW/zh-HK, launch via Locale Remulator
/// 7. Otherwise: launch game directly
/// 8. Record PID
#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    account_id: String,
    state: State<'_, AppState>,
) -> Result<u32, ErrorDto> {
    // 1. Validate input
    auth::validate_input("account_id", &account_id).map_err(auth_err_to_dto)?;

    // 2. Require valid session
    let session_guard = state.session.read().await;
    let session = auth::require_valid_session(&session_guard).map_err(auth_err_to_dto)?;

    // 3. Read config and validate game path exists on disk
    let config = state.config.read().await.clone();

    // Syntactic validation
    game_launcher::validate_game_path(&config.game_path).map_err(fs_err_to_dto)?;

    // Actual file existence check on disk
    tokio::fs::metadata(&config.game_path).await.map_err(|e| {
        let fs_err = match e.kind() {
            std::io::ErrorKind::NotFound => FsError::NotFound {
                path: config.game_path.clone(),
            },
            std::io::ErrorKind::PermissionDenied => FsError::PermissionDenied {
                path: config.game_path.clone(),
            },
            _ => FsError::Io {
                path: config.game_path.clone(),
                reason: e.to_string(),
            },
        };
        fs_err_to_dto(fs_err)
    })?;

    // 4. Get game credentials
    let credentials = beanfun_service::get_game_credentials(
        &state.http_client,
        session,
        &account_id,
        &state.cookie_jar,
    )
    .await
    .map_err(login_err_to_dto)?;

    // Drop session read lock before any further state writes
    drop(session_guard);

    // 5. Build launch command
    let launch_cmd =
        game_launcher::build_launch_command(&config, &credentials).map_err(fs_err_to_dto)?;

    // 6–7. Launch with LR or directly
    // Auto mode (matching C# reference): detect system locale, use LR if not zh-TW/zh-HK
    let system_is_zhtw = lr_service::is_system_locale_chinese_traditional();
    tracing::info!("system locale is Traditional Chinese: {system_is_zhtw}");

    let use_lr = !system_is_zhtw;

    let pid = if use_lr {
        launch_with_lr(&app, &launch_cmd)
            .await
            .map_err(proc_err_to_dto)?
    } else {
        tracing::info!("system locale is Traditional Chinese, launching directly");
        process_service::spawn_process(
            &launch_cmd.executable,
            &launch_cmd.working_dir,
            &launch_cmd.args,
        )
        .await
        .map_err(proc_err_to_dto)?
    };

    // 8. Record PID in active processes
    state
        .active_processes
        .write()
        .await
        .insert(pid, account_id.clone());

    tracing::info!(pid, account_id = %account_id, use_lr, "game launched");
    Ok(pid)
}

/// Launch the game via Locale Remulator.
///
/// Extracts LR files to app data, then spawns `LRProc.exe` with the
/// profile GUID and game path + args.
async fn launch_with_lr(
    app: &tauri::AppHandle,
    launch_cmd: &game_launcher::LaunchCommand,
) -> Result<u32, ProcessError> {
    let lr_proc = lr_service::ensure_lr_files(app).await?;

    tracing::info!(
        lr_proc = %lr_proc.display(),
        game = %launch_cmd.executable,
        "launching game via Locale Remulator"
    );

    // LRProc.exe expects: <GUID> "<game_path>" [game_args...]
    let mut lr_args = vec![
        lr_service::LR_PROFILE_GUID.to_string(),
        launch_cmd.executable.clone(),
    ];
    lr_args.extend(launch_cmd.args.iter().cloned());

    let lr_proc_str = lr_proc.to_str().unwrap_or_default();

    process_service::spawn_process(lr_proc_str, &launch_cmd.working_dir, &lr_args).await
}

/// Check if any MapleStory process is currently running.
///
/// Checks both tracked PIDs and the MapleStory window class name.
#[tauri::command]
pub async fn is_game_running(state: State<'_, AppState>) -> Result<bool, ErrorDto> {
    // Check tracked processes first
    let active = state.active_processes.read().await;
    for &pid in active.keys() {
        if process_service::is_process_running(pid) {
            return Ok(true);
        }
    }
    drop(active);

    // Also check by window class name (in case game was launched externally)
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        let class_names = ["MapleStoryClass", "MapleStoryClassTW"];
        for name in class_names {
            let wide: Vec<u16> = OsStr::new(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let hwnd = unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
                    wide.as_ptr(),
                    std::ptr::null(),
                )
            };
            if !hwnd.is_null() {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Check whether a tracked game process is still running (Req 4.5).
///
/// Returns `true` if the process is alive, `false` otherwise.
/// Automatically removes dead processes from the active list.
#[tauri::command]
pub async fn get_process_status(pid: u32, state: State<'_, AppState>) -> Result<bool, ErrorDto> {
    let tracked = state.active_processes.read().await.contains_key(&pid);
    if !tracked {
        return Ok(false);
    }

    let running = process_service::is_process_running(pid);

    if !running {
        state.active_processes.write().await.remove(&pid);
        tracing::info!(pid, "game process exited, removed from active list");
    }

    Ok(running)
}

// ---------------------------------------------------------------------------
// Error mapping helpers
// ---------------------------------------------------------------------------

/// Convert an [`AuthError`] into an [`ErrorDto`].
fn auth_err_to_dto(err: AuthError) -> ErrorDto {
    let app_err: AppError = err.into();
    ErrorDto::from(app_err)
}

/// Convert an [`FsError`] into an [`ErrorDto`].
fn fs_err_to_dto(err: FsError) -> ErrorDto {
    let app_err: AppError = err.into();
    ErrorDto::from(app_err)
}

/// Convert a [`ProcessError`] into an [`ErrorDto`].
fn proc_err_to_dto(err: ProcessError) -> ErrorDto {
    let app_err: AppError = err.into();
    ErrorDto::from(app_err)
}

/// Convert a [`beanfun_service::LoginError`] into an [`ErrorDto`].
fn login_err_to_dto(err: beanfun_service::LoginError) -> ErrorDto {
    match err {
        beanfun_service::LoginError::Auth(e) => auth_err_to_dto(e),
        beanfun_service::LoginError::Network(e) => {
            let app_err: AppError = e.into();
            ErrorDto::from(app_err)
        }
    }
}
