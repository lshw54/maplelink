//! Process management — spawning, monitoring (PID tracking), and termination.

use std::process::Command;

use crate::core::error::ProcessError;

/// Spawns a process and returns its PID.
///
/// Uses `CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP` on Windows
/// so the child process survives when MapleLink exits.
///
/// # Errors
///
/// Returns [`ProcessError::SpawnFailed`] if the process cannot be started.
pub async fn spawn_process(
    executable: &str,
    working_dir: &str,
    args: &[String],
) -> Result<u32, ProcessError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_BREAKAWAY_FROM_JOB (0x01000000) + CREATE_NEW_PROCESS_GROUP (0x00000200)
        let child = Command::new(executable)
            .current_dir(working_dir)
            .args(args)
            .creation_flags(0x01000200)
            .spawn()
            .map_err(|e| ProcessError::SpawnFailed {
                path: executable.to_string(),
                reason: e.to_string(),
            })?;
        Ok(child.id())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let child = Command::new(executable)
            .current_dir(working_dir)
            .args(args)
            .spawn()
            .map_err(|e| ProcessError::SpawnFailed {
                path: executable.to_string(),
                reason: e.to_string(),
            })?;
        Ok(child.id())
    }
}

/// Checks if any process with the given executable name is running.
pub fn is_process_name_running(name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {name}"), "/NH", "/FO", "CSV"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.to_lowercase().contains(&name.to_lowercase())
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = name;
        false
    }
}

/// Checks if a process with the given PID is still running.
///
/// Uses a platform-specific approach:
/// - On Windows: runs `tasklist /FI "PID eq <pid>"` and checks the output.
/// - On other platforms: always returns `false` (Windows-only application).
pub fn is_process_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // tasklist CSV output contains the PID as a quoted field when the
                // process exists. If no matching process is found the output
                // contains "INFO: No tasks are running..." instead.
                stdout.contains(&format!("\"{pid}\""))
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        // MapleLink is Windows-only; stub for compilation on other platforms.
        let _ = pid;
        false
    }
}

/// Terminates a process by PID.
///
/// Uses a platform-specific approach:
/// - On Windows: runs `taskkill /PID <pid> /F`.
/// - On other platforms: returns an error (Windows-only application).
///
/// # Errors
///
/// Returns [`ProcessError::SpawnFailed`] if the termination command fails.
pub async fn terminate_process(pid: u32) -> Result<(), ProcessError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let output = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| ProcessError::SpawnFailed {
                path: "taskkill".to_string(),
                reason: e.to_string(),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ProcessError::SpawnFailed {
                path: format!("taskkill /PID {pid}"),
                reason: stderr.trim().to_string(),
            })
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // MapleLink is Windows-only; stub for compilation on other platforms.
        Err(ProcessError::SpawnFailed {
            path: format!("kill {pid}"),
            reason: "Process termination is only supported on Windows".to_string(),
        })
    }
}

/// Hand a URL to whatever the OS has registered for its scheme.
///
/// Deliberately not `open::that`: that goes through `cmd /c start` on Windows,
/// and the Gamania Games Manager's URLs separate their fields with `&&&&`,
/// which the shell reads as command separators and chops the URL apart. Nothing
/// is launched and nothing reports an error. `ShellExecuteW` takes the string
/// as given.
#[cfg(target_os = "windows")]
pub fn open_uri(uri: &str) -> Result<(), ProcessError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = OsStr::new(uri)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Documented contract: a value above 32 means the handler was started.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    if result as usize > 32 {
        Ok(())
    } else {
        Err(ProcessError::SpawnFailed {
            path: uri.to_string(),
            reason: format!("ShellExecuteW returned {}", result as usize),
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn open_uri(uri: &str) -> Result<(), ProcessError> {
    open::that(uri).map_err(|e| ProcessError::SpawnFailed {
        path: uri.to_string(),
        reason: e.to_string(),
    })
}

/// Path of the Gamania Games Manager's web-start executable, if installed.
#[cfg(target_os = "windows")]
pub fn ggm_webstart_path() -> Option<std::path::PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for flags in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
        let Ok(key) = hklm.open_subkey_with_flags(r"SOFTWARE\gamaniaGamesManager", flags) else {
            continue;
        };
        let Ok(install) = key.get_value::<String, _>("InstallPath") else {
            continue;
        };
        let exe = std::path::Path::new(&install).join("GGMWebStart.exe");
        if exe.exists() {
            return Some(exe);
        }
    }
    None
}

/// Hand a launch URL to the game manager, preferring its executable over the
/// shell.
///
/// Going through the registered protocol handler works, but the shell decides
/// how the process is started and a console flashes on the way. Starting
/// `GGMWebStart.exe` ourselves — the very program the handler points at — keeps
/// that under our control. The shell stays as the fallback for a machine where
/// the install can't be located.
#[cfg(target_os = "windows")]
pub fn open_ggm_uri(uri: &str) -> Result<(), ProcessError> {
    use std::os::windows::process::CommandExt;

    /// CREATE_NO_WINDOW — no console for a process that has no use for one.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    if let Some(exe) = ggm_webstart_path() {
        match Command::new(&exe)
            .arg(uri)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
        {
            Ok(_) => {
                tracing::info!(exe = %exe.display(), "ggm: started the game manager directly");
                return Ok(());
            }
            Err(e) => tracing::warn!("ggm: could not start {}: {e}", exe.display()),
        }
    }
    tracing::info!("ggm: falling back to the registered protocol handler");
    open_uri(uri)
}

#[cfg(not(target_os = "windows"))]
pub fn open_ggm_uri(uri: &str) -> Result<(), ProcessError> {
    open_uri(uri)
}

/// Whether the Gamania Games Manager is installed.
///
/// Two independent signs, because either can be missing on a working install:
/// the manager's own registry key, and the `gamaniagames://` handler that
/// beanfun's site opens. One of them is enough.
#[cfg(target_os = "windows")]
pub fn ggm_installed() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    if ggm_webstart_path().is_some() {
        return true;
    }
    RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"gamaniagames\shell\open\command")
        .and_then(|key| key.get_value::<String, _>(""))
        .map(|cmd| !cmd.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub fn ggm_installed() -> bool {
    false
}
