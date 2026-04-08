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

/// Extract embedded LR files to the app data directory.
///
/// Returns the path to `LRProc.exe`. All LR files are placed in
/// `<app_data_dir>/lr/` so they are co-located as required by LRProc.
///
/// Files are only written when missing or their size differs from the
/// embedded version (simple staleness check).
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
        let dest = lr_dir.join(filename);

        // Write if destination is missing or size differs.
        let should_write = match tokio::fs::metadata(&dest).await {
            Ok(meta) => meta.len() != data.len() as u64,
            Err(_) => true,
        };

        if should_write {
            tokio::fs::write(&dest, data)
                .await
                .map_err(|e| ProcessError::SpawnFailed {
                    path: dest.display().to_string(),
                    reason: format!("Failed to write LR file: {e}"),
                })?;
            tracing::info!(file = %filename, "extracted embedded LR file");
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
