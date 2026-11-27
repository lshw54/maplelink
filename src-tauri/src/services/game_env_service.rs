//! Game path resolution, registry detection and game cache cleanup.
//!
//! Extracted from `commands/system.rs` so registry/fs side effects live in
//! `services/` (Clean Architecture). Command handlers delegate here.

use std::path::Path;

use crate::core::error::{AppError, FsError, ProcessError};
use crate::core::game_launcher::validate_game_path;
use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::models::session::Region;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

const GAME_EXE: &str = "MapleStory.exe";

/// Resolve the configured executable, portable copy, or registered install.
/// The result is only used for this operation.
pub fn resolve_game_path(configured_path: &str) -> Result<String, AppError> {
    let launcher_exe = std::env::current_exe().ok();
    resolve_game_path_from(configured_path, launcher_exe.as_deref(), || {
        detect_registered_game_path(launcher_exe.as_deref())
    })
}

fn resolve_game_path_from(
    configured_path: &str,
    launcher_exe: Option<&Path>,
    detect_registered: impl FnOnce() -> Option<String>,
) -> Result<String, AppError> {
    let path = configured_path.trim();
    if path.is_empty() {
        return launcher_exe
            .filter(|exe| {
                !exe.file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case(GAME_EXE))
            })
            .map(|exe| exe.with_file_name(GAME_EXE))
            .filter(|exe| exe.is_file())
            .map(|exe| exe.to_string_lossy().into_owned())
            .or_else(detect_registered)
            .ok_or_else(|| ProcessError::GamePathNotFound.into());
    }

    validate_game_path(path)?;
    let metadata = std::fs::metadata(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => FsError::NotFound {
            path: path.to_owned(),
        },
        std::io::ErrorKind::PermissionDenied => FsError::PermissionDenied {
            path: path.to_owned(),
        },
        _ => FsError::Io {
            path: path.to_owned(),
            reason: error.to_string(),
        },
    })?;
    if !metadata.is_file() {
        return Err(FsError::NotFound {
            path: path.to_owned(),
        }
        .into());
    }
    Ok(path.to_owned())
}

/// Installers can record either the executable or its parent directory.
#[cfg(any(target_os = "windows", test))]
fn registered_game_exe(value: &str, exe_name: &str, launcher_exe: Option<&Path>) -> Option<String> {
    let path = Path::new(value.trim());
    if !path.is_absolute() {
        return None;
    }
    let executable = if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        path.to_path_buf()
    } else {
        path.join(exe_name)
    };

    // The web-launch interception may put the launcher in the game's registry key.
    if launcher_exe
        .and_then(Path::file_name)
        .zip(executable.file_name())
        .is_some_and(|(ours, candidate)| ours.eq_ignore_ascii_case(candidate))
    {
        return None;
    }
    let path = executable.to_str()?;
    validate_game_path(path).ok()?;
    executable.is_file().then(|| path.to_owned())
}

#[cfg(target_os = "windows")]
fn detect_registered_game_path(launcher_exe: Option<&Path>) -> Option<String> {
    use super::web_launch::{BACKUP_VALUE, GAMANIA_SUBKEY, PATH_VALUE};

    let candidates = [
        (GAMANIA_SUBKEY, PATH_VALUE),
        (GAMANIA_SUBKEY, BACKUP_VALUE),
        (r"SOFTWARE\Wizet\MapleStory", "ExecPath"),
        (r"SOFTWARE\Gamania\MapleStory HK", "Path"),
    ];
    read_registered_game_paths(&candidates, GAME_EXE, launcher_exe)
}

#[cfg(target_os = "windows")]
fn read_registered_game_paths(
    candidates: &[(&str, &str)],
    exe_name: &str,
    launcher_exe: Option<&Path>,
) -> Option<String> {
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };
    use winreg::RegKey;

    let user = RegKey::predef(HKEY_CURRENT_USER);
    let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
    for (hive, flags) in [
        (&user, KEY_READ),
        (&machine, KEY_READ | KEY_WOW64_64KEY),
        (&machine, KEY_READ | KEY_WOW64_32KEY),
    ] {
        for &(key, name) in candidates {
            let value = hive
                .open_subkey_with_flags(key, flags)
                .and_then(|key| key.get_value::<String, _>(name));
            if let Some(path) = value
                .ok()
                .and_then(|value| registered_game_exe(&value, exe_name, launcher_exe))
            {
                tracing::debug!(%path, "detected game path in Windows registry");
                return Some(path);
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn detect_registered_game_path(_launcher_exe: Option<&Path>) -> Option<String> {
    None
}

/// Detect an installed game on request, including registry locations supplied
/// by beanfun's service INI.
pub async fn detect_game_path(state: &AppState) -> Result<Option<String>, ErrorDto> {
    #[cfg(target_os = "windows")]
    {
        let launcher_exe = std::env::current_exe().ok();
        if let Some(path) = detect_registered_game_path(launcher_exe.as_deref()) {
            return Ok(Some(path));
        }

        let host = match state.config.read().await.region {
            Region::HK => "bfweb.hk",
            Region::TW => "tw",
        };
        let ini_url = format!(
            "https://{host}.beanfun.com/beanfun_block/generic_handlers/get_service_ini.ashx"
        );

        if let Ok(response) = state
            .http_client
            .get(ini_url)
            .header("User-Agent", WEBVIEW_USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            if let Some(body) =
                crate::services::http_util::read_capped_text(response, 4 * 1024 * 1024).await
            {
                let game_code = "610074_T9";
                let subkey = extract_ini_value(&body, game_code, "dir_reg");
                let value_name = extract_ini_value(&body, game_code, "dir_value_name");
                let exe_field = extract_ini_value(&body, game_code, "exe");
                let exe_name = exe_field
                    .as_deref()
                    .and_then(|field| field.split_whitespace().next())
                    .filter(|name| name.to_ascii_lowercase().ends_with(".exe"))
                    .unwrap_or(GAME_EXE);

                if let (Some(subkey), Some(value_name)) = (subkey, value_name) {
                    let subkey = subkey
                        .strip_prefix("HKEY_LOCAL_MACHINE\\")
                        .unwrap_or(&subkey);
                    return Ok(read_registered_game_paths(
                        &[(subkey, value_name.as_str())],
                        exe_name,
                        launcher_exe.as_deref(),
                    ));
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    let _ = state;

    Ok(None)
}

/// Extract a value from a simple INI-style string for a given section and key.
fn extract_ini_value(ini: &str, section: &str, key: &str) -> Option<String> {
    let section_header = format!("[{section}]");
    let mut in_section = false;

    for line in ini.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == section_header;
            continue;
        }
        if in_section {
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Clean the game's cache directories, failed-update folders (`*.$$$`), crash
/// dumps (`*.dmp`) and stale locale DLLs. Returns a short summary string.
///
/// Matches the reference Beanfun `btn_Recycling_Click` logic.
pub async fn cleanup_game_cache(state: &AppState) -> Result<String, ErrorDto> {
    let game_path = state.config.read().await.game_path.clone();

    if game_path.is_empty() {
        return Err(ErrorDto {
            code: "SYS_NO_GAME_PATH".to_string(),
            message: "Game path is not configured".into(),
            category: ErrorCategory::Configuration,
            details: None,
        });
    }

    let game_dir = std::path::Path::new(&game_path)
        .parent()
        .ok_or_else(|| ErrorDto {
            code: "SYS_INVALID_PATH".to_string(),
            message: "Cannot determine game directory".into(),
            category: ErrorCategory::Configuration,
            details: Some(game_path.clone()),
        })?
        .to_path_buf();

    if !game_dir.exists() {
        return Err(ErrorDto {
            code: "SYS_DIR_NOT_FOUND".to_string(),
            message: format!("Game directory not found: {}", game_dir.display()),
            category: ErrorCategory::FileSystem,
            details: Some(game_dir.display().to_string()),
        });
    }

    let mut cleaned = Vec::new();

    // 1. Remove known cache directories
    let cache_dirs = ["blob_storage", "GPUCache", "VideoDecodeStats", "XignCode"];
    for dir_name in &cache_dirs {
        let dir = game_dir.join(dir_name);
        if dir.exists() && std::fs::remove_dir_all(&dir).is_ok() {
            cleaned.push(format!("dir: {dir_name}"));
        }
    }

    // 2. Remove failed update cache (directories ending with .$$$)
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".$$$") && std::fs::remove_dir_all(&path).is_ok() {
                        cleaned.push(format!("dir: {name}"));
                    }
                }
            }
        }
    }

    // 3. Remove crash dumps (.dmp) and stale DLLs
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let lower = name.to_lowercase();
                    if (lower.ends_with(".dmp")
                        || lower == "localeemulator.dll"
                        || lower == "loaderdll.dll")
                        && std::fs::remove_file(&path).is_ok()
                    {
                        cleaned.push(format!("file: {name}"));
                    }
                }
            }
        }
    }

    let summary = if cleaned.is_empty() {
        "nothing to clean".to_string()
    } else {
        format!("cleaned {} items", cleaned.len())
    };

    tracing::info!("game cache cleanup: {summary} ({:?})", cleaned);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolves_configured_portable_and_registry_paths_in_order() {
        let folder =
            std::env::temp_dir().join(format!("maplelink-portable-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&folder).unwrap();
        let launcher = folder.join("MapleLink.exe");
        let adjacent = folder.join(GAME_EXE);
        let configured = folder.join("Configured.exe");
        let registered_dir = folder.join("Registered Games");
        fs::create_dir(&registered_dir).unwrap();
        let registered = registered_dir.join(GAME_EXE);
        let detect_registered =
            || registered_game_exe(registered_dir.to_str().unwrap(), GAME_EXE, Some(&launcher));

        fs::create_dir(&adjacent).unwrap();
        let missing = resolve_game_path_from("", Some(&launcher), detect_registered).unwrap_err();
        assert_eq!(
            ErrorDto::from(missing).code,
            "PROC_GAME_PATH_NOT_FOUND",
            "a folder with the expected name must still prompt for the executable"
        );
        fs::remove_dir(&adjacent).unwrap();

        fs::write(&registered, b"registered copy").unwrap();
        assert_eq!(
            resolve_game_path_from("", Some(&launcher), detect_registered).unwrap(),
            registered.to_str().unwrap()
        );
        assert_eq!(
            registered_game_exe(registered.to_str().unwrap(), GAME_EXE, Some(&launcher)).as_deref(),
            registered.to_str()
        );

        fs::write(&adjacent, b"portable copy").unwrap();
        let skip_registry = || panic!("the registry must not be checked when a path was selected");
        assert_eq!(
            resolve_game_path_from("", Some(&launcher), skip_registry).unwrap(),
            adjacent.to_str().unwrap()
        );
        assert!(matches!(
            resolve_game_path_from(configured.to_str().unwrap(), Some(&launcher), skip_registry),
            Err(AppError::FileSystem(FsError::NotFound { .. }))
        ));

        fs::write(&configured, b"configured copy").unwrap();
        assert_eq!(
            resolve_game_path_from(configured.to_str().unwrap(), Some(&launcher), skip_registry)
                .unwrap(),
            configured.to_str().unwrap()
        );

        fs::remove_file(configured).unwrap();
        fs::remove_file(adjacent).unwrap();
        fs::remove_file(registered).unwrap();
        fs::remove_dir(registered_dir).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn registry_detection_skips_the_launcher_and_web_launch_helper() {
        let folder =
            std::env::temp_dir().join(format!("maplelink-intercept-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&folder).unwrap();
        let launcher = folder.join("Beanfun.exe");
        let helper = folder.join("maplelink_web_launch.bat");
        for file in [&launcher, &helper] {
            fs::write(file, b"fixture").unwrap();
            assert_eq!(
                registered_game_exe(file.to_str().unwrap(), GAME_EXE, Some(&launcher)),
                None
            );
            fs::remove_file(file).unwrap();
        }
        fs::remove_dir(folder).unwrap();
    }
}
