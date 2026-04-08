//! Tauri commands for the auto-update system.
//!
//! Thin wrappers that delegate to [`crate::services::update_service`] and
//! respect the `auto_update` toggle in [`AppConfig`].

use tauri::State;

use crate::core::error::AppError;
use crate::models::app_state::AppState;
use crate::models::error::ErrorDto;
use crate::models::update::UpdateInfo;
use crate::services::update_service;

/// Check for an available update.
///
/// When `manual` is `false` (startup check), the command respects the
/// `auto_update` config toggle — returning `Ok(None)` immediately if
/// auto-update is disabled. When `manual` is `true`, the check always
/// proceeds regardless of the toggle.
#[tauri::command]
pub async fn check_update(
    manual: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Option<UpdateInfo>, ErrorDto> {
    let is_manual = manual.unwrap_or(false);

    if !is_manual {
        let config = state.config.read().await;
        if !update_service::should_check_on_startup(config.auto_update) {
            tracing::info!("auto-update disabled, skipping startup check");
            return Ok(None);
        }
    }

    let version = update_service::current_version();

    update_service::check_for_update(&state.http_client, version)
        .await
        .map_err(|e| {
            let app_err: AppError = e.into();
            ErrorDto::from(app_err)
        })
}

/// Download and apply an available update.
///
/// The caller should have previously called `check_update` to obtain the
/// [`UpdateInfo`]. This command downloads the binary, stages it, and
/// signals that a restart is needed.
#[tauri::command]
pub async fn apply_update(
    download_url: String,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    let bytes = update_service::download_update(&state.http_client, &download_url)
        .await
        .map_err(|e| {
            let app_err: AppError = e.into();
            ErrorDto::from(app_err)
        })?;

    // Stage the update in a temp directory next to the config.
    let staging_dir = state
        .config_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("updates");

    update_service::apply_update(&bytes, &staging_dir)
        .await
        .map_err(|e| {
            let app_err: AppError = e.into();
            ErrorDto::from(app_err)
        })?;

    Ok(())
}
