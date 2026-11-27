//! Locale Remulator (LR) file management. The embedded files are extracted at
//! runtime to a directory whose path survives LR's ANSI conversions.
//!
//! LR spoofs the system locale so MapleStory runs under Traditional Chinese
//! code-page (950) without requiring the user to change their Windows locale.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::core::error::ProcessError;
use crate::utils::ascii_path;

/// The LR profile GUID for "Run in Taiwan (Admin)" in `LRConfig.xml`.
pub const LR_PROFILE_GUID: &str = "ef3e7b42-a87c-4c07-ae3e-eeebeef12762";

/// Shared roots to try when the per-user path is unsuitable or unwritable.
const ASCII_ROOTS: [&str; 3] = ["ProgramData", "ALLUSERSPROFILE", "SystemDrive"];

/// Folder we create under an [`ASCII_ROOTS`] entry. Kept in sync with the
/// uninstaller (`installer/hooks.nsh`), which deletes it.
const ASCII_FALLBACK_DIR: &str = "MapleLink";

/// The bundled x64 LRProc and LRHookx64 import this runtime.
#[cfg(any(target_os = "windows", test))]
const VC_RUNTIME_DLL: &str = "VCRUNTIME140.dll";

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

/// Replace one file atomically. Each extraction uses its own staging file
/// because the UI and the headless launcher can run at the same time.
fn sync_lr_file(lr_dir: &Path, filename: &str, data: &[u8]) -> io::Result<()> {
    let dest = lr_dir.join(filename);
    let result = (|| {
        let staged = lr_dir.join(format!("{filename}.{}.tmp", uuid::Uuid::new_v4()));
        let mut file = fs::File::create_new(&staged)?;
        let write_result = file.write_all(data);
        drop(file);

        let result = write_result.and_then(|()| fs::rename(&staged, &dest));
        if result.is_err() {
            let _ = fs::remove_file(&staged);
        }
        result
    })();

    // An existing hook can be locked by a running game, and a shared directory
    // prepared by the elevated UI can be read-only for the headless launcher.
    // In either case only allow the existing file if it matches this build.
    result.or_else(|error| {
        if lr_file_matches(&dest, data) {
            Ok(())
        } else {
            Err(error)
        }
    })
}

fn lr_file_matches(path: &Path, data: &[u8]) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() == data.len() as u64)
        && fs::read(path).is_ok_and(|bytes| bytes == data)
}

fn lr_dir_candidates(app_data_dir: &Path, fallback_root: &str) -> Vec<PathBuf> {
    let mut candidates = vec![app_data_dir.join("lr")];
    if !fallback_root.is_empty() {
        let root = Path::new(fallback_root);
        match check_lr_dir(root) {
            Ok(()) => candidates.push(root.join(ASCII_FALLBACK_DIR).join("lr")),
            Err(error) => {
                tracing::warn!(dir = %root.display(), "ignoring invalid LR fallback directory: {error}");
            }
        }
    }
    for var in ASCII_ROOTS {
        if let Some(root) = ascii_path::env_root(var) {
            let dir = root.join(ASCII_FALLBACK_DIR).join("lr");
            if !candidates.contains(&dir) {
                candidates.push(dir);
            }
        }
    }
    candidates.retain(|dir| dir.is_absolute() && ascii_path::is_ascii(dir));
    candidates
}

/// LRProc uses the CLR assembly location to locate its hooks and Detours
/// passes the DLL path through ANSI. Validate the real directory too so a
/// short name or junction cannot send LR back to a Unicode profile path.
pub(crate) fn check_lr_dir(dir: &Path) -> io::Result<()> {
    if !dir.is_absolute()
        || !ascii_path::is_ascii(dir)
        || dir
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .any(u8::is_ascii_control)
        || !ascii_path::is_ascii(&fs::canonicalize(dir)?)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Locale Remulator requires an absolute ASCII directory",
        ));
    }
    if !fs::metadata(dir)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "Locale Remulator needs a folder",
        ));
    }
    Ok(())
}

fn prepare_lr_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    check_lr_dir(dir)?;
    for &(filename, data) in EMBEDDED_LR {
        sync_lr_file(dir, filename, data)
            .map_err(|error| io::Error::new(error.kind(), format!("{filename}: {error}")))?;
    }
    Ok(())
}

/// Extract the full LR bundle before returning its executable. A directory
/// may exist even when the launcher cannot write its files, so try the next
/// location if preparing the bundle fails.
pub fn ensure_lr_files_in(
    app_data_dir: &Path,
    fallback_root: &str,
) -> Result<PathBuf, ProcessError> {
    prepare_lr_from_candidates(
        app_data_dir,
        &lr_dir_candidates(app_data_dir, fallback_root),
    )
}

fn prepare_lr_from_candidates(
    app_data_dir: &Path,
    candidates: &[PathBuf],
) -> Result<PathBuf, ProcessError> {
    let mut failures = Vec::new();
    for dir in candidates {
        match prepare_lr_dir(dir) {
            Ok(()) => {
                tracing::debug!(dir = %dir.display(), "Locale Remulator files ready");
                return Ok(dir.join("LRProc.exe"));
            }
            Err(error) => {
                tracing::warn!(dir = %dir.display(), "could not prepare LR files: {error}");
                failures.push(format!("{}: {error}", dir.display()));
            }
        }
    }

    let primary = app_data_dir.join("lr");
    let reason = if failures.is_empty() {
        "No absolute ASCII directory is available for Locale Remulator".to_owned()
    } else {
        format!(
            "Failed to prepare Locale Remulator: {}",
            failures.join("; ")
        )
    };
    Err(ProcessError::SpawnFailed {
        path: primary.display().to_string(),
        reason,
    })
}

/// Prepare LR for the in-app launcher without blocking the async runtime.
pub async fn ensure_lr_files(app_handle: &tauri::AppHandle) -> Result<PathBuf, ProcessError> {
    use tauri::Manager;

    let app_data_dir =
        app_handle
            .path()
            .app_data_dir()
            .map_err(|error| ProcessError::SpawnFailed {
                path: "app_data_dir".to_string(),
                reason: format!("Failed to resolve app data directory: {error}"),
            })?;

    let state = app_handle.state::<crate::models::app_state::AppState>();
    let fallback_root = state.config.read().await.lr_fallback_dir.clone();
    tokio::task::spawn_blocking(move || ensure_lr_files_in(&app_data_dir, &fallback_root))
        .await
        .map_err(|error| ProcessError::SpawnFailed {
            path: "LRProc.exe".to_owned(),
            reason: format!("LR extraction task failed: {error}"),
        })?
}

/// Report readiness only when a complete, current bundle exists.
#[cfg(target_os = "windows")]
pub fn find_lr_proc(app_data_dir: &Path, fallback_root: &str) -> Option<PathBuf> {
    lr_proc_in(&lr_dir_candidates(app_data_dir, fallback_root))
}

#[cfg(any(target_os = "windows", test))]
fn lr_proc_in(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|dir| {
            check_lr_dir(dir).is_ok()
                && EMBEDDED_LR
                    .iter()
                    .all(|(filename, data)| lr_file_matches(&dir.join(filename), data))
        })
        .map(|dir| dir.join("LRProc.exe"))
}

/// Catch a missing runtime before Windows starts LRProc or the injected game.
/// The game directory and inherited PATH also support app-local runtimes.
#[cfg(target_os = "windows")]
pub fn check_launch_runtime(game_path: &Path) -> Result<(), ProcessError> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

    let mut buffer = [0u16; 260];
    let len = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if len == 0 || len >= buffer.len() {
        tracing::warn!("could not resolve the Windows system directory for the LR runtime check");
        return Ok(());
    }

    let system_dir = PathBuf::from(OsString::from_wide(&buffer[..len]));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs = std::iter::once(system_dir)
        .chain(game_path.parent().map(Path::to_path_buf))
        .chain(std::env::split_paths(&path).filter(|dir| dir.is_absolute()));
    let result = check_runtime_in(dirs);
    if let Err(error) = &result {
        tracing::warn!(game = %game_path.display(), "LR launch prerequisite failed: {error}");
    }
    result
}

#[cfg(any(target_os = "windows", test))]
fn check_runtime_in(dirs: impl IntoIterator<Item = PathBuf>) -> Result<(), ProcessError> {
    if dirs
        .into_iter()
        .any(|dir| dir.join(VC_RUNTIME_DLL).is_file())
    {
        Ok(())
    } else {
        Err(ProcessError::MissingVcRuntime)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn check_launch_runtime(_game_path: &Path) -> Result<(), ProcessError> {
    Ok(())
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

    const MIXED_CHINESE_PROFILE: &str = "简体用户（管理員）";

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("maplelink_lr_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_runtime_reports_an_actionable_process_error() {
        let system_dir = scratch();
        let game_dir = scratch();
        let dirs = [system_dir, game_dir.clone()];

        assert!(matches!(
            check_runtime_in(dirs.clone()),
            Err(ProcessError::MissingVcRuntime)
        ));
        fs::write(game_dir.join(VC_RUNTIME_DLL), b"runtime fixture").unwrap();
        assert!(check_runtime_in(dirs).is_ok());
    }

    #[test]
    fn writes_a_missing_file() {
        let dir = scratch();
        sync_lr_file(&dir, "f.bin", b"hello").unwrap();
        assert_eq!(fs::read(dir.join("f.bin")).unwrap(), b"hello");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn rewrites_when_content_changed_at_same_length() {
        let dir = scratch();
        fs::write(dir.join("f.bin"), b"AAAA").unwrap();
        sync_lr_file(&dir, "f.bin", b"BBBB").unwrap();
        assert_eq!(fs::read(dir.join("f.bin")).unwrap(), b"BBBB");
    }

    #[test]
    fn an_ascii_profile_prefers_app_data() {
        let primary = scratch();
        let fallback = scratch();
        let dirs = lr_dir_candidates(&primary, fallback.to_str().unwrap());
        assert_eq!(dirs.first(), Some(&primary.join("lr")));
        assert_eq!(
            dirs.get(1),
            Some(&fallback.join(ASCII_FALLBACK_DIR).join("lr"))
        );
    }

    #[test]
    fn non_ascii_profiles_are_never_candidates() {
        let root = scratch();
        for profile in [
            MIXED_CHINESE_PROFILE,
            "测试 user",
            "Dj小明",
            "繁體",
            "ＭａｐｌｅＬｉｎｋ",
        ] {
            let requested = root.join(profile).join("AppData").join("Roaming");
            let candidates = lr_dir_candidates(&requested, "");
            assert!(!candidates.contains(&requested.join("lr")));
            for dir in candidates {
                assert!(dir.is_absolute() && ascii_path::is_ascii(&dir));
                assert!(dir.ends_with(Path::new(ASCII_FALLBACK_DIR).join("lr")));
            }
        }
    }

    #[test]
    fn a_chinese_profile_prefers_the_configured_backup_folder() {
        let root = scratch();
        let app_data = root.join(MIXED_CHINESE_PROFILE).join("AppData/Roaming");
        let fallback = root.join("LR Backup");
        fs::create_dir(&fallback).unwrap();
        let dirs = lr_dir_candidates(&app_data, fallback.to_str().unwrap());
        let expected = fallback.join(ASCII_FALLBACK_DIR).join("lr");

        assert_eq!(dirs.first(), Some(&expected));
        let chosen = prepare_lr_from_candidates(&app_data, &dirs[..1]).unwrap();
        assert_eq!(chosen, expected.join("LRProc.exe"));
        assert_eq!(lr_proc_in(&dirs[..1]), Some(chosen));
    }

    #[test]
    fn a_removed_backup_folder_is_skipped() {
        let root = scratch();
        let fallback = root.join("Missing Backup");
        let automatic = lr_dir_candidates(&root, "");
        assert_eq!(
            lr_dir_candidates(&root, fallback.to_str().unwrap()),
            automatic
        );
    }

    #[test]
    fn readiness_rejects_a_legacy_bundle_under_a_chinese_profile() {
        let legacy = scratch()
            .join(MIXED_CHINESE_PROFILE)
            .join("AppData/Roaming/com.maplelink.app/lr");
        fs::create_dir_all(&legacy).unwrap();
        for &(filename, data) in EMBEDDED_LR {
            fs::write(legacy.join(filename), data).unwrap();
        }

        assert_eq!(lr_proc_in(&[legacy]), None);
    }

    #[test]
    fn readiness_requires_the_complete_current_bundle() {
        let dir = scratch();
        fs::write(dir.join("LRProc.exe"), EMBEDDED_LR[0].1).unwrap();
        assert_eq!(lr_proc_in(std::slice::from_ref(&dir)), None);

        prepare_lr_dir(&dir).unwrap();
        assert_eq!(
            lr_proc_in(std::slice::from_ref(&dir)),
            Some(dir.join("LRProc.exe")),
        );

        fs::write(dir.join("LRConfig.xml"), b"old").unwrap();
        assert_eq!(lr_proc_in(&[dir]), None);
    }

    #[test]
    fn extraction_retries_if_a_directory_exists_but_cannot_hold_the_bundle() {
        let root = scratch();
        let app_data = root
            .join(MIXED_CHINESE_PROFILE)
            .join("AppData/Roaming/com.maplelink.app");
        let blocked = root.join("blocked");
        fs::create_dir_all(blocked.join("LRHookx32.dll")).unwrap();
        let backup = root.join("backup");
        let legacy = app_data.join("lr");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("LRProc.exe"), b"legacy").unwrap();

        let chosen = prepare_lr_from_candidates(
            &app_data,
            &[blocked.clone(), legacy.clone(), backup.clone()],
        )
        .unwrap();
        assert_eq!(chosen, backup.join("LRProc.exe"));
        assert_eq!(lr_proc_in(&[blocked, backup]), Some(chosen));
        assert_eq!(fs::read(legacy.join("LRProc.exe")).unwrap(), b"legacy");
    }

    #[test]
    fn extraction_reports_failed_locations_and_never_uses_a_chinese_path() {
        let root = scratch();
        let app_data = root.join("简体用户");
        let blocked = root.join("wall");
        fs::write(&blocked, b"x").unwrap();

        let error = prepare_lr_from_candidates(
            &app_data,
            &[
                blocked.join("first"),
                blocked.join("second"),
                app_data.join("lr"),
            ],
        )
        .unwrap_err();
        let ProcessError::SpawnFailed { path, reason } = error else {
            panic!("unexpected preparation error: {error}");
        };
        assert_eq!(path, app_data.join("lr").display().to_string());
        assert!(
            reason.contains("first") && reason.contains("second"),
            "{reason}"
        );
        assert!(reason.contains("absolute ASCII directory"), "{reason}");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_locked_file_is_reused_only_if_it_matches_the_embedded_copy() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = scratch();
        let file = dir.join("f.bin");
        fs::write(&file, b"current").unwrap();
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&file)
            .unwrap();

        sync_lr_file(&dir, "f.bin", b"current").unwrap();
        assert!(sync_lr_file(&dir, "f.bin", b"changed").is_err());
        assert_eq!(fs::read(file).unwrap(), b"current");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn an_ascii_junction_to_a_chinese_directory_is_rejected() {
        let root = scratch();
        let target = root.join(MIXED_CHINESE_PROFILE);
        fs::create_dir(&target).unwrap();
        let junction = root.join("alias");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        assert!(status.status.success(), "{status:?}");

        let result = check_lr_dir(&junction);
        fs::remove_dir(&junction).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}
