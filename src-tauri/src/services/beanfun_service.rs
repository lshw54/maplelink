//! HTTP client layer for the Beanfun platform.
//!
//! Implements the real HK login flow (regular + TOTP), game account retrieval,
//! and OTP credential fetching based on the original Beanfun client.
//! TW region flows are kept as placeholders.
//!
//! All network I/O lives here — the rest of the app calls these functions
//! through [`crate::commands`] handlers.

use regex::Regex;
use reqwest::cookie::CookieStore;
use reqwest::Client;

use crate::core::error::{AuthError, NetworkError};
use crate::models::game_account::{GameAccount, GameCredentials};
use crate::models::session::{Region, Session, TotpState};
use crate::utils::crypto::des_ecb_decrypt_hex;

/// User-Agent matching the original Beanfun client.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36";

/// Magic constant used in the OTP retrieval request.
const OTP_PPPPP: &str = "1F552AEAFF976018F942B13690C990F60ED01510DDF89165F1658CCE7BC21DBA";

/// Default service code for MapleStory.
const DEFAULT_SERVICE_CODE: &str = "610074";

/// Default service region for MapleStory HK.
const DEFAULT_SERVICE_REGION: &str = "T9";

/// Data returned when initiating a QR-code login flow (TW region).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrCodeData {
    pub session_key: String,
    pub qr_image_url: String,
    /// Cached `__RequestVerificationToken` from the login page.
    /// Used for subsequent `CheckLoginStatus` POST requests.
    pub verification_token: String,
    /// Beanfun app deeplink URL for mobile QR scanning.
    pub deeplink: String,
}

/// Polling result for an in-progress QR-code login.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QrPollResult {
    pub status: QrPollStatus,
    pub session: Option<Session>,
}

/// Status of a QR-code login poll.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QrPollStatus {
    Pending,
    Scanned,
    Confirmed,
    Expired,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Authenticate with username + password against the Beanfun platform.
///
/// For HK region, implements the full session-key → login → redirect flow.
/// If TOTP is required, returns a partial [`Session`] with `totp_state` set
/// and an `AuthError::TotpRequired` wrapped in `LoginError`.
///
/// For TW region, returns a placeholder error (not yet implemented).
pub async fn login(
    client: &Client,
    account: &str,
    password: &str,
    region: &Region,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Session, LoginError> {
    match region {
        Region::HK => hk_login(client, account, password).await,
        Region::TW => tw_login(client, account, password, cookie_jar).await,
    }
}

/// Start a QR-code login flow (TW region only).
///
/// Gets a session key, then fetches the QR code image from the TW login API.
pub async fn qr_login_start(client: &Client, region: &Region) -> Result<QrCodeData, LoginError> {
    match region {
        Region::TW => tw_qr_start(client).await,
        Region::HK => Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "QR login is only available for TW region".into(),
        })),
    }
}

/// Poll the status of an in-progress QR-code login.
pub async fn qr_login_poll(
    client: &Client,
    session_key: &str,
    verification_token: &str,
    region: &Region,
) -> Result<QrPollResult, LoginError> {
    match region {
        Region::TW => tw_qr_poll(client, session_key, verification_token).await,
        Region::HK => Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "QR login poll is only available for TW region".into(),
        })),
    }
}

/// Complete QR login after poll returns confirmed.
pub async fn qr_login_complete(
    client: &Client,
    session_key: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Session, LoginError> {
    tw_qr_complete(client, session_key, cookie_jar).await
}

/// Verify a TOTP code for HK region login.
///
/// Uses the saved `TotpState` from the partial session (stored after login
/// returned `need_totp`) to submit the 6-digit TOTP code and complete
/// authentication.
pub async fn totp_verify(
    client: &Client,
    code: &str,
    token: &str,
    region: &Region,
) -> Result<Session, LoginError> {
    match region {
        Region::HK => hk_totp_verify(client, code, token).await,
        Region::TW => Err(LoginError::Auth(AuthError::TotpFailed)),
    }
}

/// Attempt to refresh an existing session. Placeholder.
pub async fn refresh_session(
    client: &Client,
    refresh_token: &str,
    region: &Region,
) -> Result<Session, LoginError> {
    let _ = (client, refresh_token, region);
    Err(LoginError::Auth(AuthError::SessionExpired))
}

/// Log out from the Beanfun platform (invalidate server-side session).
pub async fn logout(client: &Client, region: &Region) -> Result<(), LoginError> {
    let (host, login_host) = match region {
        Region::HK => ("bfweb.hk.beanfun.com", "login.hk.beanfun.com"),
        Region::TW => ("tw.beanfun.com", "tw.newlogin.beanfun.com"),
    };

    let _ = http_get_text(
        client,
        &format!("https://{host}/generic_handlers/remove_bflogin_session.ashx"),
    )
    .await;

    let _ = http_get_text(
        client,
        &format!("https://{login_host}/logout.aspx?service=999999_T0"),
    )
    .await;

    // TW requires an extra erase_token step
    if *region == Region::TW {
        let erase_url = format!("https://{login_host}/generic_handlers/erase_token.ashx");
        let _ = client
            .post(&erase_url)
            .header("User-Agent", USER_AGENT)
            .form(&[("web_token", "1")])
            .send()
            .await;
    }

    tracing::info!("beanfun logout completed for {:?}", region);
    Ok(())
}

/// Retrieve the list of game accounts for an authenticated session.
///
/// For HK region, authenticates via `auth.aspx`, then parses the account
/// list HTML page using regex.
pub async fn get_game_accounts(
    client: &Client,
    session: &Session,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Vec<GameAccount>, LoginError> {
    match session.region {
        Region::HK => hk_get_accounts(client, session, cookie_jar).await,
        Region::TW => tw_get_accounts(client, session, cookie_jar).await,
    }
}

/// Parse TW game accounts from raw account list HTML.
///
/// Used by GamePass login where the HTML is fetched inside the WebView2
/// (which has the full cookie session) and passed to the backend for parsing.
pub fn parse_tw_account_list_html(html: &str) -> Vec<GameAccount> {
    let re = match Regex::new(r#"onclick="([^"]*)"><div id="(\w+)" sn="(\d+)" name="([^"]+)""#) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut accounts = Vec::new();
    for caps in re.captures_iter(html) {
        let sid = caps.get(2).map_or("", |m| m.as_str()).to_string();
        let ssn = caps.get(3).map_or("", |m| m.as_str()).to_string();
        let sname = html_decode(caps.get(4).map_or("", |m| m.as_str()));

        if sid.is_empty() || ssn.is_empty() || sname.is_empty() {
            continue;
        }

        accounts.push(GameAccount {
            id: sid,
            display_name: sname,
            game_type: format!("{}_{}", DEFAULT_SERVICE_CODE, DEFAULT_SERVICE_REGION),
            sn: ssn,
            status: if caps.get(1).map_or("", |m| m.as_str()).is_empty() {
                "banned".to_string()
            } else {
                "normal".to_string()
            },
            created_at: String::new(),
        });
    }

    accounts.sort_by(|a, b| a.sn.cmp(&b.sn));
    tracing::info!(
        "parse_tw_account_list_html: found {} accounts",
        accounts.len()
    );
    accounts
}

/// Retrieve one-time game credentials (OTP) for a specific account.
///
/// For HK region, implements the full long-polling + DES decryption flow
/// matching the original GetOTP flow.
pub async fn get_game_credentials(
    client: &Client,
    session: &Session,
    account_id: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<GameCredentials, LoginError> {
    match session.region {
        Region::HK => hk_get_otp(client, session, account_id, cookie_jar).await,
        Region::TW => tw_get_otp(client, session, account_id, cookie_jar).await,
    }
}
/// Ping the beanfun server to keep the session alive.
/// Fire-and-forget: catches all errors, never triggers logout.
pub async fn ping(client: &Client, region: &Region) {
    let host = match region {
        Region::HK => "bfweb.hk",
        Region::TW => "tw",
    };
    let url = format!(
        "https://{host}.beanfun.com/beanfun_block/generic_handlers/echo_token.ashx?webtoken=1"
    );
    match http_get_text(client, &url).await {
        Ok(body) => {
            tracing::info!("session ping ({:?}): ok, body_len={}", region, body.len());
        }
        Err(e) => {
            tracing::warn!("session ping ({:?}): failed: {e}", region);
        }
    }
}

/// Retrieve the user's remaining Beanfun points.
///
/// GETs the `get_remain_point.ashx` endpoint and parses the JSON-like
/// response for the `RemainPoint` value. Returns `0` when the field is
/// absent or unparseable.
pub async fn get_remain_point(client: &Client, region: &Region) -> Result<i32, LoginError> {
    let host = match region {
        Region::HK => "bfweb.hk",
        Region::TW => "tw",
    };
    let url = format!(
        "https://{host}.beanfun.com/beanfun_block/generic_handlers/get_remain_point.ashx?webtoken=1"
    );
    let body = http_get_text(client, &url).await?;

    let re = Regex::new(r#""RemainPoint"\s*:\s*"(\d+)""#)
        .map_err(|_| parse_error_str("failed to compile remain point regex"))?;

    let points = re
        .captures(&body)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .unwrap_or(0);

    tracing::debug!("remain points: {points}");
    Ok(points)
}
/// Change the display name of a game account.
///
/// POSTs to `gamezone.ashx` with `strFunction=ChangeServiceAccountDisplayName`.
/// Returns `true` if the server responds with `intResult: 1`.
pub async fn change_display_name(
    client: &Client,
    region: &Region,
    game_code: &str,
    account_id: &str,
    new_name: &str,
) -> Result<bool, LoginError> {
    // Only TW region has a server-side rename API
    if *region != Region::TW {
        // HK: no API, return false so caller saves locally
        return Ok(false);
    }

    let url = "https://tw.beanfun.com/generic_handlers/gamezone.ashx";

    let form = [
        ("strFunction", "ChangeServiceAccountDisplayName"),
        ("sl", game_code),
        ("said", account_id),
        ("nsadn", new_name),
    ];

    let resp = client
        .post(url)
        .header("User-Agent", USER_AGENT)
        .form(&form)
        .send()
        .await
        .map_err(|e| map_reqwest_error(url, e))?;

    let body = resp.text().await.map_err(|e| map_reqwest_error(url, e))?;

    tracing::debug!("change_display_name response: {body}");

    // Response is JSON: {"intResult": 1} on success
    let success = body.contains("\"intResult\":1") || body.contains("\"intResult\": 1");
    Ok(success)
}

/// Retrieve the authenticated user's email address.
///
/// - HK region: not supported, returns empty string.
/// - TW region: GETs the `loader.ashx` page and parses the email from
///   `BeanFunBlock.LoggedInUserData.Email = "...";`.
pub async fn get_email(client: &Client, region: &Region) -> Result<String, LoginError> {
    match region {
        Region::HK => Ok(String::new()),
        Region::TW => {
            let url = "https://tw.beanfun.com/beanfun_block/loader.ashx?service_code=999999&service_region=T0";

            let resp = client
                .get(url)
                .header("User-Agent", USER_AGENT)
                .header("Referer", "https://tw.beanfun.com/")
                .send()
                .await
                .map_err(|e| map_reqwest_error(url, e))?;

            let body = resp.text().await.map_err(|e| map_reqwest_error(url, e))?;

            let re = Regex::new(r#"BeanFunBlock\.LoggedInUserData\.Email\s*=\s*"([^"]+)""#)
                .map_err(|_| parse_error_str("failed to compile email regex"))?;

            let email = re
                .captures(&body)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            tracing::debug!("TW auth email: {email}");
            Ok(email)
        }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Unified error type for beanfun service operations.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Network(#[from] NetworkError),
}

// ---------------------------------------------------------------------------
// HK Login Implementation
// ---------------------------------------------------------------------------

/// Full HK regular login flow: GetSessionkey → HkRegularLogin → LoginCompleted.
async fn hk_login(client: &Client, account: &str, password: &str) -> Result<Session, LoginError> {
    // Step 1: Get session key
    let skey = hk_get_session_key(client).await?;
    tracing::debug!("HK session key obtained");

    // Step 2: Login form submission
    let login_url =
        format!("https://login.hk.beanfun.com/login/id-pass_form_newBF.aspx?otp1={skey}");

    let page_html = http_get_text(client, &login_url).await?;

    let viewstate = extract_html_field(&page_html, "__VIEWSTATE")?;
    let event_validation = extract_html_field(&page_html, "__EVENTVALIDATION")?;
    let viewstate_generator = extract_html_field(&page_html, "__VIEWSTATEGENERATOR")?;

    // Step 3: POST login form
    // Use the main client (with cookies from step 1) and let it follow redirects.
    // The final URL after redirect should contain `akey=`.
    let form = [
        ("__EVENTTARGET", ""),
        ("__EVENTARGUMENT", ""),
        ("__VIEWSTATE", viewstate.as_str()),
        ("__VIEWSTATEGENERATOR", viewstate_generator.as_str()),
        ("__VIEWSTATEENCRYPTED", ""),
        ("__EVENTVALIDATION", event_validation.as_str()),
        ("t_AccountID", account),
        ("t_Password", password),
        ("btn_login", "登入"),
    ];

    let resp = client
        .post(&login_url)
        .header("User-Agent", USER_AGENT)
        .form(&form)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&login_url, e))?;

    let final_url = resp.url().to_string();
    let response_body = resp.text().await.unwrap_or_default();

    tracing::debug!(
        "HK login POST final_url={}, body_len={}, body_preview={}",
        final_url,
        response_body.len(),
        &response_body[..response_body.len().min(500)]
    );

    // Step 4: Check response
    if response_body.contains("totpLoginBtn") {
        tracing::info!("HK login requires TOTP verification");
        let partial_session = Session {
            token: String::new(),
            refresh_token: None,
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            region: Region::HK,
            account_name: account.to_string(),
            session_key: Some(skey),
            totp_state: Some(TotpState {
                response_html: response_body,
                post_url: login_url,
            }),
        };
        return Err(LoginError::Auth(AuthError::TotpRequired {
            partial_session: Box::new(partial_session),
        }));
    }

    // Check for akey in final URL (after redirect) or response body
    let akey = extract_akey_from_url_or_body(&final_url, &response_body)?;

    // Step 5: LoginCompleted
    let web_token = hk_login_completed(client, &skey, &akey).await?;

    Ok(Session {
        token: web_token,
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: Region::HK,
        account_name: account.to_string(),
        session_key: Some(skey),
        totp_state: None,
    })
}

/// HK TOTP verification: extract viewstate from saved response, POST TOTP code.
async fn hk_totp_verify(
    _client: &Client,
    _code: &str,
    _token: &str,
) -> Result<Session, LoginError> {
    // The caller (auth.rs command) passes session.token as `_token`.
    // For TOTP, we need the totp_state from the session, but since the command
    // layer only passes token + region, we need to retrieve the full session
    // state. The command layer stores the partial session in AppState.
    // However, the function signature only gives us token/region.
    //
    // We work around this by encoding the TOTP state as JSON in the token field
    // when the partial session is created. This is decoded here.
    //
    // Actually, looking at auth.rs more carefully: totp_verify gets token from
    // session.token. We'll store serialized TotpContext in session.token for
    // the partial session case.
    //
    // Better approach: the partial session stored in AppState has totp_state.
    // But this function only receives token (String). We'll encode the needed
    // state as JSON in the token field.

    // For now, this function needs to be called with the full context.
    // The command layer passes session.token — we'll need to adjust.
    // Let's provide an alternative that takes the Session directly.
    Err(LoginError::Auth(AuthError::TotpFailed))
}

/// HK TOTP verification using the full session (with totp_state).
///
/// This is the real implementation called when we have access to the
/// complete partial session with TOTP state.
pub async fn hk_totp_verify_with_session(
    client: &Client,
    code: &str,
    partial_session: &Session,
) -> Result<Session, LoginError> {
    let totp_state = partial_session
        .totp_state
        .as_ref()
        .ok_or(LoginError::Auth(AuthError::TotpFailed))?;

    let skey = partial_session
        .session_key
        .as_ref()
        .ok_or(LoginError::Auth(AuthError::TotpFailed))?;

    let viewstate = extract_html_field(&totp_state.response_html, "__VIEWSTATE")?;
    let event_validation = extract_html_field(&totp_state.response_html, "__EVENTVALIDATION")?;
    let viewstate_generator =
        extract_html_field(&totp_state.response_html, "__VIEWSTATEGENERATOR")?;

    // Split the 6-digit code into individual digits
    let digits: Vec<char> = code.chars().collect();
    if digits.len() != 6 || !digits.iter().all(|c| c.is_ascii_digit()) {
        return Err(LoginError::Auth(AuthError::TotpFailed));
    }

    let d = |i: usize| -> String { digits[i].to_string() };

    // Use the main client (with cookies from login) and let it follow redirects.
    // The final URL after redirect should contain `akey=`.
    let form = [
        ("__EVENTTARGET", String::new()),
        ("__EVENTARGUMENT", String::new()),
        ("__VIEWSTATE", viewstate),
        ("__VIEWSTATEGENERATOR", viewstate_generator),
        ("__VIEWSTATEENCRYPTED", String::new()),
        ("__EVENTVALIDATION", event_validation),
        ("otpCode1", d(0)),
        ("otpCode2", d(1)),
        ("otpCode3", d(2)),
        ("otpCode4", d(3)),
        ("otpCode5", d(4)),
        ("otpCode6", d(5)),
        ("totpLoginBtn", "登入".to_string()),
    ];

    let resp = client
        .post(&totp_state.post_url)
        .header("User-Agent", USER_AGENT)
        .form(&form)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&totp_state.post_url, e))?;

    let final_url = resp.url().to_string();
    let response_body = resp.text().await.unwrap_or_default();

    tracing::trace!(
        "TOTP POST final_url={}, body_len={}, body_preview={}",
        final_url,
        response_body.len(),
        &response_body[..response_body.len().min(500)]
    );

    let akey = extract_akey_from_url_or_body(&final_url, &response_body)?;

    let web_token = hk_login_completed(client, skey, &akey).await?;

    Ok(Session {
        token: web_token,
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: Region::HK,
        account_name: partial_session.account_name.clone(),
        session_key: Some(skey.clone()),
        totp_state: None,
    })
}

// ---------------------------------------------------------------------------
// HK GetAccounts Implementation
// ---------------------------------------------------------------------------

/// Retrieve game accounts for HK region by parsing the account list HTML.
async fn hk_get_accounts(
    client: &Client,
    _session: &Session,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Vec<GameAccount>, LoginError> {
    let host = "bfweb.hk.beanfun.com";
    let sc = DEFAULT_SERVICE_CODE;
    let sr = DEFAULT_SERVICE_REGION;

    // Read bfWebToken from shared cookie jar
    let web_token = read_bf_web_token(cookie_jar, host);

    tracing::trace!(
        "bfWebToken from jar: '{}'",
        &web_token[..web_token.len().min(20)]
    );

    if web_token.is_empty() {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "no bfWebToken cookie found".into(),
        }));
    }

    // Step 1: Auth page with real web_token
    let auth_url = format!(
        "https://{host}/beanfun_block/auth.aspx?channel=game_zone\
         &page_and_query=game_start.aspx%3Fservice_code_and_region%3D{sc}_{sr}\
         &web_token={web_token}"
    );
    let auth_resp = http_get_text(client, &auth_url).await?;
    tracing::trace!(
        "auth.aspx response length={}, last 500: {}",
        auth_resp.len(),
        &auth_resp[auth_resp.len().saturating_sub(500)..]
    );

    // Step 2: Account list page
    let timestamp = get_current_time_method2();
    let list_url = format!(
        "https://{host}/beanfun_block/game_zone/game_server_account_list.aspx\
         ?sc={sc}&sr={sr}&dt={timestamp}"
    );
    let list_html = http_get_text(client, &list_url).await?;

    tracing::trace!(
        "HK account list HTML length={}, last 3000 chars:\n{}",
        list_html.len(),
        &list_html[list_html.len().saturating_sub(3000)..]
    );

    // Step 3: Parse accounts from HTML
    let re = Regex::new(r#"onclick="([^"]*)"><div id="(\w+)" sn="(\d+)" name="([^"]+)""#)
        .map_err(|_| parse_error_str("failed to compile account regex"))?;

    let mut accounts = Vec::new();
    for caps in re.captures_iter(&list_html) {
        let sid = caps.get(2).map_or("", |m| m.as_str()).to_string();
        let ssn = caps.get(3).map_or("", |m| m.as_str()).to_string();
        let sname = html_decode(caps.get(4).map_or("", |m| m.as_str()));

        if sid.is_empty() || ssn.is_empty() || sname.is_empty() {
            continue;
        }

        accounts.push(GameAccount {
            id: sid,
            display_name: sname,
            game_type: format!("{sc}_{sr}"),
            sn: ssn.clone(),
            status: if caps.get(1).map_or("", |m| m.as_str()).is_empty() {
                "banned".to_string()
            } else {
                "normal".to_string()
            },
            created_at: get_create_time(client, host, sc, sr, &ssn).await,
        });
    }

    // Sort by sn
    accounts.sort_by(|a, b| a.sn.cmp(&b.sn));

    tracing::info!("HK: found {} game accounts", accounts.len());
    Ok(accounts)
}

/// Fetch the creation time for a single service account.
async fn get_create_time(client: &Client, host: &str, sc: &str, sr: &str, sn: &str) -> String {
    let timestamp = get_current_time_method2();
    let url = format!(
        "https://{host}/beanfun_block/game_zone/game_start_step2.aspx\
         ?service_code={sc}&service_region={sr}&sotp={sn}&dt={timestamp}"
    );
    match http_get_text(client, &url).await {
        Ok(html) => {
            let re = Regex::new(r#"ServiceAccountCreateTime: "([^"]+)""#).ok();
            re.and_then(|r| r.captures(&html))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        }
        Err(_) => String::new(),
    }
}

// ---------------------------------------------------------------------------
// HK GetOTP Implementation
// ---------------------------------------------------------------------------

/// Retrieve OTP credentials for a specific game account (HK region).
///
/// Implements the full long-polling + DES decryption flow.
async fn hk_get_otp(
    client: &Client,
    session: &Session,
    account_id: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<GameCredentials, LoginError> {
    let host = "bfweb.hk.beanfun.com";
    let login_host = "login.hk.beanfun.com";
    let sc = DEFAULT_SERVICE_CODE;
    let sr = DEFAULT_SERVICE_REGION;

    // Read bfWebToken from cookie jar for OTP requests
    let web_token = read_bf_web_token(cookie_jar, host);

    let accounts = hk_get_accounts(client, session, cookie_jar).await?;
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| {
            LoginError::Auth(AuthError::InvalidCredentials {
                reason: format!("account {account_id} not found"),
            })
        })?;

    let ssn = &account.sn;
    let sname = &account.display_name;

    // Step 1: Get game_start_step2 page
    let timestamp = get_current_time_method2();
    let step2_url = format!(
        "https://{host}/beanfun_block/game_zone/game_start_step2.aspx\
         ?service_code={sc}&service_region={sr}&sotp={ssn}&dt={timestamp}"
    );
    let step2_html = http_get_text(client, &step2_url).await?;

    // Step 2: Extract long polling key
    let lp_re = Regex::new(r#"GetResultByLongPolling&key=(.*)""#)
        .map_err(|_| parse_error_str("failed to compile long polling regex"))?;
    let long_polling_key = lp_re
        .captures(&step2_html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| parse_error_str("no long polling key found"))?;

    // Step 3: Extract create time
    let create_time = if account.created_at.is_empty() {
        let ct_re = Regex::new(r#"ServiceAccountCreateTime: "([^"]+)""#)
            .map_err(|_| parse_error_str("failed to compile create time regex"))?;
        ct_re
            .captures(&step2_html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    } else {
        account.created_at.clone()
    };

    // Step 4: Get secret code
    let cookies_url = format!("https://{login_host}/generic_handlers/get_cookies.ashx");
    let cookies_html = http_get_text(client, &cookies_url).await?;

    let sc_re = Regex::new(r"var m_strSecretCode = '(.*)';")
        .map_err(|_| parse_error_str("failed to compile secret code regex"))?;
    let secret_code = sc_re
        .captures(&cookies_html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| parse_error_str("no secret code found"))?;

    // Step 5: Record service start
    let record_url =
        format!("https://{host}/beanfun_block/generic_handlers/record_service_start.ashx");
    let record_form = [
        ("service_code", sc),
        ("service_region", sr),
        ("service_account_id", account_id),
        ("sotp", ssn),
        ("service_account_display_name", sname),
        ("service_account_create_time", &create_time),
    ];
    let _ = client
        .post(&record_url)
        .header("User-Agent", USER_AGENT)
        .form(&record_form)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&record_url, e))?;

    // Step 6: Long polling
    let now_ts = get_current_time_default();
    let poll_url = format!(
        "https://{host}/generic_handlers/get_result.ashx\
         ?meth=GetResultByLongPolling&key={long_polling_key}&_={now_ts}"
    );
    let _ = http_get_text(client, &poll_url).await?;

    // Step 7: Get OTP
    let create_time_encoded = create_time.replace(' ', "%20");
    let tick_count = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let otp_url = format!(
        "https://{host}/beanfun_block/generic_handlers/get_webstart_otp.ashx\
         ?SN={long_polling_key}&WebToken={web_token}&SecretCode={secret_code}\
         &ppppp={OTP_PPPPP}&ServiceCode={sc}&ServiceRegion={sr}\
         &ServiceAccount={account_id}&CreateTime={create_time_encoded}\
         &d={tick_count}"
    );
    let otp_response = http_get_text(client, &otp_url).await?;

    // Step 8: Parse response "{status};{data}"
    let parts: Vec<&str> = otp_response.splitn(2, ';').collect();
    if parts.len() < 2 {
        return Err(parse_error_str("OTP response format invalid"));
    }
    if parts[0] != "1" {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: format!("OTP retrieval failed: {}", parts.get(1).unwrap_or(&"")),
        }));
    }

    let data = parts[1];
    if data.len() < 8 {
        return Err(parse_error_str("OTP data too short for DES key"));
    }

    // Step 9: DES decrypt
    let des_key = &data[..8];
    let encrypted = &data[8..];
    let otp = des_ecb_decrypt_hex(encrypted, des_key).ok_or_else(|| {
        LoginError::Auth(AuthError::InvalidCredentials {
            reason: "OTP decryption failed".into(),
        })
    })?;

    tracing::info!(account_id = %account_id, "OTP retrieved successfully");

    Ok(GameCredentials {
        account_id: account_id.to_string(),
        otp,
        retrieved_at: chrono::Utc::now(),
        command_line_template: Some(
            "tw.login.maplestory.beanfun.com 8484 BeanFun %s %s".to_string(),
        ),
    })
}

// ---------------------------------------------------------------------------
// HK Shared Helpers
// ---------------------------------------------------------------------------

/// Get the HK session key by parsing the OTP span from the default login page.
async fn hk_get_session_key(client: &Client) -> Result<String, LoginError> {
    let url = "https://bfweb.hk.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";
    let html = http_get_text(client, url).await?;

    tracing::debug!(
        "hk_get_session_key response length={}, first 500 chars: {}",
        html.len(),
        &html[..html.len().min(500)]
    );

    let re = Regex::new(r#"<span id="ctl00_ContentPlaceHolder1_lblOtp1">(.*)</span>"#)
        .map_err(|_| parse_error_str("failed to compile session key regex"))?;

    re.captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            tracing::error!("no OTP1 span found in response. Full HTML:\n{html}");
            LoginError::Auth(AuthError::InvalidCredentials {
                reason: "failed to extract session key (no OTP1 span)".into(),
            })
        })
}

/// Complete HK login by POSTing session key + auth key to return.aspx,
/// then following the redirect to extract the `bfWebToken` cookie.
async fn hk_login_completed(client: &Client, skey: &str, akey: &str) -> Result<String, LoginError> {
    let host = "bfweb.hk.beanfun.com";
    let return_url = format!("https://{host}/beanfun_block/bflogin/return.aspx");

    tracing::debug!("LoginCompleted: posting SessionKey + AuthKey to return.aspx");

    // POST using main client. The server will set bfWebToken cookie via Set-Cookie.
    let form = [
        ("SessionKey", skey),
        ("AuthKey", akey),
        ("ServiceCode", ""),
        ("ServiceRegion", ""),
        ("ServiceAccountSN", "0"),
    ];

    let resp = client
        .post(&return_url)
        .header("User-Agent", USER_AGENT)
        .form(&form)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&return_url, e))?;

    let final_url = resp.url().to_string();
    tracing::trace!("return.aspx final_url={final_url}");

    // Read the response body (triggers cookie storage in reqwest jar)
    let _body = resp.text().await.unwrap_or_default();

    // Now verify login succeeded via echo_token endpoint
    let token_url =
        format!("https://{host}/beanfun_block/generic_handlers/echo_token.ashx?webtoken=1");
    let token_resp = http_get_text(client, &token_url).await?;

    tracing::trace!(
        "echo_token raw response: '{}'",
        &token_resp[..token_resp.len().min(200)]
    );

    // Check that login was successful (ResultCode:1 means logged in)
    if !token_resp.contains("ResultCode:1") {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "login verification failed: not logged in".into(),
        }));
    }

    // The actual bfWebToken is stored as a cookie in the reqwest cookie jar.
    // We don't need to extract it explicitly — reqwest will send it automatically
    // with all subsequent requests to bfweb.hk.beanfun.com.
    // We store a marker token so the session is considered valid.
    let web_token = "cookie_auth".to_string();

    if web_token.is_empty() || web_token.contains("<!DOCTYPE") || web_token.contains("<html") {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "failed to obtain bfWebToken after login".into(),
        }));
    }

    tracing::info!("HK login completed, web token obtained");
    Ok(web_token)
}

// ---------------------------------------------------------------------------
// TW Login Implementation
// ---------------------------------------------------------------------------

/// Get TW session key by following the redirect from bflogin/default.aspx.
async fn tw_get_session_key(client: &Client) -> Result<String, LoginError> {
    let url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";

    let resp = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| map_reqwest_error(url, e))?;

    let final_url = resp.url().to_string();
    tracing::debug!("TW session key redirect URL: {final_url}");

    // Extract pSKey or SessionKey from the final URL
    let re = Regex::new(r"[pP]?[sS][Kk]ey=([^&]+)")
        .map_err(|_| parse_error_str("failed to compile skey regex"))?;

    re.captures(&final_url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            tracing::error!("no pSKey found in TW redirect URL: {final_url}");
            LoginError::Auth(AuthError::InvalidCredentials {
                reason: "failed to extract TW session key".into(),
            })
        })
}

/// Full TW regular login flow using the new JSON API.
async fn tw_login(
    client: &Client,
    account: &str,
    password: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Session, LoginError> {
    let skey = tw_get_session_key(client).await?;
    tracing::debug!("TW session key: {}", &skey[..skey.len().min(20)]);

    let api_base = "https://login.beanfun.com";
    let index_url = format!("{api_base}/Login/Index?pSKey={skey}");

    // Step 1: Get index page and __RequestVerificationToken
    let index_html = http_get_text(client, &index_url).await?;
    let form_token = extract_request_verification_token(&index_html)?;
    tracing::debug!("TW form token obtained");

    // Step 2: CheckAccountType
    let check_url = format!("{api_base}/Login/CheckAccountType?pSKey={skey}");
    let check_body = serde_json::json!({
        "Account": account,
        "Captcha": "",
        "__RequestVerificationToken": form_token,
    });

    let check_resp = client
        .post(&check_url)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "application/json; charset=utf-8")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("RequestVerificationToken", &form_token)
        .header("Referer", &index_url)
        .header("Origin", api_base)
        .json(&check_body)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&check_url, e))?;

    let check_text = check_resp.text().await.unwrap_or_default();
    let captcha_token = serde_json::from_str::<serde_json::Value>(&check_text)
        .ok()
        .and_then(|j| j["ResultData"]["Captcha"].as_str().map(String::from))
        .unwrap_or_default();

    // Step 3: AccountLogin
    let login_url = format!("{api_base}/Login/AccountLogin?pSKey={skey}");
    let login_body = serde_json::json!({
        "Account": account,
        "Pasw": password,
        "IsMobile": false,
        "Captcha": captcha_token,
        "__RequestVerificationToken": form_token,
    });

    let login_resp = client
        .post(&login_url)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "application/json; charset=utf-8")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("RequestVerificationToken", &form_token)
        .header("Referer", &index_url)
        .header("Origin", api_base)
        .json(&login_body)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&login_url, e))?;

    let login_text = login_resp.text().await.unwrap_or_default();
    tracing::debug!(
        "TW AccountLogin response: {}",
        &login_text[..login_text.len().min(500)]
    );

    let login_json: serde_json::Value = serde_json::from_str(&login_text)
        .map_err(|_| parse_error_str("failed to parse AccountLogin response"))?;

    let result_code = login_json["ResultCode"].as_i64().unwrap_or(-1);
    let result = login_json["Result"].as_i64().unwrap_or(-1);
    let result_msg = login_json["ResultMessage"].as_str().unwrap_or("");

    match result_code {
        1 => {
            if result == 1 {
                // AdvanceCheck required (no URL)
                return Err(LoginError::Auth(AuthError::AdvanceCheckRequired {
                    url: None,
                }));
            }
            // Success — complete via SendLogin flow
            let web_token = tw_send_login_flow(client, &skey, cookie_jar).await?;
            Ok(Session {
                token: web_token,
                refresh_token: None,
                expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
                region: Region::TW,
                account_name: account.to_string(),
                session_key: Some(skey),
                totp_state: None,
            })
        }
        2 => {
            // AdvanceCheck with URL
            let url = if result_msg.starts_with("http") {
                Some(result_msg.to_string())
            } else {
                None
            };
            Err(LoginError::Auth(AuthError::AdvanceCheckRequired { url }))
        }
        _ => {
            let msg = if result_msg.is_empty() {
                "TW login failed".to_string()
            } else {
                result_msg.to_string()
            };
            Err(LoginError::Auth(AuthError::InvalidCredentials {
                reason: msg,
            }))
        }
    }
}

/// TW SendLogin flow: GET SendLogin page → parse form → POST return.aspx → extract bfWebToken.
///
/// This is the shared completion step used by both regular login and QR login.
async fn tw_send_login_flow(
    client: &Client,
    skey: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<String, LoginError> {
    let api_base = "https://login.beanfun.com";
    let index_url = format!("{api_base}/Login/Index?pSKey={skey}");

    // Step 4a: GET SendLogin page
    let send_login_url = format!("{api_base}/Login/SendLogin");
    let send_login_html = client
        .get(&send_login_url)
        .header("User-Agent", USER_AGENT)
        .header("Referer", &index_url)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|e| map_reqwest_error(&send_login_url, e))?
        .text()
        .await
        .unwrap_or_default();

    tracing::debug!(
        "TW SendLogin HTML length={}, preview={}",
        send_login_html.len(),
        &send_login_html[..send_login_html.len().min(500)]
    );

    // Parse hidden form fields (exclude type="submit" inputs)
    let input_re = Regex::new(r#"<input[^>]+>"#)
        .map_err(|_| parse_error_str("failed to compile input regex"))?;
    let name_re = Regex::new(r#"name\s*=\s*['"]([^'"]+)['"]"#)
        .map_err(|_| parse_error_str("failed to compile name regex"))?;
    let value_re = Regex::new(r#"value\s*=\s*['"]([^'"]*)['"]"#)
        .map_err(|_| parse_error_str("failed to compile value regex"))?;

    let mut form_fields: Vec<(String, String)> = Vec::new();
    for cap in input_re.captures_iter(&send_login_html) {
        let tag = cap.get(0).map_or("", |m| m.as_str());
        // Skip submit buttons
        if tag.contains("type=\"submit\"") || tag.contains("type='submit'") {
            continue;
        }
        if let (Some(name_cap), Some(val_cap)) = (name_re.captures(tag), value_re.captures(tag)) {
            let name = name_cap.get(1).map_or("", |m| m.as_str()).to_string();
            let val = val_cap.get(1).map_or("", |m| m.as_str()).to_string();
            if !name.is_empty() {
                form_fields.push((name, val));
            }
        }
    }

    if form_fields.is_empty() {
        return Err(parse_error_str("no form fields found in SendLogin page"));
    }

    tracing::debug!("TW SendLogin form fields: {}", form_fields.len());

    // Step 4b: POST to return.aspx WITHOUT following redirects.
    // The C# code sets redirect=false and reads bfWebToken from Set-Cookie.
    // We build a temporary no-redirect client and manually forward cookies.
    let return_url = "https://tw.beanfun.com/beanfun_block/bflogin/return.aspx";

    // Encode form body
    let form_body: String = form_fields
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // Collect cookies from the shared cookie jar
    let tw_url: url::Url = "https://tw.beanfun.com/".parse().unwrap();
    let login_url_parsed: url::Url = "https://login.beanfun.com/".parse().unwrap();

    let mut cookie_header = String::new();
    if let Some(cookies) = cookie_jar.cookies(&tw_url) {
        if let Ok(s) = cookies.to_str() {
            cookie_header = s.to_string();
        }
    }
    if let Some(cookies) = cookie_jar.cookies(&login_url_parsed) {
        if let Ok(s) = cookies.to_str() {
            if !cookie_header.is_empty() {
                cookie_header.push_str("; ");
            }
            cookie_header.push_str(s);
        }
    }

    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| parse_error_str(&format!("failed to build no-redirect client: {e}")))?;

    let mut req = no_redirect_client
        .post(return_url)
        .header("User-Agent", USER_AGENT)
        .header("Referer", &format!("{api_base}/"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body);

    if !cookie_header.is_empty() {
        req = req.header("Cookie", &cookie_header);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| map_reqwest_error(return_url, e))?;

    // Extract bfWebToken from Set-Cookie header (no redirect, so we see it directly)
    let mut web_token = String::new();
    for value in resp.headers().get_all("set-cookie") {
        if let Ok(s) = value.to_str() {
            if let Some(token) = s
                .split(';')
                .next()
                .and_then(|part| part.trim().strip_prefix("bfWebToken="))
            {
                web_token = token.to_string();
                tracing::info!(
                    "extracted bfWebToken from Set-Cookie: len={}",
                    web_token.len()
                );
                break;
            }
        }
    }

    // Also store the Set-Cookie values back into the shared cookie jar
    // so subsequent requests (get_game_accounts, etc.) have the token.
    for value in resp.headers().get_all("set-cookie") {
        if let Ok(s) = value.to_str() {
            cookie_jar.add_cookie_str(s, &tw_url);
        }
    }

    let _body = resp.text().await.unwrap_or_default();

    if web_token.is_empty() {
        // Fallback: try echo_token to verify login succeeded
        let token_url =
            "https://tw.beanfun.com/beanfun_block/generic_handlers/echo_token.ashx?webtoken=1";
        let token_resp = http_get_text(client, token_url).await?;

        tracing::trace!(
            "TW echo_token: {}",
            &token_resp[..token_resp.len().min(200)]
        );

        if !token_resp.contains("ResultCode:1") {
            return Err(LoginError::Auth(AuthError::InvalidCredentials {
                reason: "TW login verification failed (no bfWebToken)".into(),
            }));
        }

        // Login succeeded via cookie jar, use placeholder
        web_token = "cookie_auth".to_string();
    }

    Ok(web_token)
}

/// Start TW QR code login flow.
async fn tw_qr_start(client: &Client) -> Result<QrCodeData, LoginError> {
    let skey = tw_get_session_key(client).await?;
    let api_base = "https://login.beanfun.com";
    let index_url = format!("{api_base}/Login/Index?pSKey={skey}");

    // Load index page first (sets cookies) and extract __RequestVerificationToken
    let index_html = http_get_text(client, &index_url).await?;

    // Extract __RequestVerificationToken from the login page HTML.
    // The token is in: <input name="__RequestVerificationToken" ... value="TOKEN" />
    let verification_token = extract_request_verification_token(&index_html).unwrap_or_else(|e| {
        tracing::warn!("failed to extract __RequestVerificationToken: {e}, using empty");
        String::new()
    });

    // Get QR code data via InitLogin
    let init_url = format!("{api_base}/Login/InitLogin?pSKey={skey}");
    let init_resp = client
        .get(&init_url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json, text/plain, */*")
        .header("Referer", &index_url)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Origin", api_base)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&init_url, e))?;

    let init_text = init_resp.text().await.unwrap_or_default();
    let init_json: serde_json::Value = serde_json::from_str(&init_text)
        .map_err(|_| parse_error_str("failed to parse InitLogin response"))?;

    if init_json["Result"].as_i64() != Some(0) {
        return Err(parse_error_str("InitLogin returned non-zero result"));
    }

    let qr_image = init_json["ResultData"]["QRImage"]
        .as_str()
        .unwrap_or_default();

    let deeplink = init_json["ResultData"]["DeepLink"]
        .as_str()
        .or_else(|| init_json["ResultData"]["strUrl"].as_str())
        .unwrap_or_default()
        .to_string();

    tracing::debug!(
        "InitLogin ResultData keys: {:?}",
        init_json["ResultData"]
            .as_object()
            .map(|o| o.keys().collect::<Vec<_>>())
    );

    if qr_image.is_empty() {
        return Err(parse_error_str("no QR image in InitLogin response"));
    }

    let qr_image_url = format!("data:image/png;base64,{qr_image}");

    tracing::info!(
        "TW QR code obtained, skey={}, has_token={}",
        &skey[..skey.len().min(20)],
        !verification_token.is_empty()
    );

    Ok(QrCodeData {
        session_key: skey,
        qr_image_url,
        verification_token,
        deeplink,
    })
}

/// Poll TW QR code login status.
async fn tw_qr_poll(
    client: &Client,
    session_key: &str,
    verification_token: &str,
) -> Result<QrPollResult, LoginError> {
    let url = "https://login.beanfun.com/QRLogin/CheckLoginStatus";

    let resp = client
        .post(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json, text/plain, */*")
        .header(
            "Referer",
            &format!("https://login.beanfun.com/Login/Index?pSKey={session_key}"),
        )
        .header("Origin", "https://login.beanfun.com")
        .header("RequestVerificationToken", verification_token)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Content-Length", "0")
        .body("")
        .send()
        .await
        .map_err(|e| map_reqwest_error(url, e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    tracing::debug!(
        "QR poll response: status={}, len={}, body={}",
        status,
        text.len(),
        &text[..text.len().min(300)]
    );

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        LoginError::Network(NetworkError::HttpError {
            status: status.as_u16(),
            url: "failed to parse QR poll response".to_string(),
        })
    })?;

    let result_msg = json["ResultMessage"].as_str().unwrap_or("");

    match result_msg {
        "Failed" | "Wait Login" => Ok(QrPollResult {
            status: QrPollStatus::Pending,
            session: None,
        }),
        "Token Expired" => Ok(QrPollResult {
            status: QrPollStatus::Expired,
            session: None,
        }),
        "Success" => Ok(QrPollResult {
            status: QrPollStatus::Confirmed,
            session: None, // Session will be created by qr_login_complete
        }),
        _ => {
            tracing::warn!("unknown QR poll status: {result_msg}");
            Ok(QrPollResult {
                status: QrPollStatus::Pending,
                session: None,
            })
        }
    }
}

/// Complete TW QR login after poll returns confirmed.
async fn tw_qr_complete(
    client: &Client,
    session_key: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Session, LoginError> {
    let api_base = "https://login.beanfun.com";

    // Call QRLogin endpoint
    let qr_login_url = format!("{api_base}/QRLogin/QRLogin");
    let _ = client
        .get(&qr_login_url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json, text/plain, */*")
        .header(
            "Referer",
            &format!("{api_base}/Login/Index?pSKey={session_key}"),
        )
        .send()
        .await
        .map_err(|e| map_reqwest_error(&qr_login_url, e))?;

    // Complete via SendLogin flow
    let web_token = tw_send_login_flow(client, session_key, cookie_jar).await?;

    Ok(Session {
        token: web_token,
        refresh_token: None,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: Region::TW,
        account_name: "TW User".to_string(),
        session_key: Some(session_key.to_string()),
        totp_state: None,
    })
}

// ---------------------------------------------------------------------------
// TW GetAccounts / GetOTP Implementation
// ---------------------------------------------------------------------------

/// Retrieve game accounts for TW region.
async fn tw_get_accounts(
    client: &Client,
    _session: &Session,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Vec<GameAccount>, LoginError> {
    let host = "tw.beanfun.com";
    let sc = DEFAULT_SERVICE_CODE;
    let sr = DEFAULT_SERVICE_REGION;

    let web_token = read_bf_web_token(cookie_jar, host);
    if web_token.is_empty() {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "no bfWebToken cookie found for TW".into(),
        }));
    }

    // Auth page
    let auth_url = format!(
        "https://{host}/beanfun_block/auth.aspx?channel=game_zone\
         &page_and_query=game_start.aspx%3Fservice_code_and_region%3D{sc}_{sr}\
         &web_token={web_token}"
    );
    let _ = http_get_text(client, &auth_url).await?;

    // Account list page
    let timestamp = get_current_time_method2();
    let list_url = format!(
        "https://{host}/beanfun_block/game_zone/game_server_account_list.aspx\
         ?sc={sc}&sr={sr}&dt={timestamp}"
    );
    let list_html = http_get_text(client, &list_url).await?;

    // Parse accounts (same regex pattern as HK)
    let re = Regex::new(r#"onclick="([^"]*)"><div id="(\w+)" sn="(\d+)" name="([^"]+)""#)
        .map_err(|_| parse_error_str("failed to compile account regex"))?;

    let mut accounts = Vec::new();
    for caps in re.captures_iter(&list_html) {
        let sid = caps.get(2).map_or("", |m| m.as_str()).to_string();
        let ssn = caps.get(3).map_or("", |m| m.as_str()).to_string();
        let sname = html_decode(caps.get(4).map_or("", |m| m.as_str()));

        if sid.is_empty() || ssn.is_empty() || sname.is_empty() {
            continue;
        }

        accounts.push(GameAccount {
            id: sid,
            display_name: sname,
            game_type: format!("{sc}_{sr}"),
            sn: ssn.clone(),
            status: if caps.get(1).map_or("", |m| m.as_str()).is_empty() {
                "banned".to_string()
            } else {
                "normal".to_string()
            },
            created_at: get_create_time(client, host, sc, sr, &ssn).await,
        });
    }

    accounts.sort_by(|a, b| a.sn.cmp(&b.sn));
    tracing::info!("TW: found {} game accounts", accounts.len());
    Ok(accounts)
}

/// Retrieve OTP for TW region (same flow as HK but different host).
async fn tw_get_otp(
    client: &Client,
    session: &Session,
    account_id: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<GameCredentials, LoginError> {
    let host = "tw.beanfun.com";
    let login_host = "tw.newlogin.beanfun.com";
    let sc = DEFAULT_SERVICE_CODE;
    let sr = DEFAULT_SERVICE_REGION;

    let web_token = read_bf_web_token(cookie_jar, host);

    let accounts = tw_get_accounts(client, session, cookie_jar).await?;
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| {
            LoginError::Auth(AuthError::InvalidCredentials {
                reason: format!("account {account_id} not found"),
            })
        })?;

    let ssn = &account.sn;
    let sname = &account.display_name;

    // Step 1: game_start_step2
    let timestamp = get_current_time_method2();
    let step2_url = format!(
        "https://{host}/beanfun_block/game_zone/game_start_step2.aspx\
         ?service_code={sc}&service_region={sr}&sotp={ssn}&dt={timestamp}"
    );
    let step2_html = http_get_text(client, &step2_url).await?;

    // Step 2: Long polling key
    let lp_re = Regex::new(r#"GetResultByLongPolling&key=(.*)""#)
        .map_err(|_| parse_error_str("failed to compile long polling regex"))?;
    let long_polling_key = lp_re
        .captures(&step2_html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| parse_error_str("no long polling key found"))?;

    // Step 3: Create time
    let create_time = if account.created_at.is_empty() {
        let ct_re = Regex::new(r#"ServiceAccountCreateTime: "([^"]+)""#)
            .map_err(|_| parse_error_str("failed to compile create time regex"))?;
        ct_re
            .captures(&step2_html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    } else {
        account.created_at.clone()
    };

    // Step 4: Secret code
    let cookies_url = format!("https://{login_host}/generic_handlers/get_cookies.ashx");
    let cookies_html = http_get_text(client, &cookies_url).await?;
    let sc_re = Regex::new(r"var m_strSecretCode = '(.*)';")
        .map_err(|_| parse_error_str("failed to compile secret code regex"))?;
    let secret_code = sc_re
        .captures(&cookies_html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| parse_error_str("no secret code found"))?;

    // Step 5: Record service start
    let record_url =
        format!("https://{host}/beanfun_block/generic_handlers/record_service_start.ashx");
    let record_form = [
        ("service_code", sc),
        ("service_region", sr),
        ("service_account_id", account_id),
        ("sotp", ssn),
        ("service_account_display_name", sname),
        ("service_account_create_time", &create_time),
    ];
    let _ = client
        .post(&record_url)
        .header("User-Agent", USER_AGENT)
        .form(&record_form)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&record_url, e))?;

    // Step 6: Long polling
    let now_ts = get_current_time_default();
    let poll_url = format!(
        "https://{host}/generic_handlers/get_result.ashx\
         ?meth=GetResultByLongPolling&key={long_polling_key}&_={now_ts}"
    );
    let _ = http_get_text(client, &poll_url).await?;

    // Step 7: Get OTP
    let create_time_encoded = create_time.replace(' ', "%20");
    let tick_count = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let otp_url = format!(
        "https://{host}/beanfun_block/generic_handlers/get_webstart_otp.ashx\
         ?SN={long_polling_key}&WebToken={web_token}&SecretCode={secret_code}\
         &ppppp={OTP_PPPPP}&ServiceCode={sc}&ServiceRegion={sr}\
         &ServiceAccount={account_id}&CreateTime={create_time_encoded}\
         &d={tick_count}"
    );
    let otp_response = http_get_text(client, &otp_url).await?;

    // Step 8: Parse
    let parts: Vec<&str> = otp_response.splitn(2, ';').collect();
    if parts.len() < 2 {
        return Err(parse_error_str("OTP response format invalid"));
    }
    if parts[0] != "1" {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: format!("OTP retrieval failed: {}", parts.get(1).unwrap_or(&"")),
        }));
    }

    let data = parts[1];
    if data.len() < 8 {
        return Err(parse_error_str("OTP data too short for DES key"));
    }

    // Step 9: DES decrypt
    let des_key = &data[..8];
    let encrypted = &data[8..];
    let otp = des_ecb_decrypt_hex(encrypted, des_key).ok_or_else(|| {
        LoginError::Auth(AuthError::InvalidCredentials {
            reason: "OTP decryption failed".into(),
        })
    })?;

    tracing::info!(account_id = %account_id, "TW OTP retrieved successfully");

    Ok(GameCredentials {
        account_id: account_id.to_string(),
        otp,
        retrieved_at: chrono::Utc::now(),
        command_line_template: Some(
            "tw.login.maplestory.beanfun.com 8484 BeanFun %s %s".to_string(),
        ),
    })
}

// ---------------------------------------------------------------------------
// TW Advance Check (Verify) Implementation
// ---------------------------------------------------------------------------

/// State for an in-progress advance check verification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvanceCheckState {
    pub viewstate: String,
    pub viewstate_generator: String,
    pub event_validation: String,
    pub samplecaptcha: String,
    pub submit_url: String,
    pub captcha_image_base64: String,
    /// Hint showing the user's masked auth info (e.g. "09XX-XXX-XX3")
    pub auth_hint: String,
}

/// Fetch the advance check page and parse its form fields + captcha.
pub async fn get_advance_check_page(
    client: &Client,
    url: Option<&str>,
) -> Result<AdvanceCheckState, LoginError> {
    let page_url = url.unwrap_or("https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx");
    let html = http_get_text(client, page_url).await?;

    tracing::trace!(
        "advance check HTML length={}, preview={}",
        html.len(),
        &html[..html.len().min(2000)]
    );

    // Log more of the HTML to find auth hint
    if html.len() > 2000 {
        tracing::trace!(
            "advance check HTML part2: {}",
            &html[2000..html.len().min(4000)]
        );
    }
    if html.len() > 4000 {
        tracing::trace!(
            "advance check HTML part3: {}",
            &html[4000..html.len().min(6000)]
        );
    }

    // Check if this is the new-style SPA verification page (no ASP.NET form fields)
    // The new page has title "遊戲橘子進階驗證" and uses a JS-based flow
    let is_new_style = !html.contains("__VIEWSTATE") && !html.contains("samplecaptcha");

    if is_new_style {
        tracing::debug!("detected new-style advance check page (SPA), cannot handle in-app");
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: format!("advance_check_web:{}", page_url),
        }));
    }

    let viewstate = extract_html_field(&html, "__VIEWSTATE").unwrap_or_default();
    let event_validation = extract_html_field(&html, "__EVENTVALIDATION").unwrap_or_default();
    let viewstate_generator = extract_html_field(&html, "__VIEWSTATEGENERATOR").unwrap_or_default();

    // Extract samplecaptcha ID — try multiple patterns
    let samplecaptcha = {
        // Pattern 1: id="BDC_VCID_..." value="..."
        let re1 = Regex::new(
            r#"(?i)BDC_VCID_c_logincheck_advancecheck_samplecaptcha[^>]*value="([^"]+)""#,
        )
        .ok();
        // Pattern 2: value="..." id="BDC_VCID_..."
        let re2 = Regex::new(
            r#"(?i)value="([^"]+)"[^>]*BDC_VCID_c_logincheck_advancecheck_samplecaptcha"#,
        )
        .ok();
        // Pattern 3: name="LBD_VCID_..." value="..."
        let re3 = Regex::new(
            r#"(?i)name="LBD_VCID_c_logincheck_advancecheck_samplecaptcha"[^>]*value="([^"]+)""#,
        )
        .ok();

        re1.and_then(|r| r.captures(&html))
            .and_then(|c| c.get(1))
            .or_else(|| re2.and_then(|r| r.captures(&html)).and_then(|c| c.get(1)))
            .or_else(|| re3.and_then(|r| r.captures(&html)).and_then(|c| c.get(1)))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| {
                tracing::error!(
                    "no samplecaptcha found. HTML preview: {}",
                    &html[..html.len().min(2000)]
                );
                parse_error_str("no samplecaptcha found in advance check page")
            })?
    };

    // Extract form action URL if present
    let action_re = Regex::new(r#"<form[^>]+action="([^"]+)""#).ok();
    let submit_url = action_re
        .and_then(|re| re.captures(&html))
        .and_then(|c| c.get(1))
        .map(|m| {
            let action = m.as_str();
            if action.starts_with("http") {
                action.to_string()
            } else if action.starts_with("/") {
                format!("https://tw.newlogin.beanfun.com{action}")
            } else {
                // Relative path — resolve against the page URL's directory
                let base = page_url.rfind('/').map_or(page_url, |i| &page_url[..i]);
                format!("{base}/{action}")
            }
        })
        .unwrap_or_else(|| page_url.to_string());

    // Extract auth type hint from the HTML
    // lblVerify = "請輸入認證EMAIL" (what to input)
    // lblAuth = "提示您進階驗證資料為：" (label)
    // lblAuthType = "NOXXXXXXXXXXXXXXXXXXXXXXXX" (masked value)
    let auth_hint = {
        let verify_label_re = Regex::new(r#"id="lblVerify"[^>]*>([^<]+)<"#).ok();
        let auth_type_re = Regex::new(r#"id="lblAuthType"[^>]*>([^<]*)<"#).ok();

        let verify_label = verify_label_re
            .and_then(|r| r.captures(&html))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        let auth_type = auth_type_re
            .and_then(|r| r.captures(&html))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        if !verify_label.is_empty() && !auth_type.is_empty() {
            format!("{verify_label}\n{auth_type}")
        } else if !verify_label.is_empty() {
            verify_label
        } else if !auth_type.is_empty() {
            auth_type
        } else {
            String::new()
        }
    };

    tracing::debug!("advance check submit_url={submit_url}, auth_hint={auth_hint}");

    // Download captcha image as base64
    let captcha_url = format!(
        "https://tw.newlogin.beanfun.com/LoginCheck/BotDetectCaptcha.ashx?get=image&c=c_logincheck_advancecheck_samplecaptcha&t={samplecaptcha}"
    );
    let captcha_bytes = client
        .get(&captcha_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&captcha_url, e))?
        .bytes()
        .await
        .map_err(|e| map_reqwest_error(&captcha_url, e))?;

    use base64::Engine;
    let captcha_b64 = base64::engine::general_purpose::STANDARD.encode(&captcha_bytes);
    let captcha_image_base64 = format!("data:image/png;base64,{captcha_b64}");

    tracing::debug!("advance check page loaded, captcha obtained");

    Ok(AdvanceCheckState {
        viewstate,
        viewstate_generator,
        event_validation,
        samplecaptcha,
        submit_url,
        captcha_image_base64,
        auth_hint,
    })
}

/// Submit the advance check verification form.
///
/// Returns `true` if verification succeeded (response contains "資料已驗證成功").
/// Returns an error message string if it failed.
pub async fn submit_advance_check(
    client: &Client,
    state: &AdvanceCheckState,
    verify_code: &str,
    captcha_code: &str,
) -> Result<bool, LoginError> {
    let mut form: Vec<(&str, &str)> = vec![
        ("__VIEWSTATE", &state.viewstate),
        ("__EVENTVALIDATION", &state.event_validation),
        ("txtVerify", verify_code),
        ("CodeTextBox", captcha_code),
        ("imgbtnSubmit.x", "19"),
        ("imgbtnSubmit.y", "23"),
        (
            "LBD_VCID_c_logincheck_advancecheck_samplecaptcha",
            &state.samplecaptcha,
        ),
    ];

    if !state.viewstate_generator.is_empty() {
        form.push(("__VIEWSTATEGENERATOR", &state.viewstate_generator));
    }

    tracing::debug!(
        "submit advance check: url={}, viewstate_len={}, samplecaptcha={}",
        state.submit_url,
        state.viewstate.len(),
        state.samplecaptcha
    );

    let resp = client
        .post(&state.submit_url)
        .header("User-Agent", USER_AGENT)
        .form(&form)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&state.submit_url, e))?;

    let body = resp.text().await.unwrap_or_default();

    tracing::trace!(
        "advance check submit response length={}, preview={}",
        body.len(),
        &body[..body.len().min(2000)]
    );

    // Check for success
    if body.contains("資料已驗證成功") || body.contains("alert('資料已驗證成功") {
        tracing::info!("advance check verification succeeded");
        return Ok(true);
    }

    // Check for specific error messages
    if body.contains("圖形驗證碼輸入錯誤") {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "captcha code incorrect".into(),
        }));
    }

    // Extract alert message if present
    let alert_re = Regex::new(r"alert\('([^']+)'\)").ok();
    if let Some(msg) = alert_re
        .and_then(|re| re.captures(&body))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
    {
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: msg,
        }));
    }

    Err(LoginError::Auth(AuthError::InvalidCredentials {
        reason: "verification failed".into(),
    }))
}

/// Refresh the captcha image for an in-progress advance check.
///
/// Downloads a new captcha image using the existing samplecaptcha ID.
pub async fn refresh_advance_check_captcha(
    client: &Client,
    samplecaptcha: &str,
) -> Result<String, LoginError> {
    let captcha_url = format!(
        "https://tw.newlogin.beanfun.com/LoginCheck/BotDetectCaptcha.ashx?get=image&c=c_logincheck_advancecheck_samplecaptcha&t={samplecaptcha}"
    );
    let captcha_bytes = client
        .get(&captcha_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&captcha_url, e))?
        .bytes()
        .await
        .map_err(|e| map_reqwest_error(&captcha_url, e))?;

    use base64::Engine;
    let captcha_b64 = base64::engine::general_purpose::STANDARD.encode(&captcha_bytes);
    Ok(format!("data:image/png;base64,{captcha_b64}"))
}

// ---------------------------------------------------------------------------
// Shared Helpers
// ---------------------------------------------------------------------------

/// Extract `akey` from a final URL (after redirect) or from the response body.
fn extract_akey_from_url_or_body(url: &str, body: &str) -> Result<String, LoginError> {
    let re = Regex::new(r"akey=([^&\s]+)").ok();

    // Check URL first
    if let Some(ref re) = re {
        if let Some(caps) = re.captures(url) {
            if let Some(m) = caps.get(1) {
                return Ok(m.as_str().to_string());
            }
        }
    }

    // Check body
    if let Some(ref re) = re {
        if let Some(caps) = re.captures(body) {
            if let Some(m) = caps.get(1) {
                return Ok(m.as_str().to_string());
            }
        }
    }

    // Try to extract error message
    let msg_re = Regex::new(r"MsgBox\.Show\('([^']*)'\)").ok();
    if let Some(re) = msg_re {
        if let Some(caps) = re.captures(body) {
            if let Some(m) = caps.get(1) {
                return Err(LoginError::Auth(AuthError::InvalidCredentials {
                    reason: m.as_str().to_string(),
                }));
            }
        }
    }

    Err(LoginError::Auth(AuthError::InvalidCredentials {
        reason: "login failed: no auth key in response".into(),
    }))
}

// ---------------------------------------------------------------------------
// HTTP & Parsing Helpers
// ---------------------------------------------------------------------------

/// Perform a GET request and return the response body as text.
async fn http_get_text(client: &Client, url: &str) -> Result<String, LoginError> {
    assert_https(url)?;

    let resp = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| map_reqwest_error(url, e))?;

    if !resp.status().is_success() {
        return Err(LoginError::Network(NetworkError::HttpError {
            status: resp.status().as_u16(),
            url: url.to_string(),
        }));
    }

    resp.text().await.map_err(|e| map_reqwest_error(url, e))
}

/// Extract an ASP.NET hidden field value from HTML.
///
/// Matches `id="{field_name}" value="{value}" />` pattern used by ASP.NET WebForms.
fn extract_html_field(html: &str, field_name: &str) -> Result<String, LoginError> {
    let pattern = format!(r#"id="{field_name}" value="(.*)" />"#);
    let re = Regex::new(&pattern)
        .map_err(|_| parse_error_str(&format!("failed to compile regex for {field_name}")))?;

    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            LoginError::Auth(AuthError::InvalidCredentials {
                reason: format!("missing {field_name} in login page"),
            })
        })
}
/// Extract `__RequestVerificationToken` from an HTML page.
///
/// Looks for `<input name="__RequestVerificationToken" ... value="TOKEN" />`
/// using a name-based regex (the field may not have an `id` attribute).
/// Extract `__RequestVerificationToken` from an HTML page.
///
/// Looks for `<input ... __RequestVerificationToken ... value="TOKEN" ... />`
/// The regex does NOT assume `name` appears before `value` in the HTML attributes,
/// since attribute order is not guaranteed.
fn extract_request_verification_token(html: &str) -> Result<String, LoginError> {
    // Step 1: Find the <input> tag that contains __RequestVerificationToken
    let tag_re = Regex::new(r#"<input[^>]+__RequestVerificationToken[^>]*>"#)
        .map_err(|_| parse_error_str("failed to compile __RequestVerificationToken tag regex"))?;

    let tag = tag_re.find(html).map(|m| m.as_str()).ok_or_else(|| {
        LoginError::Auth(AuthError::InvalidCredentials {
            reason: "missing __RequestVerificationToken in login page".into(),
        })
    })?;

    // Step 2: Extract value="..." from that tag (order-independent)
    let val_re = Regex::new(r#"value="([^"]+)""#)
        .map_err(|_| parse_error_str("failed to compile value regex"))?;

    val_re
        .captures(tag)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            LoginError::Auth(AuthError::InvalidCredentials {
                reason: "missing value in __RequestVerificationToken input".into(),
            })
        })
}

/// Validate that a URL uses HTTPS.
fn assert_https(url: &str) -> Result<(), LoginError> {
    if !url.starts_with("https://") {
        return Err(LoginError::Network(NetworkError::ConnectionFailed {
            url: format!("insecure URL rejected: {url}"),
        }));
    }
    Ok(())
}
/// Extract `bfWebToken` cookie value from the shared cookie jar.
fn read_bf_web_token(cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>, host: &str) -> String {
    let jar_url: url::Url = format!("https://{host}/").parse().unwrap();
    cookie_jar
        .cookies(&jar_url)
        .and_then(|h: reqwest::header::HeaderValue| {
            h.to_str().ok().and_then(|s: &str| {
                s.split(';')
                    .find_map(|c: &str| c.trim().strip_prefix("bfWebToken=").map(String::from))
            })
        })
        .unwrap_or_default()
}

/// Map a `reqwest::Error` into our domain [`NetworkError`].
fn map_reqwest_error(url: &str, err: reqwest::Error) -> LoginError {
    if err.is_timeout() {
        LoginError::Network(NetworkError::Timeout {
            url: url.to_string(),
        })
    } else if err.is_connect() {
        LoginError::Network(NetworkError::ConnectionFailed {
            url: url.to_string(),
        })
    } else {
        LoginError::Network(NetworkError::ConnectionFailed {
            url: format!("{url} ({err})"),
        })
    }
}

/// Create a parse/protocol error.
fn parse_error_str(msg: &str) -> LoginError {
    LoginError::Network(NetworkError::HttpError {
        status: 200,
        url: msg.to_string(),
    })
}

/// Basic HTML entity decoding.
fn html_decode(s: &str) -> String {
    let s = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Decode numeric character references: &#NNNN; → Unicode char
    let re = Regex::new(r"&#(\d+);").unwrap();
    re.replace_all(&s, |caps: &regex::Captures| {
        caps.get(1)
            .and_then(|m| m.as_str().parse::<u32>().ok())
            .and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_else(|| caps[0].to_string())
    })
    .to_string()
}

/// Generate timestamp in method-2 format:
/// `{year}{month-1}{ddHHmmssfff}`
fn get_current_time_method2() -> String {
    let now = chrono::Local::now();
    let year = now.format("%Y");
    let month_zero_based = now.format("%m").to_string().parse::<u32>().unwrap_or(1) - 1;
    let rest = now.format("%d%H%M%S%3f");
    format!("{year}{month_zero_based}{rest}")
}

/// Generate timestamp in default format:
/// `yyyyMMddHHmmss.fff`
fn get_current_time_default() -> String {
    chrono::Local::now().format("%Y%m%d%H%M%S%.3f").to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_https_accepts_valid_urls() {
        assert!(assert_https("https://tw.beanfun.com/api/login").is_ok());
        assert!(assert_https("https://hk.beanfun.com/api/login").is_ok());
    }

    #[test]
    fn assert_https_rejects_insecure_urls() {
        assert!(assert_https("http://tw.beanfun.com/api/login").is_err());
        assert!(assert_https("ftp://example.com").is_err());
        assert!(assert_https("").is_err());
    }

    #[test]
    fn extract_html_field_parses_viewstate() {
        let html = r#"<input type="hidden" name="__VIEWSTATE" id="__VIEWSTATE" value="abc123" />"#;
        let result = extract_html_field(html, "__VIEWSTATE").unwrap();
        assert_eq!(result, "abc123");
    }

    #[test]
    fn extract_html_field_missing_returns_error() {
        let html = "<html><body>no fields here</body></html>";
        assert!(extract_html_field(html, "__VIEWSTATE").is_err());
    }

    #[test]
    fn extract_akey_from_url() {
        let result =
            extract_akey_from_url_or_body("https://example.com/callback?akey=MYAUTHKEY123", "");
        assert_eq!(result.unwrap(), "MYAUTHKEY123");
    }

    #[test]
    fn extract_akey_missing_returns_error() {
        let result = extract_akey_from_url_or_body("https://example.com/", "<html>no akey</html>");
        assert!(result.is_err());
    }

    #[test]
    fn html_decode_entities() {
        assert_eq!(html_decode("a&amp;b"), "a&b");
        assert_eq!(html_decode("&lt;div&gt;"), "<div>");
        assert_eq!(html_decode("he said &quot;hi&quot;"), r#"he said "hi""#);
    }

    #[test]
    fn timestamp_method2_format() {
        let ts = get_current_time_method2();
        // Should be at least 15 chars: 4(year) + 1-2(month) + 11(rest)
        assert!(ts.len() >= 15, "timestamp too short: {ts}");
    }

    #[test]
    fn timestamp_default_format() {
        let ts = get_current_time_default();
        // Format: yyyyMMddHHmmss.fff → 18 chars
        assert!(ts.contains('.'), "timestamp should contain dot: {ts}");
    }

    /// Verify that the config serializer does not write any credential-like
    /// fields.
    #[test]
    fn config_serializer_excludes_credentials() {
        use crate::core::config_parser::serialize_ini;
        use crate::models::config::AppConfig;

        let config = AppConfig::default();
        let output = serialize_ini(&config);

        let forbidden = ["token", "password", "refresh_token", "session", "secret"];
        for keyword in &forbidden {
            assert!(
                !output.to_lowercase().contains(keyword),
                "config output must not contain credential keyword '{keyword}': {output}"
            );
        }
    }

    #[test]
    fn extract_request_verification_token_parses_correctly() {
        // name before value
        let html =
            r#"<input name="__RequestVerificationToken" type="hidden" value="CfDJ8ABC123XYZ" />"#;
        let result = extract_request_verification_token(html).unwrap();
        assert_eq!(result, "CfDJ8ABC123XYZ");
    }

    #[test]
    fn extract_request_verification_token_value_before_name() {
        // value before name — attribute order should not matter
        let html =
            r#"<input type="hidden" value="TokenXYZ789" name="__RequestVerificationToken" />"#;
        let result = extract_request_verification_token(html).unwrap();
        assert_eq!(result, "TokenXYZ789");
    }

    #[test]
    fn extract_request_verification_token_missing_returns_error() {
        let html = "<html><body>no token here</body></html>";
        assert!(extract_request_verification_token(html).is_err());
    }
}
