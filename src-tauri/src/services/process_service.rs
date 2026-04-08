//! Process management — spawning, monitoring (PID tracking), and termination.

use std::process::Command;

use crate::core::error::ProcessError;

/// Spawns a process and returns its PID.
///
/// # Errors
///
/// Returns [`ProcessError::SpawnFailed`] if the process cannot be started.
pub async fn spawn_process(
    executable: &str,
    working_dir: &str,
    args: &[String],
) -> Result<u32, ProcessError> {
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

/// Checks if a process with the given PID is still running.
///
/// Uses a platform-specific approach:
/// - On Windows: runs `tasklist /FI "PID eq <pid>"` and checks the output.
/// - On other platforms: always returns `false` (Windows-only application).
pub fn is_process_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
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
        let output = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
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
