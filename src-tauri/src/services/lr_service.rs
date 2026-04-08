//! Locale Remulator (LR) file management — extracting bundled LR files
//! to the app data directory and detecting system locale.
//!
//! LR spoofs the system locale so MapleStory runs under Traditional Chinese
//! code-page (950) without requiring the user to change their Windows locale.

use std::path::PathBuf;

use crate::core::error::ProcessError;

/// The LR profile GUID for "Run in Taiwan (Admin)" in `LRConfig.xml`.
pub const LR_PROFILE_GUID: &str = "ef3e7b42-a87c-4c07-ae3e-eeebeef12762";

/// LR files that must be co-located with `LRProc.exe`.
const LR_FILES: &[&str] = &[
    "LRProc.exe",
    "LRHookx32.dll",
    "LRHookx64.dll",
    "LRConfig.xml",
    "LRSubMenus.dll",
];

/// Extract LR files from bundled resources to the app data directory.
///
/// Returns the path to `LRProc.exe`. All LR files are placed in
/// `<app_data_dir>/lr/` so they are co-located as required by LRProc.
///
/// Files are only copied when they are missing or their size differs from
/// the bundled version (simple staleness check).
///
/// # Errors
///
/// Returns [`ProcessError::SpawnFailed`] if resource resolution or file I/O fails.
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

    for filename in LR_FILES {
        let dest = lr_dir.join(filename);
        let source = app_handle
            .path()
            .resolve(
                format!("resources/lr/{filename}"),
                tauri::path::BaseDirectory::Resource,
            )
            .map_err(|e| ProcessError::SpawnFailed {
                path: filename.to_string(),
                reason: format!("Failed to resolve bundled resource: {e}"),
            })?;

        // Copy if destination is missing or size differs from source.
        let should_copy = match (
            tokio::fs::metadata(&source).await,
            tokio::fs::metadata(&dest).await,
        ) {
            (Ok(src_meta), Ok(dst_meta)) => src_meta.len() != dst_meta.len(),
            (Ok(_), Err(_)) => true,
            (Err(e), _) => {
                return Err(ProcessError::SpawnFailed {
                    path: source.display().to_string(),
                    reason: format!("Bundled LR resource not found: {e}"),
                });
            }
        };

        if should_copy {
            tokio::fs::copy(&source, &dest)
                .await
                .map_err(|e| ProcessError::SpawnFailed {
                    path: dest.display().to_string(),
                    reason: format!("Failed to copy LR file: {e}"),
                })?;
            tracing::info!(file = %filename, "extracted LR file to app data");
        }
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

    // GetSystemDefaultLocaleName returns the locale name (e.g. "zh-TW").
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
