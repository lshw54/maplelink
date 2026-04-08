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
        launch_with_lr(
            &app,
            &launch_cmd,
            config.traditional_login,
            &config.region,
            &credentials,
        )
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

    // 8. Record PID in active processes (skip PID 0 from PowerShell launch)
    if pid > 0 {
        state
            .active_processes
            .write()
            .await
            .insert(pid, account_id.clone());
    }

    // 9. Auto-kill Patcher.exe (matching C# checkPatcher_Tick)
    // The game may spawn Patcher.exe for auto-update before MapleStory.exe.
    // Kill it so the game launches directly with our OTP credentials.
    let game_dir = launch_cmd.working_dir.clone();
    tauri::async_runtime::spawn(async move {
        kill_patcher_loop(&game_dir).await;
    });

    tracing::info!(pid, account_id = %account_id, use_lr, "game launched");
    Ok(pid)
}

/// Poll for Patcher.exe in the game directory and kill it.
///
/// Matches C# `checkPatcher_Tick`: every 100ms for up to 30 seconds,
/// check if Patcher.exe from the game directory is running and kill it.
/// This prevents the game's auto-updater from blocking the direct launch.
async fn kill_patcher_loop(game_dir: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let patcher_path = std::path::Path::new(game_dir)
            .join("Patcher.exe")
            .to_string_lossy()
            .to_lowercase();

        // Poll for up to 30 seconds (300 × 100ms)
        for _ in 0..300 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let output = match Command::new("wmic")
                .args([
                    "process",
                    "where",
                    "name='Patcher.exe'",
                    "get",
                    "ProcessId,ExecutablePath",
                    "/format:csv",
                ])
                .output()
            {
                Ok(o) => o,
                Err(_) => continue,
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let lower = line.to_lowercase();
                if lower.contains(&patcher_path) {
                    // Extract PID from CSV: Node,ExecutablePath,ProcessId
                    if let Some(pid_str) = line.split(',').next_back() {
                        if let Ok(pid) = pid_str.trim().parse::<u32>() {
                            let _ = Command::new("taskkill")
                                .args(["/PID", &pid.to_string(), "/F"])
                                .output();
                            tracing::info!("killed Patcher.exe (PID {pid})");
                            return; // Done — patcher killed
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = game_dir;
    }
}

/// Launch the game via Locale Remulator.
///
/// Extracts LR files to app data, then spawns `LRProc.exe` with the
/// profile GUID and game path + args.
/// Matches C# reference: UseShellExecute=true, Verb="runas" (admin),
/// WorkingDirectory = game directory.
/// Launch the game via Locale Remulator.
///
/// Extracts LR files to app data, then spawns `LRProc.exe` with the
/// profile GUID and game path + args.
/// Since maplelink.exe runs as admin (via manifest), LRProc inherits
/// elevation — no need for ShellExecute/runas.
/// Launch the game via Locale Remulator.
///
/// Launch the game via Locale Remulator.
///
/// Extracts LR files to app data, then spawns `LRProc.exe`.
/// C# reference: `LRProc.exe GUID "gamepath" server port BeanFun account otp`
/// WorkingDirectory = game directory.
/// Launch the game via Locale Remulator.
///
/// Extracts LR files to app data, then spawns `LRProc.exe`.
/// C# reference: `LRProc.exe GUID "gamepath" server port BeanFun account otp`
/// WorkingDirectory = game directory.
/// Launch the game via Locale Remulator.
///
/// Extracts LR files to app data, then spawns `LRProc.exe`.
/// Uses `powershell Start-Process` to match the confirmed working behavior.
/// Launch the game via Locale Remulator.
///
/// Extracts LR files to app data, then spawns `LRProc.exe` via PowerShell
/// Start-Process (confirmed working).
/// Launch the game via Locale Remulator.
///
/// When `traditional_login` is true, passes full server/port/account/otp args.
/// When false, only passes GUID + game path (simpler, more compatible).
/// Launch the game via Locale Remulator.
///
/// `traditional_login=true`: only GUID + game path (user logs in manually in-game).
/// `traditional_login=false`: GUID + game path + server/port/BeanFun/account/otp
///   (game auto-connects with credentials).
async fn launch_with_lr(
    app: &tauri::AppHandle,
    launch_cmd: &game_launcher::LaunchCommand,
    traditional_login: bool,
    region: &crate::models::session::Region,
    credentials: &crate::models::game_account::GameCredentials,
) -> Result<u32, ProcessError> {
    let lr_proc = lr_service::ensure_lr_files(app).await?;
    let lr_path = lr_proc.to_string_lossy().to_string();

    // Only TW supports direct login via command line args.
    // HK uses login_action_type=8 which doesn't support command line login —
    // HK always launches the game then auto-pastes credentials.
    let use_cmd_args = !traditional_login && matches!(region, crate::models::session::Region::TW);

    let lr_args = if use_cmd_args {
        // TW non-traditional: GUID "gamepath" server port BeanFun account otp
        format!(
            "{} \"{}\" tw.login.maplestory.beanfun.com 8484 BeanFun {} {}",
            lr_service::LR_PROFILE_GUID,
            launch_cmd.executable,
            credentials.account_id,
            credentials.otp,
        )
    } else {
        // Traditional / HK: just launch the game
        format!(
            "{} \"{}\"",
            lr_service::LR_PROFILE_GUID,
            launch_cmd.executable
        )
    };

    tracing::info!(
        lr_proc = %lr_path,
        lr_args = %lr_args,
        traditional_login,
        "launching game via Locale Remulator"
    );

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};

        let ps_cmd = format!(
            "-NoProfile -Command Start-Process -FilePath '{}' -ArgumentList '{}' -WorkingDirectory '{}'",
            lr_path, lr_args, launch_cmd.working_dir
        );

        Command::new("powershell")
            .raw_arg(&ps_cmd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e: std::io::Error| ProcessError::SpawnFailed {
                path: lr_path.clone(),
                reason: e.to_string(),
            })?;

        Ok(0)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(ProcessError::SpawnFailed {
            path: lr_path,
            reason: "LR launch is only supported on Windows".to_string(),
        })
    }
}

/// Check if any MapleStory process is currently running.
///
/// Checks both tracked PIDs and the MapleStory window class name.
#[tauri::command]
pub async fn is_game_running(state: State<'_, AppState>) -> Result<bool, ErrorDto> {
    // Check tracked PIDs first
    let active = state.active_processes.read().await;
    for &pid in active.keys() {
        if pid > 0 && process_service::is_process_running(pid) {
            return Ok(true);
        }
    }
    drop(active);

    // Check if MapleStory.exe process exists (covers LR-launched games where PID=0)
    Ok(process_service::is_process_name_running("MapleStory.exe"))
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
