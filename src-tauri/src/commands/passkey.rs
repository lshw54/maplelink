//! Tauri commands for the GamaPass passkey source (toolbox → login).
//!
//! The choice lives in the prefs store; the extension files live under the
//! app data dir. Loading into the WebView2 profile happens when the GamaPass
//! window opens ([`crate::services::webview_login`]).

use tauri::Manager;

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::services::passkey_manager::{self, InstallRecord, Manager as PmManager, PasskeySource};

fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, ErrorDto> {
    app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })
}

fn failed(code: &str, message: String) -> ErrorDto {
    ErrorDto {
        code: code.to_string(),
        message,
        category: ErrorCategory::Process,
        details: None,
    }
}

fn parse_manager(key: &str) -> Result<PmManager, ErrorDto> {
    PmManager::from_key(key).ok_or_else(|| {
        failed(
            "PASSKEY_UNKNOWN_MANAGER",
            format!("unknown password manager: {key}"),
        )
    })
}

/// What the settings page shows for one manager.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerStatusDto {
    pub key: String,
    pub name: String,
    pub installed: Option<InstallRecord>,
}

/// Current source plus the install state of every supported manager.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeySettingsDto {
    pub source: String,
    pub managers: Vec<ManagerStatusDto>,
}

fn settings(app: &tauri::AppHandle) -> Result<PasskeySettingsDto, ErrorDto> {
    let data = app_data_dir(app)?;
    let source = PasskeySource::parse(
        crate::services::prefs::get(&data, passkey_manager::PREF_SOURCE).as_deref(),
    );
    Ok(PasskeySettingsDto {
        source: source.key().to_string(),
        managers: PmManager::ALL
            .iter()
            .map(|m| ManagerStatusDto {
                key: m.key().to_string(),
                name: m.display_name().to_string(),
                installed: passkey_manager::status(&data, *m),
            })
            .collect(),
    })
}

#[tauri::command]
pub async fn passkey_settings_get(app: tauri::AppHandle) -> Result<PasskeySettingsDto, ErrorDto> {
    settings(&app)
}

/// Choose where GamaPass passkeys are answered. `windows` or a manager key;
/// a manager must be installed first. Takes effect on the next GamaPass
/// window.
#[tauri::command]
pub async fn passkey_source_set(
    source: String,
    app: tauri::AppHandle,
) -> Result<PasskeySettingsDto, ErrorDto> {
    let data = app_data_dir(&app)?;
    let parsed = PasskeySource::parse(Some(&source));
    if source.trim() != "windows" && parsed == PasskeySource::Windows {
        return Err(failed(
            "PASSKEY_UNKNOWN_MANAGER",
            format!("unknown passkey source: {source}"),
        ));
    }
    if let Some(m) = parsed.manager() {
        if passkey_manager::status(&data, m).is_none() {
            return Err(failed(
                "PASSKEY_MANAGER_NOT_INSTALLED",
                format!("{} is not installed", m.display_name()),
            ));
        }
    }
    crate::services::prefs::set(&data, passkey_manager::PREF_SOURCE, parsed.key())
        .map_err(|e| failed("PASSKEY_PREF_WRITE", e))?;
    tracing::info!("passkey source set to {}", parsed.key());
    settings(&app)
}

/// Download, verify and prepare a manager's extension.
#[tauri::command]
pub async fn passkey_manager_install(
    manager: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<PasskeySettingsDto, ErrorDto> {
    let m = parse_manager(&manager)?;
    let data = app_data_dir(&app)?;
    passkey_manager::install(&state.http_client, &data, m)
        .await
        .map_err(|e| {
            tracing::warn!("passkey manager: install {} failed: {e}", m.display_name());
            failed("PASSKEY_INSTALL_FAILED", e)
        })?;
    settings(&app)
}

/// Delete a manager's files. Falls back to `windows` if it was selected.
#[tauri::command]
pub async fn passkey_manager_remove(
    manager: String,
    app: tauri::AppHandle,
) -> Result<PasskeySettingsDto, ErrorDto> {
    let m = parse_manager(&manager)?;
    let data = app_data_dir(&app)?;
    let current = PasskeySource::parse(
        crate::services::prefs::get(&data, passkey_manager::PREF_SOURCE).as_deref(),
    );
    if current == PasskeySource::Manager(m) {
        crate::services::prefs::set(&data, passkey_manager::PREF_SOURCE, "windows")
            .map_err(|e| failed("PASSKEY_PREF_WRITE", e))?;
    }
    passkey_manager::uninstall(&data, m).map_err(|e| failed("PASSKEY_REMOVE_FAILED", e))?;
    crate::services::gamepass_extensions::close_all(&app);
    settings(&app)
}

/// Open the manager's own vault page in a MapleLink window on the GamaPass
/// profile, so the player can sign in or unlock before a login.
#[tauri::command]
pub async fn passkey_manager_open(manager: String, app: tauri::AppHandle) -> Result<(), ErrorDto> {
    let m = parse_manager(&manager)?;
    crate::services::webview_login::open_passkey_manager_window(app, m)
        .await
        .map_err(|e| failed("PASSKEY_OPEN_FAILED", e))
}
