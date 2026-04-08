//! File I/O layer for configuration persistence.
//!
//! Delegates all parsing/serialization to [`crate::core::config_parser`] and
//! handles only the async file system operations via [`tokio::fs`].

use std::path::Path;

use crate::core::config_parser::{parse_ini, serialize_ini};
use crate::core::error::ConfigError;
use crate::models::config::AppConfig;

/// Load config from the given file path.
///
/// Returns [`AppConfig::default()`] if the file does not exist.
/// If the file exists but is malformed, [`parse_ini`] handles fallback to defaults.
pub async fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    if !path.exists() {
        tracing::info!(
            "config file not found at {}, using defaults",
            path.display()
        );
        return Ok(AppConfig::default());
    }

    let contents = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| ConfigError::ParseError {
            reason: format!("failed to read {}: {e}", path.display()),
        })?;

    parse_ini(&contents)
}

/// Save config to the given file path.
///
/// Creates parent directories if they do not exist.
pub async fn save_config(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ConfigError::WriteError {
                reason: format!("failed to create directories for {}: {e}", path.display()),
            })?;
    }

    let ini = serialize_ini(config);

    tokio::fs::write(path, ini)
        .await
        .map_err(|e| ConfigError::WriteError {
            reason: format!("failed to write {}: {e}", path.display()),
        })?;

    Ok(())
}

/// Ensure a default config file exists at the given path.
///
/// If the file already exists this is a no-op. Otherwise a default config is
/// written via [`save_config`].
pub async fn ensure_default_config(path: &Path) -> Result<(), ConfigError> {
    if path.exists() {
        return Ok(());
    }

    tracing::info!("creating default config at {}", path.display());
    save_config(path, &AppConfig::default()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Helper: create a unique temp dir for each test.
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("maplelink_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    #[tokio::test]
    async fn load_missing_file_returns_defaults() {
        let path = temp_dir().join("nonexistent.ini");
        let config = load_config(&path).await.unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[tokio::test]
    async fn save_then_load_round_trip() {
        let dir = temp_dir().join("round_trip");
        let path = dir.join("config.ini");

        let original = AppConfig {
            game_path: "C:\\Games\\Maple.exe".into(),
            theme: crate::models::config::Theme::Light,
            language: crate::models::config::Language::EnUS,
            auto_update: false,
            ..AppConfig::default()
        };

        save_config(&path, &original).await.unwrap();
        let loaded = load_config(&path).await.unwrap();
        assert_eq!(loaded, original);

        // cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn ensure_default_creates_file() {
        let dir = temp_dir().join("ensure_default");
        let path = dir.join("config.ini");

        // File should not exist yet.
        assert!(!path.exists());

        ensure_default_config(&path).await.unwrap();
        assert!(path.exists());

        // Loading it back should give defaults.
        let config = load_config(&path).await.unwrap();
        assert_eq!(config, AppConfig::default());

        // cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn ensure_default_is_noop_when_file_exists() {
        let dir = temp_dir().join("ensure_noop");
        let path = dir.join("config.ini");

        // Write a custom config first.
        let custom = AppConfig {
            game_path: "D:\\Custom\\Game.exe".into(),
            ..AppConfig::default()
        };
        save_config(&path, &custom).await.unwrap();

        // ensure_default_config should NOT overwrite.
        ensure_default_config(&path).await.unwrap();

        let loaded = load_config(&path).await.unwrap();
        assert_eq!(loaded.game_path, "D:\\Custom\\Game.exe");

        // cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn save_creates_parent_directories() {
        let dir = temp_dir().join("nested").join("deep").join("dir");
        let path = dir.join("config.ini");

        save_config(&path, &AppConfig::default()).await.unwrap();
        assert!(path.exists());

        // cleanup
        std::fs::remove_dir_all(temp_dir().join("nested")).ok();
    }
}
