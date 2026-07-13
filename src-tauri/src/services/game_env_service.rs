//! Game-environment side effects on the local machine: auto-detecting the
//! MapleStory install path (beanfun service INI + Windows Registry) and
//! cleaning the game's cache/crash-dump files.
//!
//! Extracted from `commands/system.rs` so registry/fs side effects live in
//! `services/` (Clean Architecture). Command handlers delegate here.

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::models::session::Region;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

/// Auto-detect the MapleStory game path.
///
/// Fetches beanfun's service INI to learn the registry location, reads the
/// path from HKCU/HKLM, and falls back to a few well-known registry keys.
pub async fn detect_game_path(state: &AppState) -> Result<Option<String>, ErrorDto> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        let region = state.config.read().await.region.clone();
        let host = match region {
            Region::HK => "bfweb.hk",
            Region::TW => "tw",
        };
        let ini_url = format!(
            "https://{host}.beanfun.com/beanfun_block/generic_handlers/get_service_ini.ashx"
        );

        if let Ok(ini_text) = state
            .http_client
            .get(&ini_url)
            .header("User-Agent", WEBVIEW_USER_AGENT)
            .send()
            .await
        {
            if let Ok(body) = ini_text.text().await {
                let game_code = "610074_T9";
                let dir_reg = extract_ini_value(&body, game_code, "dir_reg");
                let dir_value_name = extract_ini_value(&body, game_code, "dir_value_name");
                let exe_field = extract_ini_value(&body, game_code, "exe");

                let exe_name = exe_field
                    .as_deref()
                    .and_then(|e| {
                        let name = e.split_whitespace().next().unwrap_or("");
                        if name.to_lowercase().ends_with(".exe") {
                            Some(name.to_string())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "MapleStory.exe".to_string());

                if let (Some(reg_path), Some(val_name)) = (dir_reg, dir_value_name) {
                    let reg_path = reg_path.replace("HKEY_LOCAL_MACHINE\\", "");
                    tracing::info!("INI dir_reg={reg_path}, dir_value_name={val_name}");

                    if let Ok(key) = hkcu.open_subkey(&reg_path) {
                        if let Ok(dir) = key.get_value::<String, _>(&val_name) {
                            if !dir.is_empty() {
                                let full_str = if dir.to_lowercase().ends_with(".exe") {
                                    dir
                                } else {
                                    std::path::Path::new(&dir)
                                        .join(&exe_name)
                                        .to_string_lossy()
                                        .to_string()
                                };
                                tracing::info!("detected game path from HKCU: {full_str}");
                                return Ok(Some(full_str));
                            }
                        }
                    }

                    let hklm = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
                    if let Ok(key) = hklm.open_subkey(&reg_path) {
                        if let Ok(dir) = key.get_value::<String, _>(&val_name) {
                            if !dir.is_empty() {
                                let full_str = if dir.to_lowercase().ends_with(".exe") {
                                    dir
                                } else {
                                    std::path::Path::new(&dir)
                                        .join(&exe_name)
                                        .to_string_lossy()
                                        .to_string()
                                };
                                tracing::info!("detected game path from HKLM: {full_str}");
                                return Ok(Some(full_str));
                            }
                        }
                    }
                }
            }
        }

        let candidates: &[(&str, &str)] = &[
            (r"Software\Gamania\MapleStory", "Path"),
            (r"Software\Wizet\MapleStory", "ExecPath"),
            (r"Software\Gamania\MapleStory HK", "Path"),
        ];

        for &(subkey, value_name) in candidates {
            if let Ok(key) = hkcu.open_subkey(subkey) {
                if let Ok(path) = key.get_value::<String, _>(value_name) {
                    if !path.is_empty() {
                        tracing::info!(
                            "detected game path from fallback registry {subkey}\\{value_name}: {path}"
                        );
                        return Ok(Some(path));
                    }
                }
            }
        }

        tracing::debug!("no game path found in registry");
        Ok(None)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        tracing::debug!("game path detection is only supported on Windows");
        Ok(None)
    }
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
