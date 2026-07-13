//! Side effects for the game-launch flow: launching MapleStory via Locale
//! Remulator, and the post-launch watchers (kill the auto-updater `Patcher.exe`,
//! auto-close the "Play" startup window, detect client/server version).
//!
//! Extracted from `commands/launcher.rs` so the command layer stays thin and all
//! process/window/network side effects live in `services/` (Clean Architecture).

use crate::core::error::ProcessError;
use crate::core::game_launcher;
use crate::models::game_account::GameCredentials;
use crate::models::session::Region;
use crate::services::lr_service;

/// Poll for the game's auto-updater `Patcher.exe` in `game_dir` and kill it as
/// soon as it appears (blocks the game's forced update). When killed, emit a
/// `patcher-killed` event with the detected client + server versions.
pub async fn kill_patcher_loop(game_dir: &str, app: &tauri::AppHandle) {
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
                // Get client version from game exe
                use tauri::Emitter;
                let game_exe = std::path::Path::new(game_dir).join("MapleStory.exe");
                let client_version = get_exe_version(&game_exe);
                // Try to get server version from MapleStory login server
                let server_version = get_server_version().await;
                let payload = serde_json::json!({
                    "clientVersion": client_version,
                    "serverVersion": server_version,
                });
                let _ = app.emit("patcher-killed", payload);
                return;
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = game_dir;
        let _ = app;
    }
}

/// Get the product version string from a Windows PE executable.
/// Returns something like "1.2.437.1" or empty string on failure.
fn get_exe_version(path: &std::path::Path) -> String {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let size = windows_sys::Win32::Storage::FileSystem::GetFileVersionInfoSizeW(
                wide.as_ptr(),
                std::ptr::null_mut(),
            );
            if size == 0 {
                return String::new();
            }
            let mut buf = vec![0u8; size as usize];
            if windows_sys::Win32::Storage::FileSystem::GetFileVersionInfoW(
                wide.as_ptr(),
                0,
                size,
                buf.as_mut_ptr() as *mut _,
            ) == 0
            {
                return String::new();
            }
            let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut len: u32 = 0;
            let sub: Vec<u16> = OsStr::new("\\")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            if windows_sys::Win32::Storage::FileSystem::VerQueryValueW(
                buf.as_ptr() as *const _,
                sub.as_ptr(),
                &mut ptr,
                &mut len,
            ) == 0
            {
                return String::new();
            }
            let info = &*(ptr as *const windows_sys::Win32::Storage::FileSystem::VS_FIXEDFILEINFO);
            let major = info.dwProductVersionMS & 0xFFFF; // ProductMinorPart
            let minor = (info.dwProductVersionLS >> 16) & 0xFFFF; // FileBuildPart
            format!("{major}.{minor}")
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        String::new()
    }
}

/// Get MapleStory server version by connecting to the login server.
/// Reads the handshake packet: skip 2 bytes, read u16 major, read maple string minor.
async fn get_server_version() -> String {
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpStream;

    let result: Result<String, Box<dyn std::error::Error + Send + Sync>> = async {
        let mut stream = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            TcpStream::connect("tw.login.maplestory.beanfun.com:8484"),
        )
        .await??;

        let mut buf = [0u8; 256];
        let n = tokio::time::timeout(std::time::Duration::from_secs(3), stream.read(&mut buf))
            .await??;

        if n < 6 {
            return Ok(String::new());
        }

        // Handshake: [u16 packet_len] [u16 major_version] [u16 str_len] [str minor] ...
        let major = u16::from_le_bytes([buf[2], buf[3]]);
        let str_len = u16::from_le_bytes([buf[4], buf[5]]) as usize;
        let minor = if n >= 6 + str_len {
            String::from_utf8_lossy(&buf[6..6 + str_len]).to_string()
        } else {
            String::new()
        };

        let minor_clean = minor.split(':').next().unwrap_or("").to_string();
        if minor_clean.is_empty() {
            Ok(format!("{major}"))
        } else {
            Ok(format!("{major}.{minor_clean}"))
        }
    }
    .await;

    match result {
        Ok(v) => {
            tracing::info!("MapleStory server version: {v}");
            v
        }
        Err(e) => {
            tracing::warn!("failed to get server version: {e}");
            String::new()
        }
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
pub fn find_process_pid_by_name(name: &str) -> Option<u32> {
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
pub fn find_process_pid_by_name(_name: &str) -> Option<u32> {
    None
}

/// Auto-close MapleStory's "Play" startup window (StartUpDlgClass).
///
/// The game shows a launcher window with a Play button before the actual
/// game client starts. This polls every 100ms for up to 60 seconds and
/// sends WM_CLOSE to dismiss it automatically.
/// Matches the original C# Beanfun `checkPlayPage_Tick` / `skipPlayWnd`.
pub async fn skip_play_window_loop() {
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
/// Uses CreateProcessW to avoid console window flash and capture the child PID.
/// TW non-traditional mode passes server/port/account/otp args.
pub async fn launch_with_lr(
    app: &tauri::AppHandle,
    launch_cmd: &game_launcher::LaunchCommand,
    traditional_login: bool,
    _region: &Region,
    credentials: &GameCredentials,
) -> Result<u32, ProcessError> {
    let lr_proc = lr_service::ensure_lr_files(app).await?;
    let lr_path = lr_proc.to_string_lossy().to_string();

    let use_cmd_args =
        !traditional_login && !credentials.account_id.is_empty() && !credentials.otp.is_empty();

    // Build LR arguments: GUID + game path (+ optional server/port/account/otp)
    let lr_args = if use_cmd_args {
        // Use the command line template from the beanfun service if available,
        // replacing %s placeholders with account_id and otp (matching original
        // Beanfun behavior).  Only supported for TW region.
        let template = credentials
            .command_line_template
            .as_deref()
            .unwrap_or("tw.login.maplestory.beanfun.com 8484 BeanFun %s %s");
        let cmd_line = template
            .replacen("%s", &credentials.account_id, 1)
            .replacen("%s", &credentials.otp, 1);
        format!(
            "{} \"{}\" {}",
            lr_service::LR_PROFILE_GUID,
            launch_cmd.executable,
            cmd_line,
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
