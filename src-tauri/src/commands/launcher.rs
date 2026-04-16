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
    otp: Option<String>,
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
        let _bf_lock = state.bf_client_lock.lock().await;
        let creds = beanfun_service::get_game_credentials(
            &state.http_client,
            session,
            &account_id,
            &state.cookie_jar,
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

    // 8. Record initial PID in active processes.
    // For LR launches, `pid` is LRProc.exe which exits quickly.
    let account_id_for_track = account_id.clone();
    if pid > 0 {
        state
            .active_processes
            .write()
            .await
            .insert(pid, account_id.clone());
    }

    // Background monitor: continuously track the real MapleStory.exe PID.
    // Anti-cheat (e.g. NGS/HackShield) may restart the game process multiple
    // times during startup, so we keep polling until the game truly exits.
    {
        let app_handle = app.clone();
        let lr_pid = if use_lr { pid } else { 0 };
        let acct = account_id_for_track.clone();
        tauri::async_runtime::spawn(async move {
            use tauri::Manager;
            let state = app_handle.state::<AppState>();

            // If launched via LR, wait for LRProc.exe to exit first (up to 10s)
            if lr_pid > 0 {
                for _ in 0..100 {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if !process_service::is_process_running(lr_pid) {
                        break;
                    }
                }
                state.active_processes.write().await.remove(&lr_pid);
            }

            // Continuously track MapleStory.exe PID.
            // Poll every 2s for up to 10 minutes. Anti-cheat may restart the
            // process several times, so we keep updating the tracked PID.
            let mut current_pid: Option<u32> = None;
            let mut consecutive_missing = 0u32;

            for _ in 0..300 {
                tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

                let found_pid = find_process_pid_by_name("MapleStory.exe");

                match (found_pid, current_pid) {
                    (Some(new_pid), Some(old_pid)) if new_pid != old_pid => {
                        // PID changed (anti-cheat restarted the process)
                        state.active_processes.write().await.remove(&old_pid);
                        state
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
                        state
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
                            state.active_processes.write().await.remove(&old_pid);
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
        tauri::async_runtime::spawn(async move {
            kill_patcher_loop(&game_dir).await;
        });
    }

    // 10. Auto-close the game's "Play" startup window (StartUpDlgClass).
    // MapleStory shows a launcher window with a Play button before the actual
    // game starts. When skipPlayConfirm is enabled, we auto-close it.
    if config.skip_play_confirm {
        tauri::async_runtime::spawn(async move {
            skip_play_window_loop().await;
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

    // Build a minimal launch command with no OTP
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
        launch_with_lr(
            &app,
            &launch_cmd,
            true, // force traditional (no credential args)
            &config.region,
            &dummy_creds,
        )
        .await
        .map_err(proc_err_to_dto)?
    } else {
        process_service::spawn_process(&launch_cmd.executable, &launch_cmd.working_dir, &[])
            .await
            .map_err(proc_err_to_dto)?
    };

    // Register initial PID
    if pid > 0 {
        state
            .active_processes
            .write()
            .await
            .insert(pid, "__direct__".to_string());
    }

    // Background monitor: track the real MapleStory.exe PID
    {
        let app_handle = app.clone();
        let lr_pid = if use_lr { pid } else { 0 };
        tauri::async_runtime::spawn(async move {
            use tauri::Manager;
            let state = app_handle.state::<AppState>();

            // Wait for LRProc.exe to exit
            if lr_pid > 0 {
                for _ in 0..100 {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if !process_service::is_process_running(lr_pid) {
                        break;
                    }
                }
                state.active_processes.write().await.remove(&lr_pid);
            }

            // Track MapleStory.exe PID
            let mut current_pid: Option<u32> = None;
            let mut consecutive_missing = 0u32;

            for _ in 0..300 {
                tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

                let found_pid = find_process_pid_by_name("MapleStory.exe");

                match (found_pid, current_pid) {
                    (Some(new_pid), Some(old_pid)) if new_pid != old_pid => {
                        state.active_processes.write().await.remove(&old_pid);
                        state
                            .active_processes
                            .write()
                            .await
                            .insert(new_pid, "__direct__".to_string());
                        tracing::info!(old_pid, new_pid, "MapleStory PID changed");
                        current_pid = Some(new_pid);
                        consecutive_missing = 0;
                    }
                    (Some(new_pid), None) => {
                        state
                            .active_processes
                            .write()
                            .await
                            .insert(new_pid, "__direct__".to_string());
                        tracing::info!(pid = new_pid, "MapleStory.exe found (direct launch)");
                        current_pid = Some(new_pid);
                        consecutive_missing = 0;
                    }
                    (Some(_), Some(_)) => {
                        consecutive_missing = 0;
                    }
                    (None, Some(old_pid)) => {
                        consecutive_missing += 1;
                        if consecutive_missing >= 5 {
                            state.active_processes.write().await.remove(&old_pid);
                            tracing::info!(pid = old_pid, "MapleStory.exe exited (direct launch)");
                            return;
                        }
                    }
                    (None, None) => {
                        consecutive_missing += 1;
                        if consecutive_missing >= 30 {
                            tracing::warn!("MapleStory.exe never appeared (direct launch)");
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
        tauri::async_runtime::spawn(async move {
            kill_patcher_loop(&game_dir).await;
        });
    }

    if config.skip_play_confirm {
        tauri::async_runtime::spawn(async move {
            skip_play_window_loop().await;
        });
    }

    tracing::info!(pid, use_lr, "game launched directly (no login)");
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

/// Auto-close MapleStory's "Play" startup window (StartUpDlgClass).
///
/// The game shows a launcher window with a Play button before the actual
/// game client starts. This polls every 100ms for up to 60 seconds and
/// sends WM_CLOSE to dismiss it automatically.
/// Matches the original C# Beanfun `checkPlayPage_Tick` / `skipPlayWnd`.
async fn skip_play_window_loop() {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

        const WM_CLOSE: u32 = 0x0010;

        fn to_wide(s: &str) -> Vec<u16> {
            OsStr::new(s)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        }

        let class_name = to_wide("StartUpDlgClass");
        let window_title = to_wide("MapleStory");

        // Poll for up to 60 seconds (600 × 100ms)
        for _ in 0..600 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let hwnd = unsafe { FindWindowW(class_name.as_ptr(), window_title.as_ptr()) };
            if !hwnd.is_null() {
                unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
                tracing::info!("auto-closed MapleStory StartUpDlg (Play window)");
                return;
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // no-op
    }
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

        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
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

        // Try with CREATE_BREAKAWAY_FROM_JOB first. If the current job object
        // doesn't allow breakaway, this fails with ERROR_ACCESS_DENIED (5).
        // Fall back to CREATE_NEW_PROCESS_GROUP only.
        let mut success = unsafe {
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
            let err_code = unsafe { GetLastError() };
            if err_code == 5 {
                // ACCESS_DENIED — retry without CREATE_BREAKAWAY_FROM_JOB
                tracing::warn!("CreateProcessW with BREAKAWAY_FROM_JOB denied, retrying without");
                cmd_line = to_wide_null(&format!("\"{}\" {}", lr_path, lr_args));
                si = unsafe { mem::zeroed() };
                si.cb = mem::size_of::<STARTUPINFOW>() as u32;
                pi = unsafe { mem::zeroed() };

                success = unsafe {
                    CreateProcessW(
                        application.as_ptr(),
                        cmd_line.as_mut_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        0,
                        0x00000200, // CREATE_NEW_PROCESS_GROUP only
                        std::ptr::null(),
                        working_dir.as_ptr(),
                        &si,
                        &mut pi,
                    )
                };
            }
        }

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

/// Return the PID of the currently tracked MapleStory process, or 0 if none.
#[tauri::command]
pub async fn get_game_pid(state: State<'_, AppState>) -> Result<u32, ErrorDto> {
    let active = state.active_processes.read().await;
    for &pid in active.keys() {
        if pid > 0 && process_service::is_process_running(pid) {
            return Ok(pid);
        }
    }
    Ok(0)
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
/// Kill all running MapleStory processes and clear tracked PIDs.
///
/// Used for the "relaunch" flow — kill the old game before starting a new one.
#[tauri::command]
pub async fn kill_game(state: State<'_, AppState>) -> Result<(), ErrorDto> {
    // Kill all tracked PIDs
    let pids: Vec<u32> = state
        .active_processes
        .read()
        .await
        .keys()
        .copied()
        .collect();
    for pid in &pids {
        if *pid > 0 && process_service::is_process_running(*pid) {
            let _ = process_service::terminate_process(*pid).await;
            tracing::info!(pid, "killed tracked game process");
        }
    }

    // Also kill any MapleStory.exe not in our tracked list
    #[cfg(target_os = "windows")]
    {
        while let Some(pid) = find_process_pid_by_name("MapleStory.exe") {
            let _ = process_service::terminate_process(pid).await;
            tracing::info!(pid, "killed MapleStory.exe");
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    // Clear all tracked processes
    state.active_processes.write().await.clear();
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
