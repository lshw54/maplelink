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

    // 4. Get game credentials (acquire bf_client_lock to prevent concurrent beanfun HTTP)
    let _bf_lock = state.bf_client_lock.lock().await;
    let credentials = beanfun_service::get_game_credentials(
        &state.http_client,
        session,
        &account_id,
        &state.cookie_jar,
    )
    .await
    .map_err(login_err_to_dto)?;

    // Drop locks before further state writes
    drop(session_guard);
    drop(_bf_lock);

    // 5. Build launch command
    let launch_cmd =
        game_launcher::build_launch_command(&config, &credentials).map_err(fs_err_to_dto)?;

    // 6–7. Launch with LR or directly
    // Auto mode: detect system locale, use LR if not zh-TW/zh-HK
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

    // 8. Record PID in active processes.
    // For LR launches, `pid` is LRProc.exe which exits quickly after injecting
    // the DLL. A background task will replace it with the real MapleStory PID.
    let account_id_for_track = account_id.clone();
    if pid > 0 {
        state
            .active_processes
            .write()
            .await
            .insert(pid, account_id.clone());
    }

    // Background: if launched via LR, wait for LRProc to exit then find the
    // real MapleStory.exe PID and update active_processes.
    if use_lr && pid > 0 {
        let app_handle = app.clone();
        let lr_pid = pid;
        let acct = account_id_for_track.clone();
        tauri::async_runtime::spawn(async move {
            use tauri::Manager;
            let state = app_handle.state::<AppState>();

            // Wait for LRProc.exe to exit (poll up to 10s)
            for _ in 0..100 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if !process_service::is_process_running(lr_pid) {
                    break;
                }
            }
            // Remove stale LRProc PID
            state.active_processes.write().await.remove(&lr_pid);

            // Poll for MapleStory.exe PID (up to 15s)
            for _ in 0..150 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if let Some(game_pid) = find_process_pid_by_name("MapleStory.exe") {
                    state
                        .active_processes
                        .write()
                        .await
                        .insert(game_pid, acct.clone());
                    tracing::info!(game_pid, lr_pid, "replaced LRProc PID with MapleStory PID");
                    return;
                }
            }
            tracing::warn!("could not find MapleStory.exe PID after LR launch");
        });
    }

    // 9. Auto-kill Patcher.exe
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
/// Every 100ms for up to 30 seconds,
/// check if Patcher.exe from the game directory is running and kill it.
/// This prevents the game's auto-updater from blocking the direct launch.
/// Polls for `Patcher.exe` in the game directory and kills it when found.
///
/// Uses native Windows Toolhelp32 APIs to enumerate processes in-process,
/// avoiding the `wmic.exe` console window popup that the previous
/// implementation caused (wmic spawns a visible console every 100ms).
async fn kill_patcher_loop(game_dir: &str) {
    #[cfg(target_os = "windows")]
    {
        let patcher_path = std::path::Path::new(game_dir)
            .join("Patcher.exe")
            .to_string_lossy()
            .to_lowercase();

        let patcher_path_clone = patcher_path.clone();

        // Poll for up to 30 seconds (300 × 100ms)
        for _ in 0..300 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let path_for_check = patcher_path_clone.clone();
            let found = tokio::task::spawn_blocking(move || find_and_kill_patcher(&path_for_check))
                .await
                .unwrap_or(false);

            if found {
                return;
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = game_dir;
    }
}

/// Find Patcher.exe by enumerating processes via Toolhelp32 snapshot and kill
/// it if its executable path matches the expected game directory.
///
/// Returns `true` if the patcher was found and killed.
#[cfg(target_os = "windows")]
fn find_and_kill_patcher(expected_path_lower: &str) -> bool {
    use std::ffi::OsString;
    use std::mem;
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }

        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry) == 0 {
            CloseHandle(snapshot);
            return false;
        }

        loop {
            // Check if this process is Patcher.exe by its szExeFile field
            let exe_name = OsString::from_wide(
                &entry.szExeFile[..entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len())],
            )
            .to_string_lossy()
            .to_lowercase();

            if exe_name == "patcher.exe" {
                let pid = entry.th32ProcessID;

                // Open the process to query its full path
                let proc_handle = OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                    0, // bInheritHandle = FALSE
                    pid,
                );

                if !proc_handle.is_null() {
                    let mut buf = [0u16; 1024];
                    let mut size = buf.len() as u32;

                    if QueryFullProcessImageNameW(proc_handle, 0, buf.as_mut_ptr(), &mut size) != 0
                    {
                        let full_path = OsString::from_wide(&buf[..size as usize])
                            .to_string_lossy()
                            .to_lowercase();

                        if full_path == *expected_path_lower {
                            TerminateProcess(proc_handle, 1);
                            CloseHandle(proc_handle);
                            CloseHandle(snapshot);
                            tracing::info!("killed Patcher.exe (PID {pid})");
                            return true;
                        }
                    }

                    CloseHandle(proc_handle);
                }
            }

            if Process32NextW(snapshot, &mut entry) == 0 {
                break;
            }
        }

        CloseHandle(snapshot);
    }

    false
}

/// Find a running process by executable name and return its PID.
///
/// Uses Toolhelp32 snapshot to enumerate processes in-process (no console popups).
/// Returns the first matching PID, or `None` if not found.
#[cfg(target_os = "windows")]
fn find_process_pid_by_name(name: &str) -> Option<u32> {
    use std::ffi::OsString;
    use std::mem;
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let name_lower = name.to_lowercase();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry) == 0 {
            CloseHandle(snapshot);
            return None;
        }

        loop {
            let exe_name = OsString::from_wide(
                &entry.szExeFile[..entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len())],
            )
            .to_string_lossy()
            .to_lowercase();

            if exe_name == name_lower {
                let pid = entry.th32ProcessID;
                CloseHandle(snapshot);
                return Some(pid);
            }

            if Process32NextW(snapshot, &mut entry) == 0 {
                break;
            }
        }

        CloseHandle(snapshot);
    }

    None
}

#[cfg(not(target_os = "windows"))]
fn find_process_pid_by_name(_name: &str) -> Option<u32> {
    None
}

/// Launch the game via Locale Remulator.
///
/// Uses ShellExecuteW to avoid console window flash.
/// TW non-traditional mode passes server/port/account/otp args.
async fn launch_with_lr(
    app: &tauri::AppHandle,
    launch_cmd: &game_launcher::LaunchCommand,
    traditional_login: bool,
    region: &crate::models::session::Region,
    credentials: &crate::models::game_account::GameCredentials,
) -> Result<u32, ProcessError> {
    let lr_proc = lr_service::ensure_lr_files(app).await?;
    let lr_path = lr_proc.to_string_lossy().to_string();

    let use_cmd_args = !traditional_login && matches!(region, crate::models::session::Region::TW);

    // Build LR arguments: GUID + game path (+ optional server/port/account/otp)
    let lr_args = if use_cmd_args {
        format!(
            "{} \"{}\" tw.login.maplestory.beanfun.com 8484 BeanFun {} {}",
            lr_service::LR_PROFILE_GUID,
            launch_cmd.executable,
            credentials.account_id,
            credentials.otp,
        )
    } else {
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

    // Use CreateProcessW to get the child PID (ShellExecuteW doesn't return it).
    // Since maplelink.exe already runs elevated, the child inherits the token.
    // CREATE_BREAKAWAY_FROM_JOB (0x01000000) + CREATE_NEW_PROCESS_GROUP (0x00000200)
    // ensure the game survives if MapleLink exits.
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::mem;
        use std::os::windows::ffi::OsStrExt;

        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
        };

        fn to_wide_null(s: &str) -> Vec<u16> {
            OsStr::new(s)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        }

        let application = to_wide_null(&lr_path);
        // CreateProcessW wants a mutable command line buffer
        let mut cmd_line = to_wide_null(&format!("\"{}\" {}", lr_path, lr_args));
        let working_dir = to_wide_null(&launch_cmd.working_dir);

        let mut si: STARTUPINFOW = unsafe { mem::zeroed() };
        si.cb = mem::size_of::<STARTUPINFOW>() as u32;

        let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };

        let success = unsafe {
            CreateProcessW(
                application.as_ptr(),
                cmd_line.as_mut_ptr(),
                std::ptr::null(),     // lpProcessAttributes
                std::ptr::null(),     // lpThreadAttributes
                0,                    // bInheritHandles = FALSE
                0x01000200,           // CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP
                std::ptr::null(),     // lpEnvironment (inherit)
                working_dir.as_ptr(), // lpCurrentDirectory
                &si,
                &mut pi,
            )
        };

        if success == 0 {
            let err = std::io::Error::last_os_error();
            return Err(ProcessError::SpawnFailed {
                path: lr_path,
                reason: format!("CreateProcessW failed: {err}"),
            });
        }

        let pid = pi.dwProcessId;

        // Close handles — we only need the PID, not the handles.
        unsafe {
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }

        tracing::info!(pid, "LRProc.exe started via CreateProcessW");
        Ok(pid)
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
