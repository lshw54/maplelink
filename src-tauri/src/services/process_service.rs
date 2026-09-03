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
fn open_uri(uri: &str) -> Result<(), ProcessError> {
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
fn open_uri(uri: &str) -> Result<(), ProcessError> {
    open::that(uri).map_err(|e| ProcessError::SpawnFailed {
        path: uri.to_string(),
        reason: e.to_string(),
    })
}

/// Path of the Gamania Games Manager's web-start executable, if installed.
#[cfg(target_os = "windows")]
fn ggm_webstart_path() -> Option<std::path::PathBuf> {
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

/// What beanfun's credential endpoint asks the caller to state about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIntegrity {
    /// `GGMWebStart.dll`'s version.
    pub cv: String,
    /// That same file's SHA-256, lowercase hex.
    pub hash: String,
    /// `x64` or `x86`.
    pub arch: String,
}

/// The values for the build these were taken from, used when the game manager
/// isn't installed.
///
/// They are constants — a version string and the hash of one file in a release
/// — so shipping the file itself would buy nothing over writing them down:
/// nothing here executes it, it is only ever read to produce these two strings.
/// Refresh them from a newer `GGMWebStart.dll` if beanfun starts requiring one.
const GGM_FALLBACK_CV: &str = "1.5.0.2";
const GGM_FALLBACK_HASH: &str = "dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06";

/// The architecture this process reports as.
pub fn arch() -> String {
    if cfg!(target_pointer_width = "64") {
        "x64".to_string()
    } else {
        "x86".to_string()
    }
}

/// What to tell beanfun about the client asking.
///
/// Prefers the game manager installed on this machine, so a user who has it
/// tracks whatever build they are on without us shipping an update. Falls back
/// to the values above, so a user who doesn't have it — most of them — needs
/// nothing installed at all.
/// Values from a `GGMWebStart.dll` the user placed in the data folder.
///
/// Putting it there is a deliberate act, so nothing else overrides it.
pub fn dropped_client_integrity() -> Option<ClientIntegrity> {
    let dropped = dropped_ggm_dll().and_then(|p| integrity_from_dll(&p))?;
    tracing::info!(cv = %dropped.cv, "ggm: using the {GGM_DLL} placed in the data folder");
    Some(dropped)
}

/// Values from the game manager installed on this machine.
///
/// A real build, but not necessarily a current one: the manager updates itself
/// when it runs, and a copy that hasn't been run in months reports whatever it
/// was then. So this is consulted after the published pair, not before.
pub fn installed_client_integrity() -> Option<ClientIntegrity> {
    let installed = installed_ggm_integrity()?;
    tracing::debug!(cv = %installed.cv, "ggm: using the installed game manager");
    Some(installed)
}

/// The pair compiled in, used when nothing better is available.
pub fn builtin_client_integrity() -> ClientIntegrity {
    ClientIntegrity {
        cv: GGM_FALLBACK_CV.to_string(),
        hash: GGM_FALLBACK_HASH.to_string(),
        arch: arch(),
    }
}

/// The file name a user drops in to override these values.
const GGM_DLL: &str = "GGMWebStart.dll";

/// A copy of the manager's library placed in MapleLink's own data folder.
///
/// The escape hatch for the day beanfun requires a build newer than the one
/// compiled in: drop the file in and the values follow it, with no new release
/// of MapleLink and no install of the manager. Checked first for exactly that
/// reason — someone who put it there meant it.
fn dropped_ggm_dll() -> Option<std::path::PathBuf> {
    let dir = std::env::var("APPDATA").ok()?;
    let path = std::path::Path::new(&dir)
        .join("com.maplelink.app")
        .join(GGM_DLL);
    path.is_file().then_some(path)
}

/// Read the version and hash out of a copy of the manager's library.
#[cfg(target_os = "windows")]
fn integrity_from_dll(dll: &std::path::Path) -> Option<ClientIntegrity> {
    use sha2::{Digest, Sha256};

    let bytes = std::fs::read(dll).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    Some(ClientIntegrity {
        cv: file_version(dll)?,
        hash,
        arch: arch(),
    })
}

#[cfg(not(target_os = "windows"))]
fn integrity_from_dll(_dll: &std::path::Path) -> Option<ClientIntegrity> {
    None
}

/// Read the values off the installed game manager, if it is there.
#[cfg(target_os = "windows")]
fn installed_ggm_integrity() -> Option<ClientIntegrity> {
    integrity_from_dll(&ggm_webstart_path()?.with_file_name(GGM_DLL))
}

/// The `FileVersion` of a Windows binary, as `a.b.c.d`.
#[cfg(target_os = "windows")]
fn file_version(path: &std::path::Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut buffer = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, buffer.as_mut_ptr().cast()) == 0 {
            return None;
        }

        let root: Vec<u16> = "\\".encode_utf16().chain(std::iter::once(0)).collect();
        let mut info: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len: u32 = 0;
        if VerQueryValueW(buffer.as_ptr().cast(), root.as_ptr(), &mut info, &mut len) == 0
            || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return None;
        }

        let fixed = &*(info as *const VS_FIXEDFILEINFO);
        Some(format!(
            "{}.{}.{}.{}",
            fixed.dwFileVersionMS >> 16,
            fixed.dwFileVersionMS & 0xFFFF,
            fixed.dwFileVersionLS >> 16,
            fixed.dwFileVersionLS & 0xFFFF
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn installed_ggm_integrity() -> Option<ClientIntegrity> {
    None
}
