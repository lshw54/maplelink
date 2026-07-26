//! Locale Remulator (LR) file management — embedded LR binaries are
//! extracted to the app data directory at runtime.
//!
//! LR spoofs the system locale so MapleStory runs under Traditional Chinese
//! code-page (950) without requiring the user to change their Windows locale.

use std::path::PathBuf;

use crate::core::error::ProcessError;

/// The LR profile GUID for "Run in Taiwan (Admin)" in `LRConfig.xml`.
pub const LR_PROFILE_GUID: &str = "ef3e7b42-a87c-4c07-ae3e-eeebeef12762";

/// Embedded LR files — compiled directly into the binary so a standalone
/// `maplelink.exe` works without needing a `resources/lr/` folder alongside it.
const EMBEDDED_LR: &[(&str, &[u8])] = &[
    (
        "LRProc.exe",
        include_bytes!("../../resources/lr/LRProc.exe"),
    ),
    (
        "LRHookx32.dll",
        include_bytes!("../../resources/lr/LRHookx32.dll"),
    ),
    (
        "LRHookx64.dll",
        include_bytes!("../../resources/lr/LRHookx64.dll"),
    ),
    (
        "LRConfig.xml",
        include_bytes!("../../resources/lr/LRConfig.xml"),
    ),
    (
        "LRSubMenus.dll",
        include_bytes!("../../resources/lr/LRSubMenus.dll"),
    ),
];

/// What `sync_lr_file` did with one file.
#[derive(Debug, PartialEq, Eq)]
enum SyncOutcome {
    /// The embedded copy was written over whatever was there.
    Written,
    /// The file couldn't be replaced, but what's on disk is byte-identical to
    /// the embedded copy anyway — nothing is stale.
    AlreadyCurrent,
    /// The file couldn't be replaced and its content differs (locked by a
    /// running game). The old copy is left in place; the launch continues.
    UpdateSkipped,
}

/// Re-extract one embedded LR file into `lr_dir`, replacing what's there.
///
/// The on-disk copy is never trusted: every call rewrites the file, so a stale,
/// truncated or externally-modified binary can't survive. The write goes to a
/// `.tmp` sibling and is renamed over the destination, so an interrupted write
/// can never leave a half-written binary in place.
///
/// Windows refuses to rename over a file that is locked — typically
/// `LRHookx*.dll` mapped into a MapleStory process that is still running. In
/// that case the destination is first moved aside to `<name>.old` so the fresh
/// copy still lands (and is restored if that second step fails). Only if even
/// that fails do we keep the existing file: non-fatal when one exists
/// ([`SyncOutcome::UpdateSkipped`]), fatal when the file is missing entirely,
/// since LR then has nothing to run.
async fn sync_lr_file(
    lr_dir: &std::path::Path,
    filename: &str,
    data: &[u8],
) -> Result<SyncOutcome, ProcessError> {
    let dest = lr_dir.join(filename);

    let mut tmp = dest.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);

    let mut aside = dest.clone().into_os_string();
    aside.push(".old");
    let aside = PathBuf::from(aside);

    let write_and_swap = async {
        tokio::fs::write(&tmp, data).await?;
        tokio::fs::rename(&tmp, &dest).await
    };

    let err = match write_and_swap.await {
        Ok(()) => {
            // Drop any copy parked by an earlier locked run; best-effort, it may
            // still be held open.
            let _ = tokio::fs::remove_file(&aside).await;
            tracing::debug!(file = %filename, "extracted embedded LR file");
            return Ok(SyncOutcome::Written);
        }
        Err(e) => e,
    };

    // Couldn't replace the destination. If it already holds exactly the embedded
    // bytes there is nothing to fix, so don't fight the lock.
    if tokio::fs::read(&dest).await.ok().as_deref() == Some(data) {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Ok(SyncOutcome::AlreadyCurrent);
    }

    // Stale content behind a lock: park the locked file and move the fresh copy
    // into its place. Renaming an open file is allowed where replacing it isn't.
    let _ = tokio::fs::remove_file(&aside).await;
    if tokio::fs::rename(&dest, &aside).await.is_ok() {
        match tokio::fs::rename(&tmp, &dest).await {
            Ok(()) => {
                tracing::info!(file = %filename, "replaced in-use LR file (old copy moved aside)");
                return Ok(SyncOutcome::Written);
            }
            // Put the working copy back rather than leave nothing there.
            Err(_) => {
                let _ = tokio::fs::rename(&aside, &dest).await;
            }
        }
    }

    let _ = tokio::fs::remove_file(&tmp).await; // clean up the staged copy
    if tokio::fs::try_exists(&dest).await.unwrap_or(false) {
        // The old copy is still usable — keep running rather than block the
        // launch. This is the "DLL locked by a running game" path.
        tracing::warn!(
            file = %filename,
            "could not replace LR file (in use?); keeping the existing copy: {err}"
        );
        Ok(SyncOutcome::UpdateSkipped)
    } else {
        Err(ProcessError::SpawnFailed {
            path: dest.display().to_string(),
            reason: format!("Failed to write LR file: {err}"),
        })
    }
}

/// Extract embedded LR files to the app data directory.
///
/// Returns the path to `LRProc.exe`. All LR files are placed in
/// `<app_data_dir>/lr/` so they are co-located as required by LRProc.
///
/// Every file is re-extracted on each call (see [`sync_lr_file`]) rather than
/// compared first, so the shipped binaries always win over whatever is on disk.
pub async fn ensure_lr_files(app_handle: &tauri::AppHandle) -> Result<PathBuf, ProcessError> {
    use tauri::Manager;

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| ProcessError::SpawnFailed {
            path: "app_data_dir".to_string(),
            reason: format!("Failed to resolve app data directory: {e}"),
        })?;

    let lr_dir = app_data_dir.join("lr");
    tokio::fs::create_dir_all(&lr_dir)
        .await
        .map_err(|e| ProcessError::SpawnFailed {
            path: lr_dir.display().to_string(),
            reason: format!("Failed to create LR directory: {e}"),
        })?;

    for &(filename, data) in EMBEDDED_LR {
        sync_lr_file(&lr_dir, filename, data).await?;
    }

    Ok(lr_dir.join("LRProc.exe"))
}

/// Check if the system locale is Traditional Chinese.
///
/// Returns `true` for zh-TW, zh-HK, zh-MO, zh-CHT, zh-Hant locales,
/// meaning LR is not needed.
#[cfg(target_os = "windows")]
pub fn is_system_locale_chinese_traditional() -> bool {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH = 85
    let len = unsafe {
        windows_sys::Win32::Globalization::GetSystemDefaultLocaleName(
            buf.as_mut_ptr(),
            buf.len() as i32,
        )
    };

    if len <= 0 {
        return false;
    }

    let locale_name = OsString::from_wide(&buf[..((len - 1) as usize)])
        .to_string_lossy()
        .to_lowercase();

    matches!(
        locale_name.as_str(),
        "zh-tw" | "zh-hk" | "zh-mo" | "zh-cht" | "zh-hant"
    )
}

/// Stub for non-Windows platforms — always returns `false`.
#[cfg(not(target_os = "windows"))]
pub fn is_system_locale_chinese_traditional() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("maplelink_lr_test").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn writes_a_missing_file() {
        let dir = scratch("missing");
        let out = sync_lr_file(&dir, "f.bin", b"hello").await.unwrap();
        assert_eq!(out, SyncOutcome::Written);
        assert_eq!(std::fs::read(dir.join("f.bin")).unwrap(), b"hello");
    }

    #[tokio::test]
    async fn rewrites_even_when_content_is_identical() {
        let dir = scratch("identical");
        std::fs::write(dir.join("f.bin"), b"hello").unwrap();
        let out = sync_lr_file(&dir, "f.bin", b"hello").await.unwrap();
        assert_eq!(out, SyncOutcome::Written);
        assert_eq!(std::fs::read(dir.join("f.bin")).unwrap(), b"hello");
    }

    #[tokio::test]
    async fn rewrites_when_content_changed_at_same_length() {
        let dir = scratch("same_len");
        std::fs::write(dir.join("f.bin"), b"AAAA").unwrap();
        let out = sync_lr_file(&dir, "f.bin", b"BBBB").await.unwrap();
        assert_eq!(out, SyncOutcome::Written);
        assert_eq!(std::fs::read(dir.join("f.bin")).unwrap(), b"BBBB");
    }

    #[tokio::test]
    async fn leaves_no_temp_or_aside_file_behind() {
        let dir = scratch("no_temp");
        // A leftover .old from an earlier locked run is cleaned up on success.
        std::fs::write(dir.join("f.bin.old"), b"stale").unwrap();
        sync_lr_file(&dir, "f.bin", b"data").await.unwrap();
        assert!(!dir.join("f.bin.tmp").exists());
        assert!(!dir.join("f.bin.old").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }
}
