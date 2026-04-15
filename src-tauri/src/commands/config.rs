//! Tauri commands for configuration management.
//!
//! Thin wrappers that read/write [`AppState::config`] and delegate
//! persistence to [`crate::services::config_service`].

use tauri::State;

use crate::core::error::{AppError, ConfigError};
use crate::models::app_state::AppState;
use crate::models::config::{AppConfig, FontSize, Language, Theme, UpdateChannel};
use crate::models::error::ErrorDto;
use crate::models::session::Region;
use crate::services::config_service;

/// Return the current in-memory configuration.
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, ErrorDto> {
    let config = state.config.read().await;
    Ok(config.clone())
}

/// Update a single configuration field by key and persist to disk.
///
/// Supported keys (flat, snake_case):
/// `game_path`, `locale`, `theme`, `language`,
/// `auto_update`, `skip_play_confirm`, `auto_start`, `region`,
/// `debug_logging`, `window_x`, `window_y`, `window_width`, `window_height`.
#[tauri::command]
pub async fn set_config(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    let mut config = state.config.write().await;

    apply_config_field(&mut config, &key, &value).map_err(|e| {
        let app_err: AppError = e.into();
        ErrorDto::from(app_err)
    })?;

    config_service::save_config(&state.config_path, &config)
        .await
        .map_err(|e| {
            let app_err: AppError = e.into();
            ErrorDto::from(app_err)
        })?;

    tracing::info!("config updated: {key} = {value}");
    Ok(())
}

/// Reset configuration to defaults and persist to disk.
#[tauri::command]
pub async fn reset_config(state: State<'_, AppState>) -> Result<(), ErrorDto> {
    let mut config = state.config.write().await;
    *config = AppConfig::default();

    config_service::save_config(&state.config_path, &config)
        .await
        .map_err(|e| {
            let app_err: AppError = e.into();
            ErrorDto::from(app_err)
        })?;

    tracing::info!("config reset to defaults");
    Ok(())
}

// ---------------------------------------------------------------------------
// Field mapping helper
// ---------------------------------------------------------------------------

/// Map a flat key + string value onto the corresponding [`AppConfig`] field.
fn apply_config_field(config: &mut AppConfig, key: &str, value: &str) -> Result<(), ConfigError> {
    match key {
        "game_path" => config.game_path = value.to_string(),
        "locale" => config.locale = value.to_string(),
        "theme" => {
            config.theme = match value.to_lowercase().as_str() {
                "system" => Theme::System,
                "dark" => Theme::Dark,
                "light" => Theme::Light,
                _ => {
                    return Err(ConfigError::ParseError {
                        reason: format!("unknown theme value: {value}"),
                    });
                }
            };
        }
        "language" => {
            config.language = match value.to_lowercase().replace('-', "").as_str() {
                "enus" | "en_us" => Language::EnUS,
                "zhtw" | "zh_tw" => Language::ZhTW,
                "zhcn" | "zh_cn" => Language::ZhCN,
                _ => {
                    return Err(ConfigError::ParseError {
                        reason: format!("unknown language value: {value}"),
                    });
                }
            };
        }
        "auto_update" => {
            config.auto_update = parse_bool(value)?;
        }
        "skip_play_confirm" => {
            config.skip_play_confirm = parse_bool(value)?;
        }
        "auto_start" => {
            config.auto_start = parse_bool(value)?;
        }
        "region" => {
            config.region = match value.to_uppercase().as_str() {
                "TW" => Region::TW,
                "HK" => Region::HK,
                _ => {
                    return Err(ConfigError::ParseError {
                        reason: format!("unknown region value: {value}"),
                    });
                }
            };
        }
        "debug_logging" => {
            config.debug_logging = parse_bool(value)?;
        }
        "gamepass_incognito" => {
            config.gamepass_incognito = parse_bool(value)?;
        }
        "traditional_login" => {
            config.traditional_login = parse_bool(value)?;
        }
        "auto_kill_patcher" => {
            config.auto_kill_patcher = parse_bool(value)?;
        }
        "update_channel" => {
            config.update_channel = match value.to_lowercase().replace('_', "-").as_str() {
                "release" => UpdateChannel::Release,
                "pre-release" | "prerelease" => UpdateChannel::PreRelease,
                _ => {
                    return Err(ConfigError::ParseError {
                        reason: format!("unknown update_channel value: {value}"),
                    });
                }
            };
        }
        "font_size" => {
            config.font_size = match value.to_lowercase().as_str() {
                "small" => FontSize::Small,
                "medium" => FontSize::Medium,
                "large" => FontSize::Large,
                "extra-large" => FontSize::ExtraLarge,
                _ => {
                    return Err(ConfigError::ParseError {
                        reason: format!("unknown font_size value: {value}"),
                    });
                }
            };
        }
        "window_x" => {
            config.window_x = parse_optional_i32(value)?;
        }
        "window_y" => {
            config.window_y = parse_optional_i32(value)?;
        }
        "window_width" => {
            config.window_width = parse_optional_u32(value)?;
        }
        "window_height" => {
            config.window_height = parse_optional_u32(value)?;
        }
        "__reset__" => {
            *config = AppConfig::default();
        }
        _ => {
            return Err(ConfigError::ParseError {
                reason: format!("unknown config key: {key}"),
            });
        }
    }
    Ok(())
}

fn parse_bool(value: &str) -> Result<bool, ConfigError> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(ConfigError::ParseError {
            reason: format!("expected boolean, got: {value}"),
        }),
    }
}

fn parse_optional_i32(value: &str) -> Result<Option<i32>, ConfigError> {
    if value.is_empty() || value == "null" || value == "none" {
        return Ok(None);
    }
    value
        .parse::<i32>()
        .map(Some)
        .map_err(|_| ConfigError::ParseError {
            reason: format!("expected integer, got: {value}"),
        })
}

fn parse_optional_u32(value: &str) -> Result<Option<u32>, ConfigError> {
    if value.is_empty() || value == "null" || value == "none" {
        return Ok(None);
    }
    value
        .parse::<u32>()
        .map(Some)
        .map_err(|_| ConfigError::ParseError {
            reason: format!("expected unsigned integer, got: {value}"),
        })
}
