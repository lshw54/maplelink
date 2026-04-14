//! Tauri commands for the auto-update system.

use tauri::State;

use crate::core::error::AppError;
use crate::models::app_state::AppState;
use crate::models::error::ErrorDto;
use crate::models::update::UpdateInfo;
use crate::services::update_service;

/// Check GitHub Releases for an available update.
/// Respects the update_channel config (release vs pre-release).
#[tauri::command]
pub async fn check_update(
    manual: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Option<UpdateInfo>, ErrorDto> {
    let is_manual = manual.unwrap_or(false);

    let config = state.config.read().await;
    if !is_manual && !update_service::should_check_on_startup(config.auto_update) {
        return Ok(None);
    }

    let include_prerelease =
        config.update_channel == crate::models::config::UpdateChannel::PreRelease;
    drop(config);

    let version = update_service::current_version();
    update_service::check_for_update(&state.http_client, version, include_prerelease)
        .await
        .map_err(|e| ErrorDto::from(AppError::from(e)))
}

/// Download and stage the update installer. Returns the installer path.
#[tauri::command]
pub async fn apply_update(
    download_url: String,
    use_proxy: Option<bool>,
    state: State<'_, AppState>,
) -> Result<String, ErrorDto> {
    let url = update_service::get_download_url(&download_url, use_proxy.unwrap_or(false));

    let bytes = update_service::download_update(&state.http_client, &url)
        .await
        .map_err(|e| ErrorDto::from(AppError::from(e)))?;

    let staging_dir = state
        .config_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("updates");

    let path = update_service::apply_update(&bytes, &staging_dir)
        .await
        .map_err(|e| ErrorDto::from(AppError::from(e)))?;

    Ok(path.display().to_string())
}

/// Test if GitHub is directly reachable (for ghproxy detection).
#[tauri::command]
pub async fn test_github_access(state: State<'_, AppState>) -> Result<bool, ErrorDto> {
    Ok(update_service::test_github_connectivity(&state.http_client).await)
}
