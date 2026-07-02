//! Tauri commands for Beanfun authentication.
//!
//! Thin wrappers: validate inputs, delegate to core/service, map errors to [`ErrorDto`].

use base64::Engine;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::Manager;
use tauri::State;

use crate::core::auth::{self, SessionAction};
use crate::core::error::{AppError, AuthError};
use crate::models::app_state::{AppState, SessionInfo};
use crate::models::error::ErrorDto;
use crate::models::session::Session;
use crate::services::beanfun_service::{self, QrCodeData, QrPollResult};

type SessionKeyFallbackSender = tokio::sync::oneshot::Sender<WebviewFetchResult>;

static SESSION_KEY_WEBVIEW_RESULTS: OnceLock<Mutex<HashMap<String, SessionKeyFallbackSender>>> =
    OnceLock::new();

#[derive(Debug, Clone)]
struct WebviewFetchResult {
    url: String,
    html: String,
}

// ---------------------------------------------------------------------------
// DTOs returned to the frontend
// ---------------------------------------------------------------------------

/// Subset of [`Session`] safe to send to the frontend (no refresh token).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub session_id: String,
    pub token: String,
    pub region: String,
    pub account_name: String,
    pub expires_at: String,
}

impl SessionDto {
    fn from_session(s: &Session, session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            token: s.token.clone(),
            region: format!("{:?}", s.region),
            account_name: s.account_name.clone(),
            expires_at: s.expires_at.to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Session management commands
// ---------------------------------------------------------------------------

/// Create a new empty session and return its ID.
#[tauri::command]
pub async fn create_session(state: State<'_, AppState>) -> Result<String, ErrorDto> {
    let (id, _) = state.create_session().await;
    tracing::info!("created new session: {id}");
    Ok(id)
}

/// List all active sessions with their basic info.
#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionInfo>, ErrorDto> {
    Ok(state.list_sessions().await)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Normal username + password login.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn login(
    session_id: String,
    account: String,
    password: String,
    recaptcha_check: Option<String>,
    recaptcha_login: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SessionDto, ErrorDto> {
    // Input validation
    auth::validate_input("account", &account).map_err(to_dto)?;
    auth::validate_input("password", &password).map_err(to_dto)?;

    let ss = state.require_session(&session_id).await?;
    let region = state.config.read().await.region.clone();

    let tokens = beanfun_service::RecaptchaTokens {
        check: recaptcha_check,
        login: recaptcha_login,
    };

    let login_result = login_with_native_fallback(
        &app,
        &ss.http_client,
        &account,
        &password,
        &region,
        &ss.cookie_jar,
        &tokens,
    )
    .await;

    // Handle TOTP required: store partial session and return specific error
    let session = match login_result {
        Ok(s) => s,
        Err(beanfun_service::LoginError::Auth(AuthError::TotpRequired { partial_session })) => {
            *ss.session.write().await = Some(*partial_session);
            tracing::info!("login requires TOTP verification");
            return Err(to_dto(AuthError::TotpRequired {
                partial_session: Box::new(
                    ss.session
                        .read()
                        .await
                        .clone()
                        .unwrap_or_else(|| panic!("partial session was just stored")),
                ),
            }));
        }
        Err(beanfun_service::LoginError::Auth(AuthError::AdvanceCheckRequired { url })) => {
            tracing::info!("login requires advance check");
            return Err(ErrorDto {
                code: "AUTH_ADVANCE_CHECK".to_string(),
                message: url.unwrap_or_default(),
                category: crate::models::error::ErrorCategory::Authentication,
                details: Some("advance_check_required".to_string()),
            });
        }
        Err(e) => return Err(login_err_to_dto(e)),
    };

    let dto = SessionDto::from_session(&session, &session_id);

    // Fetch game accounts right after login (Req 1.6)
    let accounts = beanfun_service::get_game_accounts(&ss.http_client, &session, &ss.cookie_jar)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to fetch game accounts after login: {e}");
            Vec::new()
        });

    *ss.session.write().await = Some(session);
    *ss.game_accounts.write().await = accounts;

    tracing::info!("user logged in: {}", dto.account_name);
    Ok(dto)
}

/// Phase 1 of two-phase TW Regular login: validate the account and pass the
/// first reCAPTCHA (`recaptcha_check`), then stash the session key + form token
/// on the session for [`tw_login_submit`].
///
/// Frontend flow: collect account → `open_recaptcha_window({ step: "check" })`
/// → await `recaptcha-token` → call this → reveal password field.
#[tauri::command]
pub async fn tw_login_check(
    session_id: String,
    account: String,
    recaptcha_check: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    auth::validate_input("account", &account).map_err(to_dto)?;

    let ss = state.require_session(&session_id).await?;
    let region = state.config.read().await.region.clone();
    if region != crate::models::session::Region::TW {
        return Err(ErrorDto {
            code: "AUTH_FLOW_UNSUPPORTED".to_string(),
            message: "two-phase reCAPTCHA login is only for TW region".to_string(),
            category: crate::models::error::ErrorCategory::Authentication,
            details: None,
        });
    }

    let (skey, form_token) =
        beanfun_service::tw_login_check(&ss.http_client, &account, recaptcha_check.as_deref())
            .await
            .map_err(login_err_to_dto)?;

    *ss.pending_tw_login.write().await = Some(crate::models::session_state::PendingTwLogin {
        skey,
        form_token,
        account,
    });

    tracing::info!("TW login phase 1 (CheckAccountType) passed");
    Ok(())
}

/// Phase 2 of two-phase TW Regular login: submit the password with the second
/// reCAPTCHA (`recaptcha_login`), reusing the state stashed by
/// [`tw_login_check`]. Returns the session and fetches game accounts on success.
#[tauri::command]
pub async fn tw_login_submit(
    session_id: String,
    password: String,
    recaptcha_login: Option<String>,
    state: State<'_, AppState>,
) -> Result<SessionDto, ErrorDto> {
    auth::validate_input("password", &password).map_err(to_dto)?;

    let ss = state.require_session(&session_id).await?;

    let pending = ss.pending_tw_login.read().await.clone().ok_or(ErrorDto {
        code: "AUTH_NO_PENDING_LOGIN".to_string(),
        message: "no pending TW login — call tw_login_check first".to_string(),
        category: crate::models::error::ErrorCategory::Authentication,
        details: None,
    })?;

    let login_result = beanfun_service::tw_login_submit(
        &ss.http_client,
        &pending.skey,
        &pending.form_token,
        &pending.account,
        &password,
        recaptcha_login.as_deref(),
        &ss.cookie_jar,
    )
    .await;

    let session = match login_result {
        Ok(s) => s,
        Err(beanfun_service::LoginError::Auth(AuthError::AdvanceCheckRequired { url })) => {
            tracing::info!("TW login phase 2 requires advance check");
            return Err(ErrorDto {
                code: "AUTH_ADVANCE_CHECK".to_string(),
                message: url.unwrap_or_default(),
                category: crate::models::error::ErrorCategory::Authentication,
                details: Some("advance_check_required".to_string()),
            });
        }
        Err(e) => return Err(login_err_to_dto(e)),
    };

    // Consume the pending state now that the login attempt resolved.
    *ss.pending_tw_login.write().await = None;

    let dto = SessionDto::from_session(&session, &session_id);

    // Fetch the game accounts, retrying a few times while empty. The bfWebToken
    // set by SendLogin can need a moment to settle before
    // game_server_account_list.aspx returns the list — without retries the UI
    // lands on an empty account list.
    let mut accounts = Vec::new();
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        }
        match beanfun_service::get_game_accounts(&ss.http_client, &session, &ss.cookie_jar).await {
            Ok(a) if !a.is_empty() => {
                accounts = a;
                break;
            }
            Ok(a) => accounts = a, // empty — retry
            Err(e) => tracing::warn!(
                "fetch game accounts after TW login (attempt {}): {e}",
                attempt + 1
            ),
        }
    }
    tracing::info!("TW two-phase login: {} game accounts", accounts.len());

    *ss.session.write().await = Some(session);
    *ss.game_accounts.write().await = accounts;

    tracing::info!("user logged in (two-phase TW): {}", dto.account_name);
    Ok(dto)
}

/// Start a QR-code login flow (TW region).
#[tauri::command]
pub async fn qr_login_start(
    session_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<QrCodeData, ErrorDto> {
    let ss = state.require_session(&session_id).await?;
    let region = state.config.read().await.region.clone();

    qr_login_start_with_native_fallback(&app, &ss.http_client, &region)
        .await
        .map_err(login_err_to_dto)
}

/// Poll the status of an in-progress QR-code login.
#[tauri::command]
pub async fn qr_login_poll(
    session_id: String,
    session_key: String,
    verification_token: String,
    state: State<'_, AppState>,
) -> Result<QrPollResult, ErrorDto> {
    auth::validate_input("session_key", &session_key).map_err(to_dto)?;

    let ss = state.require_session(&session_id).await?;
    let region = state.config.read().await.region.clone();

    let result =
        beanfun_service::qr_login_poll(&ss.http_client, &session_key, &verification_token, &region)
            .await
            .map_err(login_err_to_dto)?;

    // If confirmed, complete the login and store the session
    if result.status == beanfun_service::QrPollStatus::Confirmed {
        let session =
            beanfun_service::qr_login_complete(&ss.http_client, &session_key, &ss.cookie_jar)
                .await
                .map_err(login_err_to_dto)?;

        let accounts =
            beanfun_service::get_game_accounts(&ss.http_client, &session, &ss.cookie_jar)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("failed to fetch game accounts after QR login: {e}");
                    Vec::new()
                });

        let dto = SessionDto::from_session(&session, &session_id);
        *ss.session.write().await = Some(session);
        *ss.game_accounts.write().await = accounts;
        tracing::info!("user logged in via QR: {}", dto.account_name);

        return Ok(QrPollResult {
            status: beanfun_service::QrPollStatus::Confirmed,
            session: Some(ss.session.read().await.clone().unwrap()),
        });
    }

    Ok(result)
}

/// Verify a TOTP code (HK region).
///
/// Reads the partial session (with TOTP state) stored during login,
/// then calls the session-based TOTP verification.
#[tauri::command]
pub async fn totp_verify(
    session_id: String,
    code: String,
    state: State<'_, AppState>,
) -> Result<SessionDto, ErrorDto> {
    auth::validate_input("code", &code).map_err(to_dto)?;

    let ss = state.require_session(&session_id).await?;

    let partial_session = {
        let session_guard = ss.session.read().await;
        session_guard
            .clone()
            .ok_or(AuthError::NotAuthenticated)
            .map_err(to_dto)?
    };

    // Use the session-based TOTP verification that has access to totp_state
    let session =
        beanfun_service::hk_totp_verify_with_session(&ss.http_client, &code, &partial_session)
            .await
            .map_err(login_err_to_dto)?;

    let dto = SessionDto::from_session(&session, &session_id);

    let accounts = beanfun_service::get_game_accounts(&ss.http_client, &session, &ss.cookie_jar)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to fetch game accounts after TOTP: {e}");
            Vec::new()
        });

    *ss.session.write().await = Some(session);
    *ss.game_accounts.write().await = accounts;

    tracing::info!("user verified TOTP: {}", dto.account_name);
    Ok(dto)
}

/// Fetch the advance check (verification) page for TW login.
///
/// Returns the form state including captcha image as base64.
#[tauri::command]
pub async fn get_advance_check(
    session_id: String,
    url: Option<String>,
    state: State<'_, AppState>,
) -> Result<beanfun_service::AdvanceCheckState, ErrorDto> {
    let ss = state.require_session(&session_id).await?;

    let check_state = beanfun_service::get_advance_check_page(&ss.http_client, url.as_deref())
        .await
        .map_err(login_err_to_dto)?;

    tracing::info!("advance check page loaded");
    Ok(check_state)
}

/// Submit the advance check verification form.
///
/// Returns `true` if verification succeeded. After success, the caller
/// should retry the login.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn submit_advance_check(
    session_id: String,
    viewstate: String,
    viewstate_generator: String,
    event_validation: String,
    samplecaptcha: String,
    submit_url: String,
    verify_code: String,
    captcha_code: String,
    state: State<'_, AppState>,
) -> Result<bool, ErrorDto> {
    let ss = state.require_session(&session_id).await?;

    let check_state = beanfun_service::AdvanceCheckState {
        viewstate,
        viewstate_generator,
        event_validation,
        samplecaptcha,
        submit_url,
        captcha_image_base64: String::new(),
        auth_hint: String::new(),
    };

    let result = beanfun_service::submit_advance_check(
        &ss.http_client,
        &check_state,
        &verify_code,
        &captcha_code,
    )
    .await
    .map_err(login_err_to_dto)?;

    Ok(result)
}

/// Refresh the captcha image for an in-progress advance check.
#[tauri::command]
pub async fn refresh_advance_check_captcha(
    session_id: String,
    samplecaptcha: String,
    state: State<'_, AppState>,
) -> Result<String, ErrorDto> {
    let ss = state.require_session(&session_id).await?;

    let image = beanfun_service::refresh_advance_check_captcha(&ss.http_client, &samplecaptcha)
        .await
        .map_err(login_err_to_dto)?;

    Ok(image)
}

/// Log out — clear all in-memory credentials for this session (Req 1.8, 13.3).
#[tauri::command]
pub async fn logout(session_id: String, state: State<'_, AppState>) -> Result<(), ErrorDto> {
    // Try to call beanfun logout endpoint before removing the session
    if let Some(ss) = state.get_session(&session_id).await {
        let region = state.config.read().await.region.clone();
        let _ = beanfun_service::logout(&ss.http_client, &region).await;
    }

    state.remove_session(&session_id).await;
    tracing::info!("session {session_id} logged out and removed");
    Ok(())
}

/// Refresh the current session if it's about to expire.
///
/// Called internally or from the frontend heartbeat. Not exposed as a
/// primary user action — it's automatic (Req 1.4).
#[tauri::command]
pub async fn refresh_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<SessionDto, ErrorDto> {
    let ss = state.require_session(&session_id).await?;

    let action = {
        let session_guard = ss.session.read().await;
        auth::decide_session_action(&session_guard)
    };

    match action {
        SessionAction::UseExisting => {
            let session_guard = ss.session.read().await;
            let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;
            Ok(SessionDto::from_session(session, &session_id))
        }
        SessionAction::AttemptRefresh => {
            let (refresh_token, region) = {
                let session_guard = ss.session.read().await;
                let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;
                (
                    session
                        .refresh_token
                        .clone()
                        .ok_or(AuthError::SessionExpired)
                        .map_err(to_dto)?,
                    session.region.clone(),
                )
            };

            let new_session =
                beanfun_service::refresh_session(&ss.http_client, &refresh_token, &region)
                    .await
                    .map_err(login_err_to_dto)?;

            let dto = SessionDto::from_session(&new_session, &session_id);
            *ss.session.write().await = Some(new_session);
            tracing::info!("session refreshed for {}", dto.account_name);
            Ok(dto)
        }
        SessionAction::ReAuthenticate => Err(to_dto(AuthError::SessionExpired)),
    }
}

// ---------------------------------------------------------------------------
// Saved account commands (global state — no session_id needed)
// ---------------------------------------------------------------------------

/// DTO for saved login accounts sent to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAccountDto {
    pub account: String,
    pub region: String,
    pub has_password: bool,
    pub remember_password: bool,
}

/// Return saved accounts for the current region.
///
/// Used by the login form to populate the account dropdown and auto-fill.
#[tauri::command]
pub async fn get_saved_accounts(
    state: State<'_, AppState>,
) -> Result<Vec<SavedAccountDto>, ErrorDto> {
    let region = state.config.read().await.region.clone();
    let region_str = format!("{region:?}");

    let accounts = state.saved_accounts.read().await;
    let dtos = crate::services::account_storage::get_accounts_for_region(&accounts, &region_str)
        .iter()
        .map(|a| SavedAccountDto {
            account: a.account.clone(),
            region: a.region.clone(),
            has_password: !a.password.is_empty(),
            remember_password: a.remember_password,
        })
        .collect();

    Ok(dtos)
}

/// Return all saved accounts across all regions.
///
/// Used by the Account Manager tab in the toolbox.
#[tauri::command]
pub async fn get_all_saved_accounts(
    state: State<'_, AppState>,
) -> Result<Vec<SavedAccountDto>, ErrorDto> {
    let accounts = state.saved_accounts.read().await;
    let dtos = accounts
        .iter()
        .map(|a| SavedAccountDto {
            account: a.account.clone(),
            region: a.region.clone(),
            has_password: !a.password.is_empty(),
            remember_password: a.remember_password,
        })
        .collect();

    Ok(dtos)
}

/// Return the last used account for the current region, including the
/// saved password if available.
///
/// Used on app launch to auto-fill the login form.
#[tauri::command]
pub async fn get_last_saved_account(
    state: State<'_, AppState>,
) -> Result<Option<LastSavedAccountDto>, ErrorDto> {
    let region = state.config.read().await.region.clone();
    let region_str = format!("{region:?}");

    let accounts = state.saved_accounts.read().await;
    let result =
        crate::services::account_storage::get_last_account(&accounts, &region_str).map(|a| {
            LastSavedAccountDto {
                account: a.account.clone(),
                password: a.password.clone(),
                remember_password: a.remember_password,
            }
        });

    Ok(result)
}

/// DTO for the last saved account including the password.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastSavedAccountDto {
    pub account: String,
    pub password: String,
    pub remember_password: bool,
}

/// Return a specific saved account's details (including password) by account ID.
///
/// Used when the user selects a different account from the dropdown.
#[tauri::command]
pub async fn get_saved_account_detail(
    account: String,
    state: State<'_, AppState>,
) -> Result<Option<LastSavedAccountDto>, ErrorDto> {
    let region = state.config.read().await.region.clone();
    let region_str = format!("{region:?}");

    let accounts = state.saved_accounts.read().await;
    let result = crate::services::account_storage::get_account(&accounts, &region_str, &account)
        .map(|a| LastSavedAccountDto {
            account: a.account.clone(),
            password: a.password.clone(),
            remember_password: a.remember_password,
        });

    Ok(result)
}

/// Delete a saved login account by account ID.
///
/// Removes the account from the in-memory list and persists to disk.
/// If `region` is provided, deletes only from that region; otherwise
/// uses the current app region.
#[tauri::command]
pub async fn delete_saved_account(
    account: String,
    region: Option<String>,
    state: State<'_, AppState>,
) -> Result<bool, ErrorDto> {
    let region_str = match region {
        Some(r) => r,
        None => {
            let r = state.config.read().await.region.clone();
            format!("{r:?}")
        }
    };

    let removed = {
        let mut accounts = state.saved_accounts.write().await;
        crate::services::account_storage::remove_account(&mut accounts, &region_str, &account)
    };

    if removed {
        let accounts = state.saved_accounts.read().await;
        if let Err(e) =
            crate::services::account_storage::save_accounts(&state.accounts_path, &accounts).await
        {
            tracing::warn!("failed to persist saved accounts after delete: {e}");
        }
        tracing::info!("deleted saved account: {account}");
    }

    Ok(removed)
}

/// Save login credentials after a successful login.
///
/// The account is always saved. The password is only persisted if
/// `remember_password` is `true`.
#[tauri::command]
pub async fn save_login_credentials(
    account: String,
    password: String,
    remember_password: bool,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    let region = state.config.read().await.region.clone();
    let region_str = format!("{region:?}");

    {
        let mut accounts = state.saved_accounts.write().await;
        crate::services::account_storage::upsert_account(
            &mut accounts,
            &region_str,
            &account,
            &password,
            remember_password,
        );
    }

    // Persist to disk
    let accounts = state.saved_accounts.read().await;
    if let Err(e) =
        crate::services::account_storage::save_accounts(&state.accounts_path, &accounts).await
    {
        tracing::warn!("failed to persist saved accounts: {e}");
    }

    tracing::info!("saved login credentials (remember={})", remember_password);
    Ok(())
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

async fn login_with_native_fallback(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    account: &str,
    password: &str,
    region: &crate::models::session::Region,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    tokens: &beanfun_service::RecaptchaTokens,
) -> Result<Session, beanfun_service::LoginError> {
    match beanfun_service::login(client, account, password, region, cookie_jar, tokens).await {
        Ok(session) => Ok(session),
        Err(err) if should_try_session_key_fallback(&err) => {
            tracing::warn!(
                region = ?region,
                account = %account,
                "Primary login path failed before session key bootstrap; trying native fallbacks"
            );
            let session_key = fetch_session_key_with_fallback(app, region).await?;
            beanfun_service::login_with_session_key(
                client,
                account,
                password,
                region,
                cookie_jar,
                &session_key,
                tokens,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

async fn qr_login_start_with_native_fallback(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    region: &crate::models::session::Region,
) -> Result<QrCodeData, beanfun_service::LoginError> {
    match beanfun_service::qr_login_start(client, region).await {
        Ok(data) => Ok(data),
        Err(err) if should_try_session_key_fallback(&err) => {
            tracing::warn!(region = ?region, "QR start failed before session key bootstrap; trying native fallbacks");
            let session_key = fetch_session_key_with_fallback(app, region).await?;
            beanfun_service::qr_login_start_with_session_key(client, region, &session_key).await
        }
        Err(err) => Err(err),
    }
}

fn should_try_session_key_fallback(err: &beanfun_service::LoginError) -> bool {
    matches!(
        err,
        beanfun_service::LoginError::Network(
            crate::core::error::NetworkError::ConnectionFailed { .. }
        ) | beanfun_service::LoginError::Network(crate::core::error::NetworkError::Timeout { .. })
    )
}

async fn fetch_session_key_with_fallback(
    app: &tauri::AppHandle,
    region: &crate::models::session::Region,
) -> Result<String, beanfun_service::LoginError> {
    if let Some(session_key) = fetch_session_key_via_powershell(region).await? {
        tracing::info!(region = ?region, session_key_len = session_key.len(), "Session key obtained via PowerShell/.NET fallback");
        return Ok(session_key);
    }

    let webview = fetch_session_key_via_webview2(app, region).await?;
    let session_key = match region {
        crate::models::session::Region::HK => {
            beanfun_service::parse_hk_session_key_html(&webview.html)?
        }
        crate::models::session::Region::TW => {
            beanfun_service::parse_tw_session_key_url(&webview.url)?
        }
    };
    tracing::info!(region = ?region, session_key_len = session_key.len(), "Session key obtained via WebView2 fallback");
    Ok(session_key)
}

async fn fetch_session_key_via_powershell(
    region: &crate::models::session::Region,
) -> Result<Option<String>, beanfun_service::LoginError> {
    #[cfg(target_os = "windows")]
    {
        let script = match region {
            crate::models::session::Region::HK => format!(
                r#"$ProgressPreference='SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
$req = [System.Net.HttpWebRequest]::Create('{0}')
$req.Method = 'GET'
$req.UserAgent = '{1}'
$req.AutomaticDecompression = [System.Net.DecompressionMethods]::GZip -bor [System.Net.DecompressionMethods]::Deflate
$req.Headers['Accept-Encoding'] = 'identity'
$resp = $req.GetResponse()
$reader = New-Object System.IO.StreamReader($resp.GetResponseStream(), [System.Text.Encoding]::UTF8)
$body = $reader.ReadToEnd()
$reader.Close()
$resp.Close()
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Write-Output $body"#,
                "https://bfweb.hk.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0",
                WEBVIEW_USER_AGENT
            ),
            crate::models::session::Region::TW => format!(
                r#"$ProgressPreference='SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
$req = [System.Net.HttpWebRequest]::Create('{0}')
$req.Method = 'GET'
$req.UserAgent = '{1}'
$req.AllowAutoRedirect = $true
$req.AutomaticDecompression = [System.Net.DecompressionMethods]::GZip -bor [System.Net.DecompressionMethods]::Deflate
$req.Headers['Accept-Encoding'] = 'identity'
$resp = $req.GetResponse()
$finalUrl = $resp.ResponseUri.AbsoluteUri
$resp.Close()
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Write-Output $finalUrl"#,
                "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0",
                WEBVIEW_USER_AGENT
            ),
        };

        let encoded =
            base64::engine::general_purpose::STANDARD.encode(encode_powershell_script(&script));
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
                .output()
        })
        .await
        .map_err(|e| {
            beanfun_service::LoginError::Network(
                crate::core::error::NetworkError::ConnectionFailed {
                    url: format!("powershell session-key fallback task failed ({e})"),
                },
            )
        })?
        .map_err(|e| {
            beanfun_service::LoginError::Network(
                crate::core::error::NetworkError::ConnectionFailed {
                    url: format!("powershell session-key fallback failed to launch ({e})"),
                },
            )
        })?;

        if !output.status.success() {
            tracing::warn!(
                region = ?region,
                status = ?output.status.code(),
                stderr = %String::from_utf8_lossy(&output.stderr),
                "PowerShell/.NET session-key fallback failed"
            );
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            tracing::warn!(region = ?region, "PowerShell/.NET session-key fallback returned empty output");
            return Ok(None);
        }

        let session_key = match region {
            crate::models::session::Region::HK => {
                beanfun_service::parse_hk_session_key_html(&stdout)?
            }
            crate::models::session::Region::TW => {
                beanfun_service::parse_tw_session_key_url(&stdout)?
            }
        };

        Ok(Some(session_key))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = region;
        Ok(None)
    }
}

#[cfg(target_os = "windows")]
fn encode_powershell_script(script: &str) -> Vec<u8> {
    script
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect()
}

async fn fetch_session_key_via_webview2(
    app: &tauri::AppHandle,
    region: &crate::models::session::Region,
) -> Result<WebviewFetchResult, beanfun_service::LoginError> {
    let request_id = format!("session-key-{}", uuid::Uuid::new_v4());
    let label = format!("session-key-webview-{}", uuid::Uuid::new_v4());
    let results = SESSION_KEY_WEBVIEW_RESULTS.get_or_init(|| Mutex::new(HashMap::new()));
    let (tx, rx) = tokio::sync::oneshot::channel();
    results.lock().unwrap().insert(request_id.clone(), tx);

    let start_url = match region {
        crate::models::session::Region::HK => {
            "https://bfweb.hk.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0"
        }
        crate::models::session::Region::TW => {
            "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0"
        }
    };

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| {
            beanfun_service::LoginError::Network(
                crate::core::error::NetworkError::ConnectionFailed {
                    url: format!("failed to get app data dir for session-key webview ({e})"),
                },
            )
        })?
        .join("session-key-fallback");
    let _ = std::fs::create_dir_all(&data_dir);

    let init_script = format!(
        r#"
(() => {{
  const REQUEST_ID = "{}";
  const START_URL = "{}";
  if (window.location.href === 'about:blank') {{
    setTimeout(() => {{ window.location.href = START_URL; }}, 50);
    return;
  }}
  const send = () => {{
    const html = document.documentElement ? document.documentElement.outerHTML : document.body?.outerHTML || '';
    if (window.__TAURI_INTERNALS__) {{
      window.__TAURI_INTERNALS__.invoke('session_key_webview_done', {{
        requestId: REQUEST_ID,
        url: window.location.href,
        html
      }}).catch(err => console.error('[SessionKeyFallback] invoke failed', err));
    }}
  }};
  if (document.readyState === 'complete' || document.readyState === 'interactive') {{
    setTimeout(send, 50);
  }} else {{
    window.addEventListener('DOMContentLoaded', () => setTimeout(send, 50), {{ once: true }});
  }}
}})();
"#,
        request_id, start_url
    );

    let window = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("Session Key Fallback")
    .visible(false)
    .focused(false)
    .resizable(false)
    .inner_size(320.0, 240.0)
    .data_directory(data_dir)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .initialization_script(&init_script)
    .build()
    .map_err(|e| {
        beanfun_service::LoginError::Network(crate::core::error::NetworkError::ConnectionFailed {
            url: format!("failed to create session-key fallback webview ({e})"),
        })
    })?;

    let result = tokio::time::timeout(std::time::Duration::from_secs(20), rx)
        .await
        .map_err(|_| {
            beanfun_service::LoginError::Network(crate::core::error::NetworkError::Timeout {
                url: "session-key webview fallback timed out".to_string(),
            })
        })?
        .map_err(|_| {
            beanfun_service::LoginError::Network(
                crate::core::error::NetworkError::ConnectionFailed {
                    url: "session-key webview fallback channel closed".to_string(),
                },
            )
        })?;

    let _ = window.destroy();
    SESSION_KEY_WEBVIEW_RESULTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .remove(&request_id);

    Ok(result)
}

#[tauri::command]
pub async fn session_key_webview_done(
    request_id: String,
    url: String,
    html: String,
) -> Result<(), ErrorDto> {
    tracing::info!(
        request_id = %request_id,
        final_url = %url,
        html_len = html.len(),
        "Session-key WebView2 fallback captured page"
    );

    if let Some(sender) = SESSION_KEY_WEBVIEW_RESULTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .remove(&request_id)
    {
        let _ = sender.send(WebviewFetchResult { url, html });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// GamePass login commands (TW region)
// ---------------------------------------------------------------------------

/// User-Agent for WebView2 windows and HTTP requests.
const WEBVIEW_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

/// Open GamePass login popup for TW region.
///
/// Creates a new session for this GamePass login flow. The entire login
/// flow happens inside the WebView2:
/// 1. Navigate to `bflogin/default.aspx` → server redirects to `Login/Index?pSKey={skey}`
/// 2. Init script auto-clicks `a.use-gama-pass` on the login page
/// 3. User completes GamePass OAuth in the webview
/// 4. Init script polls `echo_token.ashx` until session is ready
/// 5. Fetches account list HTML inside the webview, then signals backend
#[tauri::command]
pub async fn open_gamepass_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let label = "gamepass-login";

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let config = state.config.read().await;
    if config.region != crate::models::session::Region::TW {
        return Err(ErrorDto {
            code: "AUTH_GAMEPASS_UNSUPPORTED".to_string(),
            message: "GamePass login is only available for TW region".to_string(),
            category: crate::models::error::ErrorCategory::Authentication,
            details: None,
        });
    }
    let incognito = config.gamepass_incognito;
    drop(config);

    // Create a new session for this GamePass login flow
    let (session_id, _) = state.create_session().await;
    tracing::info!("GamePass: created session {session_id}");

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // Navigate to bflogin/default.aspx — the server will redirect to
    // login.beanfun.com/Login/Index?pSKey={skey} automatically.
    // This way the WebView2 has the same session cookies as the skey request.
    let start_url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";

    // Pass session_id to the init script so it can invoke gamepass_webview_done with it
    let init_script = format!(
        r#"
    (() => {{
        const SESSION_ID = "{}";
        const url = window.location.href;
        const onBeanfun = url.includes('beanfun.com');
        const onLoginPage = url.includes('Login/Index');
        const onGamania = url.includes('accounts.gamania.com');
        const onDefaultAspx = url.includes('bflogin/default.aspx');

        Object.defineProperty(navigator, 'webdriver', {{get: () => false}});

        // Skip: initial redirect page, gamania OAuth, non-beanfun pages
        if (onDefaultAspx || onGamania || !onBeanfun) return;

        const _origFetch = window.fetch;

        // On login page: auto-click GamePass button
        if (onLoginPage) {{
            function tryClick() {{
                const btn = document.querySelector('a.use-gama-pass');
                if (btn) {{ btn.click(); console.log('[GamePass] clicked use-gama-pass'); return true; }}
                return false;
            }}
            // Try immediately
            if (!tryClick()) {{
                // Retry with MutationObserver + polling fallback
                const obs = new MutationObserver(() => {{ if (tryClick()) obs.disconnect(); }});
                if (document.body) {{
                    obs.observe(document.body, {{ childList: true, subtree: true }});
                }}
                // Also poll every 200ms for up to 10s as fallback
                let attempts = 0;
                const poller = setInterval(() => {{
                    attempts++;
                    if (tryClick() || attempts > 50) {{
                        clearInterval(poller);
                        obs.disconnect();
                    }}
                }}, 200);
            }}
            return;
        }}

        // On beanfun.com post-login pages (return.aspx, index.aspx, etc)
        // Poll echo_token, then fetch accounts, then signal backend
        console.log('[GamePass] post-login page detected:', url);

        (async function() {{
            // Poll echo_token until session is confirmed
            let ready = false;
            for (let i = 0; i < 60; i++) {{
                try {{
                    const r = await _origFetch(
                        'https://tw.beanfun.com/beanfun_block/generic_handlers/echo_token.ashx?webtoken=1',
                        {{ credentials: 'include' }}
                    );
                    const t = await r.text();
                    if (t.includes('ResultCode:1')) {{
                        console.log('[GamePass] session ready at attempt', i);
                        ready = true;
                        break;
                    }}
                }} catch(e) {{}}
                await new Promise(r => setTimeout(r, 500));
            }}

            if (!ready) {{
                console.log('[GamePass] session not ready, skipping (might be pre-login page)');
                return;
            }}

            // Session is ready — fetch account list
            console.log('[GamePass] fetching account list...');
            let accountHtml = '';
            try {{
                const sc = '610074', sr = 'T9';
                await _origFetch(
                    'https://tw.beanfun.com/beanfun_block/auth.aspx?channel=game_zone'
                    + '&page_and_query=game_start.aspx%3Fservice_code_and_region%3D' + sc + '_' + sr
                    + '&web_token=1',
                    {{ credentials: 'include' }}
                );
                const listResp = await _origFetch(
                    'https://tw.beanfun.com/beanfun_block/game_zone/game_server_account_list.aspx'
                    + '?sc=' + sc + '&sr=' + sr + '&dt=' + Date.now(),
                    {{ credentials: 'include' }}
                );
                accountHtml = await listResp.text();
                console.log('[GamePass] account list length:', accountHtml.length);
            }} catch(e) {{
                console.error('[GamePass] account list fetch failed:', e);
            }}

            const cookies = document.cookie;
            let webToken = 'cookie_auth';
            cookies.split(';').forEach(c => {{
                const t = c.trim();
                if (t.startsWith('bfWebToken=')) webToken = t.substring(11);
            }});

            if (window.__TAURI_INTERNALS__) {{
                console.log('[GamePass] invoking backend...');
                window.__TAURI_INTERNALS__.invoke('gamepass_webview_done', {{
                    sessionId: SESSION_ID,
                    webToken: webToken,
                    cookies: cookies,
                    accountHtml: accountHtml
                }}).then(() => console.log('[GamePass] SUCCESS'))
                  .catch(e => console.error('[GamePass] FAILED', e));
            }}
        }})();
    }})();
    "#,
        session_id
    );

    let mut builder = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External(start_url.parse().unwrap()),
    )
    .title("Beanfun GamePass Login")
    .inner_size(420.0, 580.0)
    .min_inner_size(380.0, 500.0)
    .decorations(true)
    .resizable(true)
    .center()
    .visible(true)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .initialization_script(&init_script)
    .devtools(true);

    // In normal mode, persist WebView2 data so "remember me" works.
    // In incognito mode, use a unique temp directory per session.
    let webview_data_dir = if incognito {
        let temp = std::env::temp_dir()
            .join("MapleLink")
            .join("gamepass-incognito")
            .join(format!("{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        temp
    } else {
        data_dir
    };
    builder = builder.data_directory(webview_data_dir.clone());

    let _window = builder.build().map_err(|e| ErrorDto {
        code: "AUTH_GAMEPASS_WINDOW_FAILED".to_string(),
        message: format!("Failed to open GamePass login window: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // Clean up incognito temp dir when window closes (best-effort)
    if incognito {
        let app_clone = app.clone();
        let temp_dir = webview_data_dir;
        tauri::async_runtime::spawn(async move {
            // Wait for window to be destroyed
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if app_clone.get_webview_window("gamepass-login").is_none() {
                    // Small delay to let WebView2 release file locks
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    tracing::debug!("GamePass: cleaned up incognito temp dir");
                    break;
                }
            }
        });
    }

    tracing::info!("GamePass login window opened");
    // Return the session_id so the frontend can track this GamePass session
    Ok(session_id)
}

/// Called by the GamePass webview init script when login completes.
///
/// Uses WebView2 CookieManager to extract ALL
/// cookies (including HttpOnly), injects them into the session's reqwest cookie jar,
/// then fetches game accounts via reqwest.
#[tauri::command]
pub async fn gamepass_webview_done(
    session_id: String,
    web_token: String,
    _cookies: String,
    account_html: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    use tauri::Emitter;

    tracing::info!("=== GamePass webview_done START (session: {session_id}) ===");

    let ss = state.require_session(&session_id).await?;

    // Step 1: Extract ALL cookies from WebView2 via CookieManager (including HttpOnly)
    let all_cookies = extract_webview2_cookies(&app, "gamepass-login").await;
    tracing::info!(
        "GamePass: extracted {} cookies from WebView2",
        all_cookies.len()
    );

    // Find bfWebToken from the extracted cookies
    let real_web_token = all_cookies
        .iter()
        .find(|(name, _, _, _)| name == "bfWebToken")
        .map(|(_, value, _, _)| value.clone())
        .unwrap_or_else(|| {
            tracing::warn!("GamePass: bfWebToken not found in WebView2 cookies, using JS fallback");
            if web_token != "cookie_auth" && !web_token.is_empty() {
                web_token.clone()
            } else {
                String::new()
            }
        });

    if real_web_token.is_empty() {
        tracing::error!("GamePass: no bfWebToken found anywhere");
        let _ = app.emit(
            "gamepass-login-error",
            "GamePass login failed: no bfWebToken".to_string(),
        );
        return Ok(());
    }

    tracing::info!(
        "GamePass: bfWebToken = {}...",
        &real_web_token[..real_web_token.len().min(20)]
    );

    // Step 2: Inject ALL cookies into the session's reqwest jar
    let tw_url: url::Url = "https://tw.beanfun.com/".parse().unwrap();
    let login_url: url::Url = "https://login.beanfun.com/".parse().unwrap();
    let newlogin_url: url::Url = "https://tw.newlogin.beanfun.com/".parse().unwrap();

    for (name, value, domain, path) in &all_cookies {
        let clean_domain = domain.trim_start_matches('.');
        let jar_url =
            if clean_domain.contains("login.beanfun.com") && !clean_domain.contains("newlogin") {
                &login_url
            } else if clean_domain.contains("newlogin") {
                &newlogin_url
            } else {
                &tw_url
            };
        let path_str = if path.is_empty() { "/" } else { path.as_str() };
        let cookie_str = format!("{}={}; Domain={}; Path={}", name, value, domain, path_str);
        ss.cookie_jar.add_cookie_str(&cookie_str, jar_url);
    }

    tracing::info!("GamePass: injected all cookies into session's reqwest jar");

    // Step 3: Build session with real bfWebToken
    let session = crate::models::session::Session {
        token: real_web_token,
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: crate::models::session::Region::TW,
        account_name: "GamePass".to_string(),
        session_key: None,
        totp_state: None,
    };

    // Step 4: Fetch game accounts via reqwest (now has full cookies)
    let accounts = crate::services::beanfun_service::get_game_accounts(
        &ss.http_client,
        &session,
        &ss.cookie_jar,
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("GamePass: reqwest get_game_accounts failed: {e}, trying webview HTML");
        crate::services::beanfun_service::parse_tw_account_list_html(&account_html)
    });

    tracing::info!("GamePass: got {} accounts", accounts.len());

    let dto = SessionDto::from_session(&session, &session_id);
    *ss.session.write().await = Some(session);
    *ss.game_accounts.write().await = accounts;

    let _ = app.emit("gamepass-login-complete", dto);

    if let Some(win) = app.get_webview_window("gamepass-login") {
        let _ = win.destroy();
    }

    tracing::info!("=== GamePass webview_done FINISHED ===");
    Ok(())
}

// ---------------------------------------------------------------------------
// Regular (帳密) web login — full login in a webview, then harvest cookies
// ---------------------------------------------------------------------------
//
// Same robust pattern as GamePass: the user completes the entire login inside a
// real beanfun page (account + password + reCAPTCHA + any advance check), then
// we harvest ALL cookies from WebView2 and build the reqwest session — instead
// of the fragile "grab a reCAPTCHA token and replay it" approach. The only
// difference from GamePass is the login page prefills the saved credentials.

/// Injected into the regular web-login window. Reads `window.__ML_*` globals
/// (set by a prelude) — a plain raw string so the JS braces need no escaping.
const REGULAR_LOGIN_SCRIPT: &str = r#"
(() => {
  const SESSION_ID = window.__ML_SESSION_ID__ || "";
  const ACCOUNT = window.__ML_ACCOUNT__ || "";
  const PASSWORD = window.__ML_PASSWORD__ || "";

  const url = window.location.href;
  const onBeanfun = url.includes('beanfun.com');
  const onLoginPage = url.includes('Login/Index');
  const onGamania = url.includes('accounts.gamania.com');
  const onDefaultAspx = url.includes('bflogin/default.aspx');

  try { Object.defineProperty(navigator, 'webdriver', { get: () => false }); } catch (e) {}

  // Tauri hijacks window.alert to its dialog plugin, which isn't permitted for
  // this remote origin (CSP/ACL) and throws. Route beanfun's alerts to the
  // console so they never crash the page — real errors also show inline.
  try { window.alert = function (m) { console.log('[beanfun alert]', m); }; } catch (e) {}

  if (onDefaultAspx || onGamania || !onBeanfun) return;

  const _origFetch = window.fetch;
  const hasSession = () => document.cookie.indexOf('bfWebToken=') !== -1;
  console.log('[WebLogin] page', url, '| login?', onLoginPage, '| session?', hasSession());

  // Only treat as the login form when we're on Login/Index AND not yet logged
  // in — a post-login redirect back to a Login/Index URL should fall through to
  // the harvest below.
  if (onLoginPage && !hasSession()) {
    // Self-heal reCAPTCHA: WebView2 Tracking Prevention is only turned off a
    // moment AFTER this window is created, so the very first load inits reCAPTCHA
    // with google/gstatic storage blocked and it fails to render. Wait ~4s (long
    // enough for prevention to be applied), and if no reCAPTCHA widget appeared,
    // reload once — the reload runs with prevention off and it renders. Same
    // proven approach as the standalone reCAPTCHA helper window.
    try {
      const store = window.sessionStorage;
      const RK = '__wl_reloads__';
      setTimeout(() => {
        if (document.querySelector('iframe[src*="recaptcha"]')) return; // rendered OK
        let done = 0;
        try { done = parseInt(store.getItem(RK) || '0', 10) || 0; } catch (e) {}
        if (done >= 1) {
          console.warn('[WebLogin] reCAPTCHA still missing after reload');
          return;
        }
        try { store.setItem(RK, String(done + 1)); } catch (e) {}
        console.warn('[WebLogin] reCAPTCHA not rendered — reloading once');
        location.reload();
      }, 2500);
    } catch (e) {}

    // beanfun's login is a Vue app with a TWO-STEP flow: enter account →
    // CheckAccountType (reveals the password field) → enter password →
    // AccountLogin. The inputs use deliberately-obfuscated names — account is
    // name="aaa", password is name="inputName" (password only shows after
    // step 1). We fill each once when it appears (empty), dispatching a native
    // 'input' event so Vue's v-model picks up the value; the user just solves
    // the reCAPTCHA and clicks through.
    const setVal = (el, v) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
    };
    let accFilled = false;
    let pwFilled = false;
    const prefill = () => {
      const acc = document.querySelector(
        'input[name="aaa"], input[type="email"], input[type="text"]:not([type="hidden"])'
      );
      const pw = document.querySelector('input[name="inputName"], input[type="password"]');
      if (acc && ACCOUNT && !accFilled && !acc.value) { setVal(acc, ACCOUNT); accFilled = true; }
      if (pw && PASSWORD && !pwFilled && !pw.value) { setVal(pw, PASSWORD); pwFilled = true; }
      return accFilled && pwFilled;
    };
    prefill();
    // Keep watching for up to ~60s — the password field only appears after the
    // account step (which needs the reCAPTCHA + a click).
    let n = 0;
    const timer = setInterval(() => {
      n++;
      if (prefill() || n > 120) clearInterval(timer);
    }, 500);

    // Auto-click the login buttons once the user solves the reCAPTCHA, so they
    // only ever have to solve the challenge. Step 1 button is "登入帳號"
    // (submitAccountName), step 2 is "繼續" (submitPassword); both require the
    // reCAPTCHA token, and it resets between steps (so a fresh token = a fresh
    // click). We click the first visible, enabled one that matches.
    const recaptchaToken = () => {
      try {
        const g = window.grecaptcha;
        if (g && g.enterprise && g.enterprise.getResponse) return g.enterprise.getResponse() || '';
        if (g && g.getResponse) return g.getResponse() || '';
      } catch (e) {}
      return '';
    };
    let lastToken = '';
    setInterval(() => {
      const token = recaptchaToken();
      if (!token || token === lastToken) return;
      const btns = document.querySelectorAll('a.ui-btn');
      for (const b of btns) {
        const txt = (b.textContent || '').trim();
        const visible = b.offsetParent !== null;
        const disabled = b.classList.contains('disabled');
        if (visible && !disabled && (txt.indexOf('登入帳號') !== -1 || txt.indexOf('繼續') !== -1)) {
          lastToken = token;
          console.log('[WebLogin] reCAPTCHA solved — auto-clicking', txt);
          b.click();
          break;
        }
      }
    }, 400);
    return;
  }

  // Post-login: nothing to do here. beanfun's page CSP blocks Tauri IPC on
  // these pages, so the backend watches this window's cookies for bfWebToken and
  // harvests them itself (see open_regular_web_login).
  console.log('[WebLogin] post-login page — backend will harvest cookies');
})();
"#;

/// Open the regular (帳密) web-login window: the user completes the whole login
/// on the official page (credentials prefilled), then cookies are harvested.
#[tauri::command]
pub async fn open_regular_web_login(
    session_id: String,
    account: String,
    password: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    auth::validate_input("account", &account).map_err(to_dto)?;
    auth::validate_input("password", &password).map_err(to_dto)?;
    // Session is pre-created by the frontend.
    let _ = state.require_session(&session_id).await?;

    let label = "web-login";
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Fresh incognito profile each time so the login form (for prefill + the
    // user's reCAPTCHA) always shows instead of auto-logging-in from a cookie.
    let data_dir = std::env::temp_dir()
        .join("MapleLink")
        .join("web-login")
        .join(uuid::Uuid::new_v4().to_string());
    let _ = std::fs::create_dir_all(&data_dir);
    let cleanup_dir = data_dir.clone();

    let start_url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";

    let prelude = format!(
        "window.__ML_SESSION_ID__={};window.__ML_ACCOUNT__={};window.__ML_PASSWORD__={};\n",
        serde_json::to_string(&session_id).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&account).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&password).unwrap_or_else(|_| "\"\"".into()),
    );
    let init_script = format!("{prelude}{REGULAR_LOGIN_SCRIPT}");

    let window = WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::External(start_url.parse().expect("static login URL is valid")),
    )
    .title("Beanfun 登入")
    // Wide enough for beanfun's desktop login layout (promo panel + the form
    // with the account/password fields + reCAPTCHA on the right). At ~420px only
    // the left promo shows and the form is scrolled off.
    .inner_size(1000.0, 680.0)
    .min_inner_size(720.0, 560.0)
    .center()
    .visible(true)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .data_directory(data_dir)
    .initialization_script(&init_script)
    .devtools(true)
    .build()
    .map_err(|e| ErrorDto {
        code: "AUTH_WEB_LOGIN_WINDOW_FAILED".to_string(),
        message: format!("Failed to open login window: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // The login page renders Google reCAPTCHA; WebView2 Tracking Prevention
    // blocks its google.com/gstatic storage and makes it unclickable. Disable it
    // for this window (same as the standalone reCAPTCHA helper).
    disable_tracking_prevention(&window);

    // Clean up the incognito profile after the window closes.
    let cleanup_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if cleanup_app.get_webview_window("web-login").is_none() {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = std::fs::remove_dir_all(&cleanup_dir);
                break;
            }
        }
    });

    // Watch the window's cookies for bfWebToken (set once login completes) and
    // harvest them ourselves — beanfun's page CSP blocks Tauri IPC, so the page
    // can't signal us. Fully URL-agnostic; no IPC/capability needed.
    let watch_app = app.clone();
    let watch_session = session_id.clone();
    let watch_account = account.clone();
    tauri::async_runtime::spawn(async move {
        use tauri::Emitter;
        // ~5 minutes at 1.5s.
        for _ in 0..200 {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            let Some(_win) = watch_app.get_webview_window("web-login") else {
                return; // window closed / cancelled
            };
            let cookies = extract_webview2_cookies(&watch_app, "web-login").await;
            if !cookies.iter().any(|(name, _, _, _)| name == "bfWebToken") {
                continue; // not logged in yet
            }
            tracing::info!("web-login: bfWebToken detected — harvesting session");
            let Some(state) = watch_app.try_state::<AppState>() else {
                return;
            };
            let Some(ss) = state.get_session(&watch_session).await else {
                return;
            };
            match finalize_webview_login(
                &watch_app,
                &ss,
                &watch_session,
                "web-login",
                &watch_account,
                "cookie_auth",
                "",
            )
            .await
            {
                Ok(dto) => {
                    tracing::info!("regular web-login complete: {}", dto.account_name);
                    let _ = watch_app.emit("regular-login-complete", dto);
                }
                Err(e) => {
                    tracing::error!("regular web-login harvest failed: {e}");
                    let _ = watch_app.emit("regular-login-error", format!("登入失敗: {e}"));
                }
            }
            if let Some(win) = watch_app.get_webview_window("web-login") {
                let _ = win.destroy();
            }
            return;
        }
        tracing::warn!("web-login: no login detected within timeout");
    });

    tracing::info!("regular web-login window opened");
    Ok(())
}

/// Called by the web-login init script once login completes: harvest cookies,
/// build the session + account list, and emit `regular-login-complete`.
#[tauri::command]
pub async fn regular_web_login_done(
    session_id: String,
    account: String,
    web_token: String,
    account_html: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    use tauri::Emitter;

    let ss = state.require_session(&session_id).await?;
    match finalize_webview_login(
        &app,
        &ss,
        &session_id,
        "web-login",
        &account,
        &web_token,
        &account_html,
    )
    .await
    {
        Ok(dto) => {
            tracing::info!("regular web-login complete: {}", dto.account_name);
            let _ = app.emit("regular-login-complete", dto);
        }
        Err(e) => {
            tracing::error!("regular web-login failed: {e}");
            let _ = app.emit("regular-login-error", format!("登入失敗: {e}"));
        }
    }

    if let Some(win) = app.get_webview_window("web-login") {
        let _ = win.destroy();
    }
    Ok(())
}

/// Shared finalization for a webview-completed beanfun login: harvest ALL
/// WebView2 cookies from `window_label`, inject them into the session's reqwest
/// jar, build the [`Session`] with `account_name`, fetch game accounts, and
/// store both. Returns the [`SessionDto`] or an error message.
async fn finalize_webview_login(
    app: &tauri::AppHandle,
    ss: &std::sync::Arc<crate::models::session_state::SessionState>,
    session_id: &str,
    window_label: &str,
    account_name: &str,
    web_token_fallback: &str,
    account_html: &str,
) -> Result<SessionDto, String> {
    let all_cookies = extract_webview2_cookies(app, window_label).await;
    tracing::info!(
        "web-login: extracted {} cookies from WebView2",
        all_cookies.len()
    );

    let real_web_token = all_cookies
        .iter()
        .find(|(name, _, _, _)| name == "bfWebToken")
        .map(|(_, value, _, _)| value.clone())
        .unwrap_or_else(|| {
            if web_token_fallback != "cookie_auth" && !web_token_fallback.is_empty() {
                web_token_fallback.to_string()
            } else {
                String::new()
            }
        });

    if real_web_token.is_empty() {
        return Err("no bfWebToken found in webview cookies".to_string());
    }

    let tw_url: url::Url = "https://tw.beanfun.com/".parse().unwrap();
    let login_url: url::Url = "https://login.beanfun.com/".parse().unwrap();
    let newlogin_url: url::Url = "https://tw.newlogin.beanfun.com/".parse().unwrap();
    for (name, value, domain, path) in &all_cookies {
        let clean_domain = domain.trim_start_matches('.');
        let jar_url =
            if clean_domain.contains("login.beanfun.com") && !clean_domain.contains("newlogin") {
                &login_url
            } else if clean_domain.contains("newlogin") {
                &newlogin_url
            } else {
                &tw_url
            };
        let path_str = if path.is_empty() { "/" } else { path.as_str() };
        let cookie_str = format!("{}={}; Domain={}; Path={}", name, value, domain, path_str);
        ss.cookie_jar.add_cookie_str(&cookie_str, jar_url);
    }

    let session = crate::models::session::Session {
        token: real_web_token,
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: crate::models::session::Region::TW,
        account_name: account_name.to_string(),
        session_key: None,
        totp_state: None,
    };

    let accounts = crate::services::beanfun_service::get_game_accounts(
        &ss.http_client,
        &session,
        &ss.cookie_jar,
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("web-login: reqwest get_game_accounts failed: {e}, using webview HTML");
        crate::services::beanfun_service::parse_tw_account_list_html(account_html)
    });
    tracing::info!("web-login: got {} accounts", accounts.len());

    let dto = SessionDto::from_session(&session, session_id);
    *ss.session.write().await = Some(session);
    *ss.game_accounts.write().await = accounts;
    Ok(dto)
}

/// A cookie tuple: (name, value, domain, path).
type CookieTuple = (String, String, String, String);

/// Extract all cookies from a WebView2 window using the native CookieManager API.
/// This reads HttpOnly cookies too (including secure/httponly flags).
#[cfg(target_os = "windows")]
async fn extract_webview2_cookies(app: &tauri::AppHandle, label: &str) -> Vec<CookieTuple> {
    use std::sync::{Arc, Mutex};
    use tauri::Manager;

    let Some(webview) = app.get_webview_window(label) else {
        tracing::warn!("GamePass: webview '{}' not found", label);
        return Vec::new();
    };

    let cookies: Arc<Mutex<Vec<CookieTuple>>> = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let cookies_clone = cookies.clone();
    let tx_clone = tx.clone();

    let urls = vec![
        "https://tw.beanfun.com".to_string(),
        "https://login.beanfun.com".to_string(),
        "https://tw.newlogin.beanfun.com".to_string(),
    ];
    let total = urls.len();

    let result = webview.with_webview(move |wv| {
        use webview2_com::{GetCookiesCompletedHandler, Microsoft::Web::WebView2::Win32::*};
        use windows_core::Interface;

        unsafe {
            let controller = wv.controller();
            let core: ICoreWebView2 = controller
                .CoreWebView2()
                .expect("failed to get CoreWebView2");

            let core2: ICoreWebView2_2 = core.cast().expect("failed to cast to ICoreWebView2_2");
            let cookie_manager: ICoreWebView2CookieManager =
                core2.CookieManager().expect("failed to get CookieManager");

            let remaining = Arc::new(Mutex::new(total));

            for url_str in urls {
                let cookies_ref = cookies_clone.clone();
                let remaining_ref = remaining.clone();
                let tx_ref = tx_clone.clone();

                let handler =
                    GetCookiesCompletedHandler::create(Box::new(move |hr, cookie_list| {
                        if hr.is_ok() {
                            if let Some(list) = cookie_list {
                                let mut count: u32 = 0;
                                let _ = list.Count(&mut count);
                                for i in 0..count {
                                    if let Ok(cookie) = list.GetValueAtIndex(i) {
                                        let mut name = windows_core::PWSTR::null();
                                        let mut value = windows_core::PWSTR::null();
                                        let mut domain = windows_core::PWSTR::null();
                                        let mut path = windows_core::PWSTR::null();

                                        let _ = cookie.Name(&mut name);
                                        let _ = cookie.Value(&mut value);
                                        let _ = cookie.Domain(&mut domain);
                                        let _ = cookie.Path(&mut path);

                                        let n = name.to_string().unwrap_or_default();
                                        let v = value.to_string().unwrap_or_default();
                                        let d = domain.to_string().unwrap_or_default();
                                        let p = path.to_string().unwrap_or_default();

                                        // Free the PWSTR strings
                                        if !name.is_null() {
                                            windows_core::imp::CoTaskMemFree(name.as_ptr() as _);
                                        }
                                        if !value.is_null() {
                                            windows_core::imp::CoTaskMemFree(value.as_ptr() as _);
                                        }
                                        if !domain.is_null() {
                                            windows_core::imp::CoTaskMemFree(domain.as_ptr() as _);
                                        }
                                        if !path.is_null() {
                                            windows_core::imp::CoTaskMemFree(path.as_ptr() as _);
                                        }

                                        if !n.is_empty() {
                                            if let Ok(mut c) = cookies_ref.lock() {
                                                c.push((n, v, d, p));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let mut rem = remaining_ref.lock().unwrap();
                        *rem -= 1;
                        if *rem == 0 {
                            if let Some(sender) = tx_ref.lock().unwrap().take() {
                                let _ = sender.send(());
                            }
                        }
                        Ok(())
                    }));

                let url_hstring = windows_core::HSTRING::from(&url_str);
                let _ = cookie_manager.GetCookies(&url_hstring, &handler);
            }
        }
    });

    if result.is_err() {
        tracing::warn!("GamePass: with_webview failed");
        return Vec::new();
    }

    // Wait for all cookie callbacks (timeout 10s)
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), rx).await;

    let result = cookies.lock().unwrap().clone();
    tracing::info!("GamePass: extracted {} total cookies", result.len());
    result
}

#[cfg(not(target_os = "windows"))]
async fn extract_webview2_cookies(_app: &tauri::AppHandle, _label: &str) -> Vec<CookieTuple> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// TW Regular (帳密) login — external reCAPTCHA helper window
// ---------------------------------------------------------------------------
//
// The TW "Regular" (username + password) login on `login.beanfun.com` is
// guarded by Google reCAPTCHA. Inside a bare WebView2 the widget frequently
// fails to render — usually because `www.google.com` is unreachable for the
// user. This helper opens the *official* login page in a small, frameless
// window so the human can solve their own challenge; the injected script only
// (a) swaps the reCAPTCHA origin to the Google-supported `recaptcha.net`
// mirror so the widget loads, (b) trims the surrounding page chrome, and
// (c) hands the token the human produced back to the backend. The challenge
// is solved by a person — nothing here auto-solves or bypasses it.

/// Window label for the external reCAPTCHA helper window.
const RECAPTCHA_WINDOW_LABEL: &str = "recaptcha_window";

/// Set right before the backend destroys the helper window after a SUCCESSFUL
/// token capture, so the window-destroyed handler can tell that apart from a
/// user closing the window. Without this, the destroy after phase 1 emits a
/// spurious `recaptcha-cancelled` that can abort phase 2.
static RECAPTCHA_DELIVERED: AtomicBool = AtomicBool::new(false);

/// Consume the "token delivered" flag. `true` → the window closed because we
/// captured a token (suppress cancel); `false` → user/close abort (emit cancel).
pub fn recaptcha_take_delivered() -> bool {
    RECAPTCHA_DELIVERED.swap(false, Ordering::SeqCst)
}

/// Injected at document-start into the reCAPTCHA helper window.
///
/// No `format!` placeholders here on purpose — it's a plain raw string so the
/// JS braces/backticks need no escaping.
const RECAPTCHA_INIT_SCRIPT: &str = r#"
(() => {
  'use strict';
  // recaptcha.net mirror — ONLY needed where www.google.com is blocked
  // (mainland). It breaks the Enterprise load elsewhere because recaptcha.net
  // does not reliably serve /recaptcha/enterprise.js, leaving
  // `grecaptcha.render` undefined. Keep OFF until mainland support is wired
  // (and verified against the enterprise endpoint).
  const MIRROR_ENABLED = false;
  const GOOGLE = 'www.google.com/recaptcha';
  const MIRROR = 'www.recaptcha.net/recaptcha';
  const swap = (s) =>
    MIRROR_ENABLED && typeof s === 'string' ? s.split(GOOGLE).join(MIRROR) : s;

  // Parity with the rest of the app's webviews.
  try { Object.defineProperty(navigator, 'webdriver', { get: () => false }); } catch (e) {}

  const origCreate = document.createElement.bind(document);

  // Replace a <script> node whose src still points at google.com.
  const rewriteScriptNode = (node) => {
    const next = origCreate('script');
    for (const a of node.attributes) {
      next.setAttribute(a.name, a.name === 'src' ? swap(a.value) : a.value);
    }
    if (node.parentNode) node.parentNode.replaceChild(next, node);
  };

  if (MIRROR_ENABLED) {
    // 1a. Intercept dynamically-created <script> tags and rewrite the origin.
    document.createElement = function (tag) {
      const el = origCreate.apply(document, arguments);
      if (String(tag).toLowerCase() === 'script') {
        try {
          const proto = Object.getOwnPropertyDescriptor(HTMLScriptElement.prototype, 'src');
          if (proto && proto.set) {
            Object.defineProperty(el, 'src', {
              configurable: true,
              enumerable: true,
              get() { return proto.get.call(this); },
              set(v) { proto.set.call(this, swap(v)); },
            });
          }
          const origSetAttr = el.setAttribute.bind(el);
          el.setAttribute = function (name, value) {
            return origSetAttr(name, name === 'src' ? swap(value) : value);
          };
        } catch (e) {}
      }
      return el;
    };

    // 1b. Catch static <script> tags as the parser inserts them.
    try {
      const mo = new MutationObserver((muts) => {
        for (const m of muts) {
          for (const n of m.addedNodes) {
            if (n.tagName === 'SCRIPT' && n.src && n.src.indexOf(GOOGLE) !== -1) {
              rewriteScriptNode(n);
            }
          }
        }
      });
      mo.observe(document.documentElement, { childList: true, subtree: true });
    } catch (e) {}
  }

  const onReady = () => {
    // Sweep any static google.com reCAPTCHA scripts already in the HTML.
    if (MIRROR_ENABLED) {
      document
        .querySelectorAll('script[src*="www.google.com/recaptcha"]')
        .forEach(rewriteScriptNode);
    }

    // The helper window first passes through redirect pages (default.aspx →
    // checkin → Login/Index). Only act on the actual login form.
    if (location.href.indexOf('Login/Index') === -1) return;
    console.log('[reCAPTCHA] on Login/Index, watching for token');

    // 2. Trim the page down to just the reCAPTCHA — but only AFTER the widget
    //    actually renders. If we hid the page up front and beanfun's reCAPTCHA
    //    lost its first-load render race (works after a manual refresh), the
    //    window would be all white. So keep the real page visible until the
    //    widget exists, then hide everything else via the `visibility` trick
    //    (a visible descendant shows through a hidden ancestor).
    let trimmed = false;
    const trimPage = () => {
      if (trimmed) return;
      trimmed = true;
      // Do NOT use `visibility:hidden` on an ancestor of the reCAPTCHA: Chromium
      // suppresses hit-testing for a cross-origin (out-of-process) iframe under
      // a hidden subtree, so the widget would render but be unclickable. Instead
      // lay an opaque mask BELOW reCAPTCHA's z-index (~2e9) and lift the anchor
      // checkbox above it — everything stays interactive.
      const mask = origCreate('div');
      mask.id = '__rc_mask__';
      mask.style.cssText =
        'position:fixed;top:0;left:0;width:100vw;height:100vh;background:#fff;z-index:1000000;';
      (document.body || document.documentElement).appendChild(mask);

      // Lift the anchor checkbox above the mask. The challenge bframe popup
      // already sits at z-index ~2e9, well above the mask, so it shows itself.
      const style = origCreate('style');
      style.textContent = [
        '.g-recaptcha, iframe[src*="anchor"] {',
        '  position: fixed !important; top: 16px !important;',
        '  left: 50% !important; transform: translateX(-50%) !important;',
        '  z-index: 1000001 !important;',
        '}',
      ].join('\n');
      (document.head || document.documentElement).appendChild(style);
    };

    // Reload-once guard to self-heal beanfun's first-load render race.
    let store = null;
    try { store = window.sessionStorage; } catch (e) {}
    const RKEY = '__rc_reloads__';
    const reloads = store ? (parseInt(store.getItem(RKEY) || '0', 10) || 0) : 99;

    let waited = 0;
    const renderWatch = setInterval(() => {
      if (document.querySelector('iframe[src*="recaptcha"]')) {
        clearInterval(renderWatch);
        trimPage();
        return;
      }
      waited += 300;
      if (waited >= 3500) {
        clearInterval(renderWatch);
        if (store && reloads < 1) {
          store.setItem(RKEY, String(reloads + 1));
          console.warn('[reCAPTCHA] widget did not render — reloading once');
          location.reload();
        } else {
          // Give up rather than leave a blank window the user must force-close.
          // Closing fires `recaptcha-cancelled`, which unblocks the frontend.
          console.warn('[reCAPTCHA] widget did not render (reload budget spent) — closing');
          if (store) { try { store.removeItem(RKEY); } catch (e) {} }
          if (window.__TAURI_INTERNALS__) {
            window.__TAURI_INTERNALS__
              .invoke('close_recaptcha_window')
              .catch(() => {});
          }
        }
      }
    }, 300);

    // 3 + 4. Capture the Enterprise token once the human ticks the checkbox.
    //    Primary source is grecaptcha.enterprise.getResponse() — beanfun is an
    //    Enterprise checkbox and reads it straight into its XHR rather than into
    //    a stable hidden input. Fall back to the response <textarea> / beanfun's
    //    own field. We never fire beanfun's submit, so the token stays
    //    unconsumed and can be replayed into AccountLogin's `Captcha` field.
    const readToken = () => {
      try {
        const g = window.grecaptcha;
        if (g && g.enterprise && typeof g.enterprise.getResponse === 'function') {
          const t = g.enterprise.getResponse();
          if (t) return t;
        }
        if (g && typeof g.getResponse === 'function') {
          const t = g.getResponse();
          if (t) return t;
        }
      } catch (e) {}
      const el = document.getElementById('recaptcha-token') ||
        document.querySelector('#g-recaptcha-response, textarea[name="g-recaptcha-response"]');
      return el ? el.value : '';
    };

    const initial = readToken();
    let done = false;
    const timer = setInterval(() => {
      if (done) return;
      const val = readToken();
      if (val && val !== initial && val.length > 50) {
        done = true;
        clearInterval(timer);
        if (store) { try { store.removeItem(RKEY); } catch (e) {} }
        const step = window.__RECAPTCHA_STEP__ || 'login';
        // beanfun's page CSP blocks Tauri IPC (ipc.localhost → connect-src), so
        // hand the token to the backend through a URL fragment it polls.
        // Changing the hash isn't gated by connect-src and doesn't reload.
        // reCAPTCHA tokens are URL-safe base64url, so '~' is a safe separator.
        try { window.location.hash = 'mltoken=' + step + '~' + val; } catch (e) {}
        // Best-effort IPC too, in case CSP allows it on some pages.
        try {
          if (window.__TAURI_INTERNALS__) {
            window.__TAURI_INTERNALS__
              .invoke('submit_login_token', { token: val, step })
              .catch(() => {});
          }
        } catch (e) {}
        console.log('[reCAPTCHA] token captured, handed to backend via fragment');
      }
    }, 500);
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', onReady, { once: true });
  } else {
    onReady();
  }
})();
"#;

/// Open the external reCAPTCHA helper window for TW Regular (帳密) login.
///
/// Points a small, frameless, always-on-top WebView2 at the official login
/// page and injects [`RECAPTCHA_INIT_SCRIPT`]. The human solves the challenge;
/// the script invokes [`submit_login_token`] with the resulting token.
///
/// `step` distinguishes the two challenge points (`"check"` for
/// `CheckAccountType`, `"login"` for `AccountLogin`); it is echoed back in the
/// `recaptcha-token` event so the frontend can route each token. Defaults to
/// `"login"`.
#[tauri::command]
pub async fn open_recaptcha_window(
    step: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    let step = step.unwrap_or_else(|| "login".to_string());

    // Replace any existing helper window so we never stack two. Mark it as a
    // backend close so its destroy doesn't emit a cancel that aborts this solve.
    if let Some(existing) = app.get_webview_window(RECAPTCHA_WINDOW_LABEL) {
        RECAPTCHA_DELIVERED.store(true, Ordering::SeqCst);
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Tag the window with its step (consumed by RECAPTCHA_INIT_SCRIPT). The
    // body's braces are safe here: it's interpolated as a value, not reparsed
    // as a format string.
    let init_script = format!("window.__RECAPTCHA_STEP__ = {step:?};\n{RECAPTCHA_INIT_SCRIPT}");

    // Use a FRESH (incognito) profile every time. A persistent profile retains
    // beanfun's login cookie, so loading Login/Index auto-completes the pSKey
    // login and redirects to the portal — no reCAPTCHA is ever shown, no token
    // is captured, and the frontend hangs on "登入中". A unique dir guarantees a
    // clean login page with the reCAPTCHA every time.
    let data_dir = std::env::temp_dir()
        .join("MapleLink")
        .join("recaptcha-incognito")
        .join(uuid::Uuid::new_v4().to_string());
    let _ = std::fs::create_dir_all(&data_dir);
    let cleanup_dir = data_dir.clone();

    // Navigate to bflogin/default.aspx — the server redirects to
    // login.beanfun.com/Login/Index?pSKey={skey} (a bare Login/Index with no
    // pSKey renders blank). Same entry point as open_gamepass_login.
    let url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";

    let window = WebviewWindowBuilder::new(
        &app,
        RECAPTCHA_WINDOW_LABEL,
        tauri::WebviewUrl::External(url.parse().expect("static reCAPTCHA URL is valid")),
    )
    .title("Beanfun 驗證")
    .inner_size(400.0, 550.0)
    .resizable(false)
    // Keep the native title bar so the user can always close the window — a
    // frameless window that fails to render leaves no way out and hangs the
    // login. Closing fires `recaptcha-cancelled`, which unblocks the frontend.
    .decorations(true)
    .always_on_top(true)
    .center()
    .visible(true)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .data_directory(data_dir)
    .initialization_script(&init_script)
    .build()
    .map_err(|e| ErrorDto {
        code: "AUTH_RECAPTCHA_WINDOW_FAILED".to_string(),
        message: format!("Failed to open reCAPTCHA window: {e}"),
        category: crate::models::error::ErrorCategory::Process,
        details: None,
    })?;

    // WebView2 Tracking Prevention blocks google.com/gstatic third-party
    // storage, which breaks reCAPTCHA's interaction (widget renders but clicks
    // don't verify). Turn it off for this window's profile.
    disable_tracking_prevention(&window);

    // beanfun's page CSP blocks Tauri IPC (ipc.localhost), so the injected
    // script hands the token back via the URL fragment (#mltoken=<step>~<token>).
    // Poll the window URL for it.
    let poll_app = app.clone();
    tauri::async_runtime::spawn(async move {
        // ~3 minutes at 500ms; the window-gone check ends it early on close.
        for _ in 0..360 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let Some(win) = poll_app.get_webview_window(RECAPTCHA_WINDOW_LABEL) else {
                return; // window closed or token already delivered
            };
            let Ok(u) = win.url() else { continue };
            let Some(frag) = u.fragment() else { continue };
            let Some(rest) = frag.strip_prefix("mltoken=") else {
                continue;
            };
            if let Some((step, token)) = rest.split_once('~') {
                if token.len() > 50 {
                    deliver_recaptcha_token(&poll_app, token.to_string(), step.to_string());
                    return;
                }
            }
        }
    });

    // Remove the incognito profile once the window closes (best effort).
    let cleanup_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if cleanup_app
                .get_webview_window(RECAPTCHA_WINDOW_LABEL)
                .is_none()
            {
                // Let WebView2 release its file locks before deleting.
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = std::fs::remove_dir_all(&cleanup_dir);
                break;
            }
        }
    });

    tracing::info!("reCAPTCHA helper window opened");
    Ok(())
}

/// Disable WebView2 Tracking Prevention for a window's profile.
///
/// reCAPTCHA needs third-party storage/cookies on `google.com` / `gstatic.com`;
/// Edge's Tracking Prevention blocks those by default, leaving the widget
/// visible but non-functional. This is a profile-level setting (not a Chromium
/// switch), so it must go through the WebView2 COM API.
#[cfg(target_os = "windows")]
fn disable_tracking_prevention(window: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile3, ICoreWebView2_13, COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE,
    };
    use windows_core::Interface;

    let result = window.with_webview(|wv| unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return;
        };
        let Ok(core13) = core.cast::<ICoreWebView2_13>() else {
            tracing::warn!(
                "reCAPTCHA: ICoreWebView2_13 unavailable; cannot disable tracking prevention"
            );
            return;
        };
        let Ok(profile) = core13.Profile() else {
            return;
        };
        let Ok(profile3) = profile.cast::<ICoreWebView2Profile3>() else {
            tracing::warn!(
                "reCAPTCHA: ICoreWebView2Profile3 unavailable; cannot disable tracking prevention"
            );
            return;
        };
        // Set on the profile (persisted in data_directory). The window goes
        // through a redirect chain (default.aspx → checkin → Login/Index)
        // before the reCAPTCHA loads, so this takes effect well before the
        // widget initializes — no risky Reload needed.
        match profile3
            .SetPreferredTrackingPreventionLevel(COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE)
        {
            Ok(()) => tracing::info!("reCAPTCHA: tracking prevention disabled for helper window"),
            Err(e) => tracing::warn!("reCAPTCHA: failed to disable tracking prevention: {e}"),
        }
    });

    if result.is_err() {
        tracing::warn!("reCAPTCHA: with_webview failed; tracking prevention left at default");
    }
}

#[cfg(not(target_os = "windows"))]
fn disable_tracking_prevention(_window: &tauri::WebviewWindow) {}

/// Close the reCAPTCHA helper window (called by its own script when the widget
/// fails to render, or as a frontend cleanup). Destroying it fires
/// `recaptcha-cancelled`, which unblocks any pending login.
#[tauri::command]
pub async fn close_recaptcha_window(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    if let Some(win) = app.get_webview_window(RECAPTCHA_WINDOW_LABEL) {
        let _ = win.destroy();
    }
    Ok(())
}

/// Payload re-emitted to the frontend when a reCAPTCHA token is captured.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecaptchaTokenEvent {
    pub token: String,
    /// `"check"` or `"login"` — which login step this token is for.
    pub step: String,
}

/// Emit the captured reCAPTCHA token to the frontend and close the helper
/// window. Shared by the fragment-poll path (primary) and the
/// [`submit_login_token`] IPC command (fallback). Guards against double
/// delivery by keying off the helper window still existing.
fn deliver_recaptcha_token(app: &tauri::AppHandle, token: String, step: String) {
    use tauri::Emitter;

    // If the helper window is already gone, the token was already delivered.
    let Some(win) = app.get_webview_window(RECAPTCHA_WINDOW_LABEL) else {
        return;
    };

    tracing::info!(token_len = token.len(), step = %step, "reCAPTCHA token delivered");
    if let Err(e) = app.emit("recaptcha-token", RecaptchaTokenEvent { token, step }) {
        tracing::warn!("failed to emit recaptcha-token event: {e}");
    }
    // Mark this close as a successful delivery so on_window_event does NOT emit
    // recaptcha-cancelled (which would abort the next phase).
    RECAPTCHA_DELIVERED.store(true, Ordering::SeqCst);
    let _ = win.destroy();
}

/// Fallback IPC path for receiving a reCAPTCHA token. Usually beanfun's page
/// CSP blocks Tauri IPC, so the real delivery happens via the URL-fragment poll
/// (see [`open_recaptcha_window`]); this remains for pages where IPC is allowed.
#[tauri::command]
pub async fn submit_login_token(
    token: String,
    step: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), ErrorDto> {
    deliver_recaptcha_token(&app, token, step.unwrap_or_else(|| "login".to_string()));
    Ok(())
}
