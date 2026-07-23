//! Tauri commands for Beanfun authentication.
//!
//! Thin wrappers: validate inputs, delegate to core/service, map errors to [`ErrorDto`].

use tauri::State;

use crate::core::auth::{self, SessionAction};
use crate::core::error::{AppError, AuthError};
use crate::models::app_state::{AppState, SessionInfo};
use crate::models::error::ErrorDto;
use crate::models::session::SessionDto;
use crate::services::beanfun_service::{self, QrCodeData, QrPollResult};
use crate::services::{recaptcha_window, session_key_fallback, webview_login};

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

    let login_result = session_key_fallback::login_with_native_fallback(
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

    let (skey, form_token) = beanfun_service::tw_login_check(
        &ss.http_client,
        &account,
        recaptcha_check.as_deref(),
        &ss.cookie_jar,
    )
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

    let has_login_token = recaptcha_login
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty());
    tracing::info!(
        account = %pending.account,
        skey_len = pending.skey.len(),
        has_login_token,
        "TW login phase 2 (AccountLogin) starting"
    );

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

    session_key_fallback::qr_login_start_with_native_fallback(
        &app,
        &ss.http_client,
        &region,
        &ss.cookie_jar,
    )
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
        let session_for_result = session.clone();
        *ss.session.write().await = Some(session);
        *ss.game_accounts.write().await = accounts;
        tracing::info!("user logged in via QR: {}", dto.account_name);

        return Ok(QrPollResult {
            status: beanfun_service::QrPollStatus::Confirmed,
            session: Some(session_for_result),
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
    let picked = crate::services::account_storage::get_last_account(&accounts, &region_str);
    let region_total = accounts.iter().filter(|a| a.region == region_str).count();
    tracing::info!(
        region = %region_str,
        region_total,
        picked = picked.map(|a| a.account.as_str()).unwrap_or("<none>"),
        stamped = picked.is_some_and(|a| a.last_used_at.is_some()),
        "get_last_saved_account"
    );
    let result = picked.map(|a| LastSavedAccountDto {
        account: a.account.clone(),
        password: a.password.clone(),
        remember_password: a.remember_password,
        verify_info: a.verify_info.clone(),
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
    /// Remembered advance-check verify info (email / phone), if any.
    pub verify_info: Option<String>,
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
            verify_info: a.verify_info.clone(),
        });

    tracing::info!(
        region = %region_str,
        account = %account,
        found = result.is_some(),
        has_verify_info = result.as_ref().is_some_and(|r| r.verify_info.is_some()),
        "get_saved_account_detail"
    );
    Ok(result)
}

/// Remember (or clear) the advance-check verify info (email / phone) for an
/// account, so it can be pre-filled next time an advance check appears.
#[tauri::command]
pub async fn save_verify_info(
    account: String,
    verify_info: String,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    let region = state.config.read().await.region.clone();
    let region_str = format!("{region:?}");

    {
        let mut accounts = state.saved_accounts.write().await;
        crate::services::account_storage::set_verify_info(
            &mut accounts,
            &region_str,
            &account,
            &verify_info,
        );
    }

    let accounts = state.saved_accounts.read().await;
    let saved = crate::services::account_storage::get_account(&accounts, &region_str, &account)
        .and_then(|a| a.verify_info.clone());
    tracing::info!(
        region = %region_str,
        account = %account,
        value_len = verify_info.trim().len(),
        stored = saved.is_some(),
        total_accounts = accounts.len(),
        "save_verify_info persisted"
    );
    if let Err(e) =
        crate::services::account_storage::save_accounts(&state.accounts_path, &accounts).await
    {
        tracing::warn!("failed to persist verify info: {e}");
    }
    Ok(())
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

/// Receives the captured page from the hidden session-key fallback webview
/// (invoked by its init script) and hands it to the waiting fallback.
#[tauri::command]
pub async fn session_key_webview_done(
    request_id: String,
    url: String,
    html: String,
) -> Result<(), ErrorDto> {
    session_key_fallback::deliver_webview_result(&request_id, url, html);
    Ok(())
}

// ---------------------------------------------------------------------------
// Webview login commands (GamePass / regular web login / reCAPTCHA helper)
// ---------------------------------------------------------------------------

/// Open the GamePass login popup (TW region). Returns the new session's ID so
/// the frontend can track this GamePass session.
#[tauri::command]
pub async fn open_gamepass_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, ErrorDto> {
    webview_login::open_gamepass_login_window(app, state.inner()).await
}

/// Open the MapleStory Classic (懷舊服) portal for an already-authenticated
/// session. Seeds the session's beanfun cookies into a webview and drives the
/// galaxy SSO through to the classic portal; the game is launched from there via
/// the site's own `ngm://` handler.
#[tauri::command]
pub async fn open_classic_login(
    session_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    crate::services::classic_service::open_classic_login(session_id, app, state.inner()).await
}

/// Called by the GamePass webview init script when login completes.
///
/// `web_token` is the JS `document.cookie` value, only useful as a fallback
/// (bfWebToken is HttpOnly so JS usually can't read it); the finalizer prefers
/// the WebView2 CookieManager-extracted cookie.
#[tauri::command]
pub async fn gamepass_webview_done(
    session_id: String,
    web_token: String,
    _cookies: String,
    account_html: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    tracing::info!("=== GamePass webview_done (JS IPC) session: {session_id} ===");
    let completed = webview_login::try_finalize_gamepass(
        &app,
        state.inner(),
        &session_id,
        &account_html,
        &web_token,
    )
    .await?;
    if !completed {
        tracing::info!("GamePass (JS IPC): bfWebToken not present yet — backend poll will retry");
    }
    Ok(())
}

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
    auth::validate_input("account", &account).map_err(to_dto)?;
    auth::validate_input("password", &password).map_err(to_dto)?;
    // Session is pre-created by the frontend.
    let _ = state.require_session(&session_id).await?;

    webview_login::open_regular_web_login_window(app, session_id, account, password).await
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
    use tauri::{Emitter, Manager};

    let ss = state.require_session(&session_id).await?;
    match webview_login::finalize_webview_login(
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

/// Open the external reCAPTCHA helper window for TW Regular (帳密) login.
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
    recaptcha_window::open_recaptcha_helper_window(app, step.unwrap_or_else(|| "login".to_string()))
        .await
}

/// Close the reCAPTCHA helper window (called by its own script when the widget
/// fails to render, or as a frontend cleanup). Destroying it fires
/// `recaptcha-cancelled`, which unblocks any pending login.
#[tauri::command]
pub async fn close_recaptcha_window(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    recaptcha_window::close_recaptcha_helper_window(&app);
    Ok(())
}

/// Fallback IPC path for receiving a reCAPTCHA token. Usually beanfun's page
/// CSP blocks Tauri IPC, so the real delivery happens via the URL-fragment poll
/// (see [`recaptcha_window::open_recaptcha_helper_window`]); this remains for
/// pages where IPC is allowed.
#[tauri::command]
pub async fn submit_login_token(
    token: String,
    step: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), ErrorDto> {
    recaptcha_window::deliver_recaptcha_token(
        &app,
        token,
        step.unwrap_or_else(|| "login".to_string()),
    );
    Ok(())
}
