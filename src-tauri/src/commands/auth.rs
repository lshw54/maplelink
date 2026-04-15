//! Tauri commands for Beanfun authentication.
//!
//! Thin wrappers: validate inputs, delegate to core/service, map errors to [`ErrorDto`].

use tauri::Manager;
use tauri::State;

use crate::core::auth::{self, SessionAction};
use crate::core::error::{AppError, AuthError};
use crate::models::app_state::AppState;
use crate::models::error::ErrorDto;
use crate::models::session::Session;
use crate::services::beanfun_service::{self, QrCodeData, QrPollResult};

// ---------------------------------------------------------------------------
// DTOs returned to the frontend
// ---------------------------------------------------------------------------

/// Subset of [`Session`] safe to send to the frontend (no refresh token).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub token: String,
    pub region: String,
    pub account_name: String,
    pub expires_at: String,
}

impl From<&Session> for SessionDto {
    fn from(s: &Session) -> Self {
        Self {
            token: s.token.clone(),
            region: format!("{:?}", s.region),
            account_name: s.account_name.clone(),
            expires_at: s.expires_at.to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Normal username + password login.
#[tauri::command]
pub async fn login(
    account: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<SessionDto, ErrorDto> {
    // Input validation
    auth::validate_input("account", &account).map_err(to_dto)?;
    auth::validate_input("password", &password).map_err(to_dto)?;

    let region = state.config.read().await.region.clone();

    let login_result = beanfun_service::login(
        &state.http_client,
        &account,
        &password,
        &region,
        &state.cookie_jar,
    )
    .await;

    // Handle TOTP required: store partial session and return specific error
    let session = match login_result {
        Ok(s) => s,
        Err(beanfun_service::LoginError::Auth(AuthError::TotpRequired { partial_session })) => {
            *state.session.write().await = Some(*partial_session);
            tracing::info!("login requires TOTP verification");
            return Err(to_dto(AuthError::TotpRequired {
                partial_session: Box::new(
                    state
                        .session
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

    let dto = SessionDto::from(&session);

    // Fetch game accounts right after login (Req 1.6)
    let accounts =
        beanfun_service::get_game_accounts(&state.http_client, &session, &state.cookie_jar)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("failed to fetch game accounts after login: {e}");
                Vec::new()
            });

    *state.session.write().await = Some(session);
    *state.game_accounts.write().await = accounts;

    tracing::info!("user logged in: {}", dto.account_name);
    Ok(dto)
}

/// Start a QR-code login flow (TW region).
#[tauri::command]
pub async fn qr_login_start(state: State<'_, AppState>) -> Result<QrCodeData, ErrorDto> {
    let region = state.config.read().await.region.clone();

    beanfun_service::qr_login_start(&state.http_client, &region)
        .await
        .map_err(login_err_to_dto)
}

/// Poll the status of an in-progress QR-code login.
#[tauri::command]
pub async fn qr_login_poll(
    session_key: String,
    verification_token: String,
    state: State<'_, AppState>,
) -> Result<QrPollResult, ErrorDto> {
    auth::validate_input("session_key", &session_key).map_err(to_dto)?;

    let region = state.config.read().await.region.clone();

    let result = beanfun_service::qr_login_poll(
        &state.http_client,
        &session_key,
        &verification_token,
        &region,
    )
    .await
    .map_err(login_err_to_dto)?;

    // If confirmed, complete the login and store the session
    if result.status == beanfun_service::QrPollStatus::Confirmed {
        let session =
            beanfun_service::qr_login_complete(&state.http_client, &session_key, &state.cookie_jar)
                .await
                .map_err(login_err_to_dto)?;

        let accounts =
            beanfun_service::get_game_accounts(&state.http_client, &session, &state.cookie_jar)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("failed to fetch game accounts after QR login: {e}");
                    Vec::new()
                });

        let dto = SessionDto::from(&session);
        *state.session.write().await = Some(session);
        *state.game_accounts.write().await = accounts;
        tracing::info!("user logged in via QR: {}", dto.account_name);

        return Ok(QrPollResult {
            status: beanfun_service::QrPollStatus::Confirmed,
            session: Some(state.session.read().await.clone().unwrap()),
        });
    }

    Ok(result)
}

/// Verify a TOTP code (HK region).
///
/// Reads the partial session (with TOTP state) stored during login,
/// then calls the session-based TOTP verification.
#[tauri::command]
pub async fn totp_verify(code: String, state: State<'_, AppState>) -> Result<SessionDto, ErrorDto> {
    auth::validate_input("code", &code).map_err(to_dto)?;

    let partial_session = {
        let session_guard = state.session.read().await;
        session_guard
            .clone()
            .ok_or(AuthError::NotAuthenticated)
            .map_err(to_dto)?
    };

    // Use the session-based TOTP verification that has access to totp_state
    let session =
        beanfun_service::hk_totp_verify_with_session(&state.http_client, &code, &partial_session)
            .await
            .map_err(login_err_to_dto)?;

    let dto = SessionDto::from(&session);

    let accounts =
        beanfun_service::get_game_accounts(&state.http_client, &session, &state.cookie_jar)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("failed to fetch game accounts after TOTP: {e}");
                Vec::new()
            });

    *state.session.write().await = Some(session);
    *state.game_accounts.write().await = accounts;

    tracing::info!("user verified TOTP: {}", dto.account_name);
    Ok(dto)
}

/// Fetch the advance check (verification) page for TW login.
///
/// Returns the form state including captcha image as base64.
#[tauri::command]
pub async fn get_advance_check(
    url: Option<String>,
    state: State<'_, AppState>,
) -> Result<beanfun_service::AdvanceCheckState, ErrorDto> {
    let check_state = beanfun_service::get_advance_check_page(&state.http_client, url.as_deref())
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
    viewstate: String,
    viewstate_generator: String,
    event_validation: String,
    samplecaptcha: String,
    submit_url: String,
    verify_code: String,
    captcha_code: String,
    state: State<'_, AppState>,
) -> Result<bool, ErrorDto> {
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
        &state.http_client,
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
    samplecaptcha: String,
    state: State<'_, AppState>,
) -> Result<String, ErrorDto> {
    let image = beanfun_service::refresh_advance_check_captcha(&state.http_client, &samplecaptcha)
        .await
        .map_err(login_err_to_dto)?;

    Ok(image)
}

/// Log out — clear all in-memory credentials (Req 1.8, 13.3).
#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), ErrorDto> {
    // Call beanfun logout endpoint to invalidate server-side session
    let region = state.config.read().await.region.clone();
    let _ = beanfun_service::logout(&state.http_client, &region).await;

    state.clear_credentials().await;
    tracing::info!("user logged out, credentials cleared");
    Ok(())
}

/// Refresh the current session if it's about to expire.
///
/// Called internally or from the frontend heartbeat. Not exposed as a
/// primary user action — it's automatic (Req 1.4).
#[tauri::command]
pub async fn refresh_session(state: State<'_, AppState>) -> Result<SessionDto, ErrorDto> {
    let action = {
        let session_guard = state.session.read().await;
        auth::decide_session_action(&session_guard)
    };

    match action {
        SessionAction::UseExisting => {
            let session_guard = state.session.read().await;
            let session = auth::require_valid_session(&session_guard).map_err(to_dto)?;
            Ok(SessionDto::from(session))
        }
        SessionAction::AttemptRefresh => {
            let (refresh_token, region) = {
                let session_guard = state.session.read().await;
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
                beanfun_service::refresh_session(&state.http_client, &refresh_token, &region)
                    .await
                    .map_err(login_err_to_dto)?;

            let dto = SessionDto::from(&new_session);
            *state.session.write().await = Some(new_session);
            tracing::info!("session refreshed for {}", dto.account_name);
            Ok(dto)
        }
        SessionAction::ReAuthenticate => Err(to_dto(AuthError::SessionExpired)),
    }
}

// ---------------------------------------------------------------------------
// Saved account commands
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

// ---------------------------------------------------------------------------
// GamePass login commands (TW region)
// ---------------------------------------------------------------------------

/// User-Agent for WebView2 windows and HTTP requests.
const WEBVIEW_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

/// Open GamePass login popup for TW region.
///
/// The entire login flow happens inside the WebView2:
/// 1. Navigate to `bflogin/default.aspx` → server redirects to `Login/Index?pSKey={skey}`
/// 2. Init script auto-clicks `a.use-gama-pass` on the login page
/// 3. User completes GamePass OAuth in the webview
/// 4. Init script polls `echo_token.ashx` until session is ready
/// 5. Fetches account list HTML inside the webview, then signals backend
#[tauri::command]
pub async fn open_gamepass_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
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

    let init_script = r#"
    (() => {
        const url = window.location.href;
        const onBeanfun = url.includes('beanfun.com');
        const onLoginPage = url.includes('Login/Index');
        const onGamania = url.includes('accounts.gamania.com');
        const onDefaultAspx = url.includes('bflogin/default.aspx');

        Object.defineProperty(navigator, 'webdriver', {get: () => false});

        // Skip: initial redirect page, gamania OAuth, non-beanfun pages
        if (onDefaultAspx || onGamania || !onBeanfun) return;

        const _origFetch = window.fetch;

        // On login page: auto-click GamePass button
        if (onLoginPage) {
            function tryClick() {
                const btn = document.querySelector('a.use-gama-pass');
                if (btn) { btn.click(); console.log('[GamePass] clicked use-gama-pass'); return true; }
                return false;
            }
            // Try immediately
            if (!tryClick()) {
                // Retry with MutationObserver + polling fallback
                const obs = new MutationObserver(() => { if (tryClick()) obs.disconnect(); });
                if (document.body) {
                    obs.observe(document.body, { childList: true, subtree: true });
                }
                // Also poll every 200ms for up to 10s as fallback
                let attempts = 0;
                const poller = setInterval(() => {
                    attempts++;
                    if (tryClick() || attempts > 50) {
                        clearInterval(poller);
                        obs.disconnect();
                    }
                }, 200);
            }
            return;
        }

        // On beanfun.com post-login pages (return.aspx, index.aspx, etc)
        // Poll echo_token, then fetch accounts, then signal backend
        console.log('[GamePass] post-login page detected:', url);

        (async function() {
            // Poll echo_token until session is confirmed
            let ready = false;
            for (let i = 0; i < 60; i++) {
                try {
                    const r = await _origFetch(
                        'https://tw.beanfun.com/beanfun_block/generic_handlers/echo_token.ashx?webtoken=1',
                        { credentials: 'include' }
                    );
                    const t = await r.text();
                    if (t.includes('ResultCode:1')) {
                        console.log('[GamePass] session ready at attempt', i);
                        ready = true;
                        break;
                    }
                } catch(e) {}
                await new Promise(r => setTimeout(r, 500));
            }

            if (!ready) {
                console.log('[GamePass] session not ready, skipping (might be pre-login page)');
                return;
            }

            // Session is ready — fetch account list
            console.log('[GamePass] fetching account list...');
            let accountHtml = '';
            try {
                const sc = '610074', sr = 'T9';
                await _origFetch(
                    'https://tw.beanfun.com/beanfun_block/auth.aspx?channel=game_zone'
                    + '&page_and_query=game_start.aspx%3Fservice_code_and_region%3D' + sc + '_' + sr
                    + '&web_token=1',
                    { credentials: 'include' }
                );
                const listResp = await _origFetch(
                    'https://tw.beanfun.com/beanfun_block/game_zone/game_server_account_list.aspx'
                    + '?sc=' + sc + '&sr=' + sr + '&dt=' + Date.now(),
                    { credentials: 'include' }
                );
                accountHtml = await listResp.text();
                console.log('[GamePass] account list length:', accountHtml.length);
            } catch(e) {
                console.error('[GamePass] account list fetch failed:', e);
            }

            const cookies = document.cookie;
            let webToken = 'cookie_auth';
            cookies.split(';').forEach(c => {
                const t = c.trim();
                if (t.startsWith('bfWebToken=')) webToken = t.substring(11);
            });

            if (window.__TAURI_INTERNALS__) {
                console.log('[GamePass] invoking backend...');
                window.__TAURI_INTERNALS__.invoke('gamepass_webview_done', {
                    webToken: webToken,
                    cookies: cookies,
                    accountHtml: accountHtml
                }).then(() => console.log('[GamePass] SUCCESS'))
                  .catch(e => console.error('[GamePass] FAILED', e));
            }
        })();
    })();
    "#;

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
    .initialization_script(init_script)
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
    Ok(())
}

/// Called by the GamePass webview init script when login completes.
///
/// Uses WebView2 CookieManager to extract ALL
/// cookies (including HttpOnly), injects them into the reqwest cookie jar,
/// then fetches game accounts via reqwest.
#[tauri::command]
pub async fn gamepass_webview_done(
    web_token: String,
    _cookies: String,
    account_html: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    use tauri::Emitter;

    tracing::info!("=== GamePass webview_done START ===");

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

    // Step 2: Inject ALL cookies into reqwest jar
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
        state.cookie_jar.add_cookie_str(&cookie_str, jar_url);
    }

    tracing::info!("GamePass: injected all cookies into reqwest jar");

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
        &state.http_client,
        &session,
        &state.cookie_jar,
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("GamePass: reqwest get_game_accounts failed: {e}, trying webview HTML");
        crate::services::beanfun_service::parse_tw_account_list_html(&account_html)
    });

    tracing::info!("GamePass: got {} accounts", accounts.len());

    let dto = SessionDto::from(&session);
    *state.session.write().await = Some(session);
    *state.game_accounts.write().await = accounts;

    let _ = app.emit("gamepass-login-complete", dto);

    if let Some(win) = app.get_webview_window("gamepass-login") {
        let _ = win.destroy();
    }

    tracing::info!("=== GamePass webview_done FINISHED ===");
    Ok(())
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
