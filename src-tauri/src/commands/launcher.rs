//! Tauri commands for game launching and process monitoring.
//!
//! Thin wrappers: validate inputs, delegate to core/service, map errors to [`ErrorDto`].

use tauri::State;

use crate::core::auth;
use crate::core::error::{AppError, AuthError, FsError, ProcessError};
use crate::core::game_launcher;
use crate::models::app_state::AppState;
use crate::models::error::ErrorDto;
use crate::models::game_account::GameCredentials;
use crate::services::{beanfun_service, game_launch_service, lr_service, process_service};

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
    session_id: String,
    account_id: String,
    otp: Option<String>,
    state: State<'_, AppState>,
) -> Result<u32, ErrorDto> {
    // 1. Validate input
    auth::validate_input("account_id", &account_id).map_err(auth_err_to_dto)?;

    // 2. Require valid session
    let ss = state.require_session(&session_id).await?;
    let session_guard = ss.session.read().await;
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

    // 4. Get game credentials — skip HTTP if OTP was provided by the frontend.
    let credentials = if let Some(otp_value) = otp {
        tracing::info!("using pre-fetched OTP, skipping credential HTTP request");
        drop(session_guard);
        GameCredentials {
            account_id: account_id.clone(),
            otp: otp_value,
            retrieved_at: chrono::Utc::now(),
            command_line_template: None,
        }
    } else {
        let _bf_lock = ss.bf_client_lock.lock().await;
        let creds = beanfun_service::get_game_credentials(
            &ss.http_client,
            session,
            &account_id,
            &ss.cookie_jar,
        )
        .await
        .map_err(login_err_to_dto)?;
        drop(session_guard);
        drop(_bf_lock);
        creds
    };

    // 5. Build launch command
    let launch_cmd =
        game_launcher::build_launch_command(&config, &credentials).map_err(fs_err_to_dto)?;

    // 6–7. Launch with LR or directly
    // Auto mode: detect system locale, use LR if not zh-TW/zh-HK
    let system_is_zhtw = lr_service::is_system_locale_chinese_traditional();
    tracing::info!("system locale is Traditional Chinese: {system_is_zhtw}");

    let use_lr = !system_is_zhtw;

    let pid = if use_lr {
        game_launch_service::launch_with_lr(
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
        // When not in traditional mode and credentials are available,
        // pass the full command line (server/port/account/otp) just like LR mode.
        let args = if !config.traditional_login
            && !credentials.account_id.is_empty()
            && !credentials.otp.is_empty()
        {
            let template = credentials
                .command_line_template
                .as_deref()
                .unwrap_or("tw.login.maplestory.beanfun.com 8484 BeanFun %s %s");
            let cmd_line = template
                .replacen("%s", &credentials.account_id, 1)
                .replacen("%s", &credentials.otp, 1);
            cmd_line
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        process_service::spawn_process(&launch_cmd.executable, &launch_cmd.working_dir, &args)
            .await
            .map_err(proc_err_to_dto)?
    };

    // 8. Record initial PID in active processes.
    // For LR launches, `pid` is LRProc.exe which exits quickly.
    let account_id_for_track = account_id.clone();
    if pid > 0 {
        ss.active_processes
            .write()
            .await
            .insert(pid, account_id.clone());
    }

    // Background monitor: continuously track the real MapleStory.exe PID.
    // Anti-cheat (e.g. NGS/HackShield) may restart the game process multiple
    // times during startup, so we keep polling until the game truly exits.
    {
        let ss_clone = ss.clone();
        let lr_pid = if use_lr { pid } else { 0 };
        let acct = account_id_for_track.clone();
        tauri::async_runtime::spawn(async move {
            // If launched via LR, wait for LRProc.exe to exit first (up to 10s)
            if lr_pid > 0 {
                for _ in 0..100 {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if !process_service::is_process_running(lr_pid) {
                        break;
                    }
                }
                ss_clone.active_processes.write().await.remove(&lr_pid);
            }

            // Continuously track MapleStory.exe PID.
            // Poll every 2s for up to 10 minutes. Anti-cheat may restart the
            // process several times, so we keep updating the tracked PID.
            let mut current_pid: Option<u32> = None;
            let mut consecutive_missing = 0u32;

            for _ in 0..300 {
                tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

                let found_pid = game_launch_service::find_process_pid_by_name("MapleStory.exe");

                match (found_pid, current_pid) {
                    (Some(new_pid), Some(old_pid)) if new_pid != old_pid => {
                        // PID changed (anti-cheat restarted the process)
                        ss_clone.active_processes.write().await.remove(&old_pid);
                        ss_clone
                            .active_processes
                            .write()
                            .await
                            .insert(new_pid, acct.clone());
                        tracing::info!(
                            old_pid,
                            new_pid,
                            "MapleStory PID changed (anti-cheat restart)"
                        );
                        current_pid = Some(new_pid);
                        consecutive_missing = 0;
                    }
                    (Some(new_pid), None) => {
                        // First time finding MapleStory
                        ss_clone
                            .active_processes
                            .write()
                            .await
                            .insert(new_pid, acct.clone());
                        tracing::info!(pid = new_pid, "MapleStory.exe found");
                        current_pid = Some(new_pid);
                        consecutive_missing = 0;
                    }
                    (Some(_), Some(_)) => {
                        // Same PID, still running
                        consecutive_missing = 0;
                    }
                    (None, Some(old_pid)) => {
                        // Process disappeared — might be anti-cheat restarting it.
                        // Wait a few cycles before declaring it dead.
                        consecutive_missing += 1;
                        if consecutive_missing >= 5 {
                            // 10 seconds of no MapleStory — it's really gone
                            ss_clone.active_processes.write().await.remove(&old_pid);
                            tracing::info!(pid = old_pid, "MapleStory.exe exited");
                            return;
                        }
                    }
                    (None, None) => {
                        // Not found yet — keep waiting (anti-cheat startup)
                        consecutive_missing += 1;
                        if consecutive_missing >= 30 {
                            // 60 seconds and never found — give up
                            tracing::warn!("MapleStory.exe never appeared after launch");
                            return;
                        }
                    }
                }
            }
            // 10 minutes — stop monitoring
            tracing::info!("game monitor timed out after 10 minutes");
        });
    }

    // 9. Auto-kill Patcher.exe (respects config toggle)
    if config.auto_kill_patcher {
        let game_dir = launch_cmd.working_dir.clone();
        let app_for_patcher = app.clone();
        tauri::async_runtime::spawn(async move {
            game_launch_service::kill_patcher_loop(&game_dir, &app_for_patcher).await;
        });
    }

    // 10. Auto-close the game's "Play" startup window (StartUpDlgClass).
    // MapleStory shows a launcher window with a Play button before the actual
    // game starts. When skipPlayConfirm is enabled, we auto-close it.
    if config.skip_play_confirm {
        tauri::async_runtime::spawn(async move {
            game_launch_service::skip_play_window_loop().await;
        });
    }

    tracing::info!(pid, account_id = %account_id, use_lr, "game launched");
    Ok(pid)
}

/// Launch the game directly without requiring a login session.
///
/// Validates the game path, then launches with LR or directly based on
/// system locale. No OTP or credentials are passed — the game starts
/// at its own login screen.
#[tauri::command]
pub async fn launch_game_direct(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, ErrorDto> {
    let config = state.config.read().await.clone();

    game_launcher::validate_game_path(&config.game_path).map_err(fs_err_to_dto)?;

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

    let dummy_creds = GameCredentials {
        account_id: String::new(),
        otp: String::new(),
        retrieved_at: chrono::Utc::now(),
        command_line_template: None,
    };
    let launch_cmd =
        game_launcher::build_launch_command(&config, &dummy_creds).map_err(fs_err_to_dto)?;

    let system_is_zhtw = lr_service::is_system_locale_chinese_traditional();
    let use_lr = !system_is_zhtw;

    let pid = if use_lr {
        game_launch_service::launch_with_lr(&app, &launch_cmd, true, &config.region, &dummy_creds)
            .await
            .map_err(proc_err_to_dto)?
    } else {
        process_service::spawn_process(&launch_cmd.executable, &launch_cmd.working_dir, &[])
            .await
            .map_err(proc_err_to_dto)?
    };

    // Create a temporary session for PID tracking
    let (_, ss) = state.create_session().await;
    if pid > 0 {
        ss.active_processes
            .write()
            .await
            .insert(pid, "__direct__".to_string());
    }

    // Background monitor: track the real MapleStory.exe PID
    {
        let ss_clone = ss.clone();
        let lr_pid = if use_lr { pid } else { 0 };
        tauri::async_runtime::spawn(async move {
            if lr_pid > 0 {
                for _ in 0..100 {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if !process_service::is_process_running(lr_pid) {
                        break;
                    }
                }
                ss_clone.active_processes.write().await.remove(&lr_pid);
            }

            let mut current_pid: Option<u32> = None;
            let mut consecutive_missing = 0u32;

            for _ in 0..300 {
                tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                let found_pid = game_launch_service::find_process_pid_by_name("MapleStory.exe");

                match (found_pid, current_pid) {
                    (Some(new_pid), Some(old_pid)) if new_pid != old_pid => {
                        ss_clone.active_processes.write().await.remove(&old_pid);
                        ss_clone
                            .active_processes
                            .write()
                            .await
                            .insert(new_pid, "__direct__".to_string());
                        tracing::info!(old_pid, new_pid, "MapleStory PID changed (direct)");
                        current_pid = Some(new_pid);
                        consecutive_missing = 0;
                    }
                    (Some(new_pid), None) => {
                        ss_clone
                            .active_processes
                            .write()
                            .await
                            .insert(new_pid, "__direct__".to_string());
                        tracing::info!(pid = new_pid, "MapleStory.exe found (direct)");
                        current_pid = Some(new_pid);
                        consecutive_missing = 0;
                    }
                    (Some(_), Some(_)) => {
                        consecutive_missing = 0;
                    }
                    (None, Some(old_pid)) => {
                        consecutive_missing += 1;
                        if consecutive_missing >= 5 {
                            ss_clone.active_processes.write().await.remove(&old_pid);
                            tracing::info!(pid = old_pid, "MapleStory.exe exited (direct)");
                            return;
                        }
                    }
                    (None, None) => {
                        consecutive_missing += 1;
                        if consecutive_missing >= 30 {
                            return;
                        }
                    }
                }
            }
        });
    }

    // Auto-kill patcher
    if config.auto_kill_patcher {
        let game_dir = launch_cmd.working_dir.clone();
        let app_for_patcher = app.clone();
        tauri::async_runtime::spawn(async move {
            game_launch_service::kill_patcher_loop(&game_dir, &app_for_patcher).await;
        });
    }

    if config.skip_play_confirm {
        tauri::async_runtime::spawn(async move {
            game_launch_service::skip_play_window_loop().await;
        });
    }

    tracing::info!(pid, use_lr, "game launched directly (no login)");
    Ok(pid)
}

/// Check if any MapleStory process is currently running across ALL sessions.
///
/// Checks both tracked PIDs and the MapleStory window class name.
#[tauri::command]
pub async fn is_game_running(state: State<'_, AppState>) -> Result<bool, ErrorDto> {
    Ok(state.is_any_game_running().await)
}

/// Return the PID of the currently tracked MapleStory process across ALL sessions, or 0 if none.
#[tauri::command]
pub async fn get_game_pid(state: State<'_, AppState>) -> Result<u32, ErrorDto> {
    Ok(state.get_any_game_pid().await)
}

/// Check whether a tracked game process is still running (Req 4.5).
///
/// Returns `true` if the process is alive, `false` otherwise.
/// Automatically removes dead processes from the session's active list.
#[tauri::command]
pub async fn get_process_status(
    session_id: String,
    pid: u32,
    state: State<'_, AppState>,
) -> Result<bool, ErrorDto> {
    let ss = state.require_session(&session_id).await?;

    let tracked = ss.active_processes.read().await.contains_key(&pid);
    if !tracked {
        return Ok(false);
    }

    let running = process_service::is_process_running(pid);

    if !running {
        ss.active_processes.write().await.remove(&pid);
        tracing::info!(pid, "game process exited, removed from active list");
    }

    Ok(running)
}

/// Kill all running MapleStory processes across ALL sessions and clear tracked PIDs.
#[tauri::command]
pub async fn kill_game(state: State<'_, AppState>) -> Result<(), ErrorDto> {
    // Kill all tracked PIDs across all sessions
    let sessions = state.sessions.read().await;
    for ss in sessions.values() {
        let pids: Vec<u32> = ss.active_processes.read().await.keys().copied().collect();
        for pid in &pids {
            if *pid > 0 && process_service::is_process_running(*pid) {
                let _ = process_service::terminate_process(*pid).await;
                tracing::info!(pid, "killed tracked game process");
            }
        }
        ss.active_processes.write().await.clear();
    }
    drop(sessions);

    // Also kill any MapleStory.exe not in our tracked list
    #[cfg(target_os = "windows")]
    {
        while let Some(pid) = game_launch_service::find_process_pid_by_name("MapleStory.exe") {
            let _ = process_service::terminate_process(pid).await;
            tracing::info!(pid, "killed MapleStory.exe");
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    tracing::info!("all game processes killed and tracking cleared");
    Ok(())
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
