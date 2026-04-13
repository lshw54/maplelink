//! Tauri commands for game account management.
//!
//! Thin wrappers: validate inputs, delegate to service, map errors to [`ErrorDto`].

use tauri::State;

use crate::core::auth;
use crate::core::error::{AppError, AuthError};
use crate::models::app_state::AppState;
use crate::models::error::ErrorDto;
use crate::models::game_account::{GameAccount, GameCredentials};
use crate::models::session::Region;
use crate::services::{autopaste_service, beanfun_service};

/// Default service code for MapleStory.
const DEFAULT_SERVICE_CODE: &str = "610074";

/// Default service region for MapleStory HK.
const DEFAULT_SERVICE_REGION: &str = "T9";

// ---------------------------------------------------------------------------
// DTOs returned to the frontend
// ---------------------------------------------------------------------------

/// Frontend-safe representation of a [`GameAccount`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAccountDto {
    pub id: String,
    pub display_name: String,
    pub game_type: String,
    pub sn: String,
    pub status: String,
    pub created_at: String,
}

impl From<&GameAccount> for GameAccountDto {
    fn from(a: &GameAccount) -> Self {
        Self {
            id: a.id.clone(),
            display_name: a.display_name.clone(),
            game_type: a.game_type.clone(),
            sn: a.sn.clone(),
            status: a.status.clone(),
            created_at: a.created_at.clone(),
        }
    }
}

/// Frontend-safe representation of [`GameCredentials`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCredentialsDto {
    pub account_id: String,
    pub otp: String,
    pub retrieved_at: String,
}

impl From<&GameCredentials> for GameCredentialsDto {
    fn from(c: &GameCredentials) -> Self {
        Self {
            account_id: c.account_id.clone(),
            otp: c.otp.clone(),
            retrieved_at: c.retrieved_at.to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Return the cached list of game accounts (Req 2.1).
///
/// Requires an active session. Returns the accounts stored in
/// [`AppState::game_accounts`] — populated at login or by
/// [`refresh_accounts`].
#[tauri::command]
pub async fn get_game_accounts(
    state: State<'_, AppState>,
) -> Result<Vec<GameAccountDto>, ErrorDto> {
    // Ensure the user is authenticated
    {
        let session_guard = state.session.read().await;
        auth::require_valid_session(&session_guard).map_err(to_dto)?;
    }

    let accounts = state.game_accounts.read().await;
    let dtos = accounts.iter().map(GameAccountDto::from).collect();
    Ok(dtos)
}

/// Retrieve one-time game credentials for a specific account (Req 2.2, 2.3).
///
/// Fetches fresh credentials from the Beanfun platform. On failure the
/// error message is specific enough for the user to understand what went
/// wrong and know they can retry.
#[tauri::command]
pub async fn get_game_credentials(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<GameCredentialsDto, ErrorDto> {
    auth::validate_input("account_id", &account_id).map_err(to_dto)?;

    // Acquire bf_client_lock to prevent concurrent beanfun HTTP operations
    let _bf_lock = state.bf_client_lock.lock().await;

    let session_guard = state.session.read().await;
    let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;

    let creds = beanfun_service::get_game_credentials(
        &state.http_client,
        session,
        &account_id,
        &state.cookie_jar,
    )
    .await
    .map_err(|e| {
        tracing::warn!(
            account_id = %account_id,
            "credential retrieval failed: {e}"
        );
        login_err_to_dto(e)
    })?;

    tracing::info!(account_id = %account_id, "credentials retrieved");
    Ok(GameCredentialsDto::from(&creds))
}

/// Re-fetch the game account list from the Beanfun platform (Req 2.4).
///
/// Replaces the cached list in [`AppState::game_accounts`].
#[tauri::command]
pub async fn refresh_accounts(state: State<'_, AppState>) -> Result<Vec<GameAccountDto>, ErrorDto> {
    let _bf_lock = state.bf_client_lock.lock().await;

    let session_guard = state.session.read().await;
    let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;

    let accounts =
        beanfun_service::get_game_accounts(&state.http_client, session, &state.cookie_jar)
            .await
            .map_err(login_err_to_dto)?;

    let dtos: Vec<GameAccountDto> = accounts.iter().map(GameAccountDto::from).collect();

    drop(session_guard);
    *state.game_accounts.write().await = accounts;

    tracing::info!("game accounts refreshed ({} accounts)", dtos.len());
    Ok(dtos)
}
/// Retrieve the user's remaining Beanfun points (Req 2.5).
///
/// Requires an active session. Delegates to
/// [`beanfun_service::get_remain_point`].
#[tauri::command]
pub async fn ping_session(state: State<'_, AppState>) -> Result<bool, ErrorDto> {
    let region = {
        let session_guard = state.session.read().await;
        match session_guard.as_ref() {
            Some(s) => s.region.clone(),
            None => return Ok(false),
        }
    };

    // Non-blocking: skip this ping if another operation is in progress.
    // This prevents concurrent beanfun HTTP requests from corrupting the session.
    if let Ok(_guard) = state.bf_client_lock.try_lock() {
        beanfun_service::ping(&state.http_client, &region).await;
    } else {
        tracing::debug!("ping skipped — bf_client_lock is held by another operation");
    }
    Ok(true)
}

#[tauri::command]
pub async fn get_remain_point(state: State<'_, AppState>) -> Result<i32, ErrorDto> {
    let _bf_lock = state.bf_client_lock.lock().await;

    let region = {
        let session_guard = state.session.read().await;
        let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;
        session.region.clone()
    };

    let points = beanfun_service::get_remain_point(&state.http_client, &region)
        .await
        .map_err(login_err_to_dto)?;

    tracing::info!("remain points: {points}");
    Ok(points)
}

/// Retrieve OTP and auto-paste credentials into the MapleStory game window.
///
/// 1. Fetches fresh OTP credentials for the given account.
/// 2. Attempts to find the MapleStory window and auto-input the credentials.
/// 3. Returns `true` if auto-paste succeeded, `false` if the window was not
///    found (the OTP is copied to the clipboard by the frontend in that case).
#[tauri::command]
pub async fn auto_paste_otp(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<bool, ErrorDto> {
    auth::validate_input("account_id", &account_id).map_err(to_dto)?;

    // Acquire bf_client_lock for the HTTP credential retrieval part
    let _bf_lock = state.bf_client_lock.lock().await;

    let session_guard = state.session.read().await;
    let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;

    let is_hk = session.region == Region::HK;

    let creds = beanfun_service::get_game_credentials(
        &state.http_client,
        session,
        &account_id,
        &state.cookie_jar,
    )
    .await
    .map_err(|e| {
        tracing::warn!(account_id = %account_id, "credential retrieval for auto-paste failed: {e}");
        login_err_to_dto(e)
    })?;

    // Drop locks before the blocking auto-paste call
    drop(session_guard);
    drop(_bf_lock);

    let otp = creds.otp.clone();
    let aid = creds.account_id.clone();
    let pasted = tokio::task::spawn_blocking(move || {
        autopaste_service::auto_paste_credentials(&aid, &otp, is_hk)
    })
    .await
    .unwrap_or(false);

    if pasted {
        tracing::info!(account_id = %account_id, "auto-paste succeeded");
    } else {
        tracing::info!(account_id = %account_id, "auto-paste skipped (window not found)");
    }

    Ok(pasted)
}
/// Change the display name of a game account (context menu action).
///
/// Delegates to [`beanfun_service::change_display_name`] which POSTs to
/// `gamezone.ashx` with `ChangeServiceAccountDisplayName`.
#[tauri::command]
pub async fn change_account_display_name(
    account_id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<bool, ErrorDto> {
    auth::validate_input("account_id", &account_id).map_err(to_dto)?;
    auth::validate_input("new_name", &new_name).map_err(to_dto)?;

    let _bf_lock = state.bf_client_lock.lock().await;

    let session_guard = state.session.read().await;
    let _session = auth::require_valid_session(&session_guard).map_err(to_dto)?;

    let game_code = format!("{}_{}", DEFAULT_SERVICE_CODE, DEFAULT_SERVICE_REGION);

    let success = beanfun_service::change_display_name(
        &state.http_client,
        &game_code,
        &account_id,
        &new_name,
    )
    .await
    .map_err(login_err_to_dto)?;

    if success {
        tracing::info!(account_id = %account_id, new_name = %new_name, "display name changed");
    } else {
        tracing::warn!(account_id = %account_id, "display name change failed (server returned failure)");
    }

    Ok(success)
}

/// Retrieve the authenticated user's email address (context menu action).
///
/// Delegates to [`beanfun_service::get_email`]. Returns an empty string
/// for HK region (not supported by the platform).
#[tauri::command]
pub async fn get_auth_email(state: State<'_, AppState>) -> Result<String, ErrorDto> {
    let _bf_lock = state.bf_client_lock.lock().await;

    let session_guard = state.session.read().await;
    let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;

    let email = beanfun_service::get_email(&state.http_client, &session.region)
        .await
        .map_err(login_err_to_dto)?;

    tracing::info!("auth email retrieved (empty={})", email.is_empty());
    Ok(email)
}

// ---------------------------------------------------------------------------
// Error mapping helpers
// ---------------------------------------------------------------------------

/// Convert an [`AuthError`] into an [`ErrorDto`].
fn to_dto(err: AuthError) -> ErrorDto {
    let app_err: AppError = err.into();
    ErrorDto::from(app_err)
}

/// Convert a [`beanfun_service::LoginError`] into an [`ErrorDto`].
fn login_err_to_dto(err: beanfun_service::LoginError) -> ErrorDto {
    match err {
        beanfun_service::LoginError::Auth(e) => to_dto(e),
        beanfun_service::LoginError::Network(e) => {
            let app_err: AppError = e.into();
            ErrorDto::from(app_err)
        }
    }
}
