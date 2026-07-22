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

/// Non-cryptographic digest of a byte slice, for the freshness check only.
///
/// Both the embedded copy and the on-disk copy are hashed by the same binary in
/// the same run, so `DefaultHasher`'s lack of cross-version stability doesn't
/// matter (nothing is persisted). This detects an outdated or accidentally
/// corrupted file — it is NOT tamper detection: anyone able to rewrite the file
/// can also match the hash. `%appdata%\lr\` is not a trust boundary.
fn content_hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// What `sync_lr_file` did with one file.
#[derive(Debug, PartialEq, Eq)]
enum SyncOutcome {
    /// The file was written (missing before, or content differed).
    Written,
    /// On-disk content already matched — nothing written.
    Unchanged,
    /// Content differed but the existing file couldn't be replaced (e.g. locked
    /// by a running game). The old copy is left in place; the launch continues.
    UpdateSkipped,
}

/// Bring one embedded LR file up to date in `lr_dir`, writing atomically.
///
/// Writes only when the on-disk content differs from `data` (compared by
/// [`content_hash`]), so an up-to-date file is never touched and can't collide
/// with a lock held by a running game. The write goes to a `.tmp` sibling and is
/// renamed over the destination, so an interrupted write can never leave a
/// half-written binary in place.
///
/// A failure to replace an *existing* file is non-fatal ([`SyncOutcome::UpdateSkipped`]):
/// the previous copy still works, so the launch proceeds. Only failing to create
/// a *missing* file aborts, since LR then has nothing to run.
async fn sync_lr_file(
    lr_dir: &std::path::Path,
    filename: &str,
    data: &[u8],
) -> Result<SyncOutcome, ProcessError> {
    let dest = lr_dir.join(filename);

    let existing = tokio::fs::read(&dest).await.ok();
    if let Some(bytes) = &existing {
        if content_hash(bytes) == content_hash(data) {
            return Ok(SyncOutcome::Unchanged);
        }
    }

    // Atomic replace: write a sibling, then rename over the destination. On
    // Windows the rename replaces an existing file and fails if it's locked.
    let mut tmp = dest.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);

    let write_and_swap = async {
        tokio::fs::write(&tmp, data).await?;
        tokio::fs::rename(&tmp, &dest).await
    };

    match write_and_swap.await {
        Ok(()) => {
            tracing::info!(file = %filename, "extracted embedded LR file");
            Ok(SyncOutcome::Written)
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await; // clean up a partial temp
            if existing.is_some() {
                // The old copy is still usable — keep running rather than block
                // the launch. This is the "DLL locked by a running game" path.
                tracing::warn!(
                    file = %filename,
                    "could not update LR file (in use?); keeping the existing copy: {e}"
                );
                Ok(SyncOutcome::UpdateSkipped)
            } else {
                Err(ProcessError::SpawnFailed {
                    path: dest.display().to_string(),
                    reason: format!("Failed to write LR file: {e}"),
                })
            }
        }
    }
}

/// Extract embedded LR files to the app data directory.
///
/// Returns the path to `LRProc.exe`. All LR files are placed in
/// `<app_data_dir>/lr/` so they are co-located as required by LRProc.
///
/// Each file is refreshed only when its on-disk content differs from the
/// embedded copy (see [`sync_lr_file`]), so identical files are never rewritten
/// and an in-use file is left alone.
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

    #[test]
    fn hash_matches_for_equal_content_differs_otherwise() {
        assert_eq!(content_hash(b"abc"), content_hash(b"abc"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abd"));
        // Same length, different bytes — the size-only check missed exactly this.
        assert_ne!(content_hash(&[0u8; 4]), content_hash(&[1u8; 4]));
    }

    #[tokio::test]
    async fn writes_a_missing_file() {
        let dir = scratch("missing");
        let out = sync_lr_file(&dir, "f.bin", b"hello").await.unwrap();
        assert_eq!(out, SyncOutcome::Written);
        assert_eq!(std::fs::read(dir.join("f.bin")).unwrap(), b"hello");
    }

    #[tokio::test]
    async fn leaves_identical_content_untouched() {
        let dir = scratch("identical");
        std::fs::write(dir.join("f.bin"), b"hello").unwrap();
        let out = sync_lr_file(&dir, "f.bin", b"hello").await.unwrap();
        assert_eq!(out, SyncOutcome::Unchanged);
    }

    #[tokio::test]
    async fn rewrites_when_content_changed_at_same_length() {
        let dir = scratch("same_len");
        std::fs::write(dir.join("f.bin"), b"AAAA").unwrap();
        // Same size as before — the old size-only check would have skipped this.
        let out = sync_lr_file(&dir, "f.bin", b"BBBB").await.unwrap();
        assert_eq!(out, SyncOutcome::Written);
        assert_eq!(std::fs::read(dir.join("f.bin")).unwrap(), b"BBBB");
    }

    #[tokio::test]
    async fn leaves_no_temp_file_behind() {
        let dir = scratch("no_temp");
        sync_lr_file(&dir, "f.bin", b"data").await.unwrap();
        assert!(!dir.join("f.bin.tmp").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }
}
