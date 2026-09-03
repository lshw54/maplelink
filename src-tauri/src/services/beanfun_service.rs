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
use tokio::time::{sleep, Duration};

use crate::core::error::{AuthError, NetworkError};
use crate::models::game_account::{GameAccount, GameCredentials};
use crate::models::session::{Region, Session, TotpState};
use crate::utils::crypto::des_ecb_decrypt_hex;

/// Default User-Agent for all beanfun requests — a current Chrome string
/// (kept in sync with the session client defaults and [`SEC_CH_UA`]).
use crate::services::http_util::USER_AGENT;

/// Same modern Chrome UA, used explicitly on the TW login POSTs alongside
/// [`SEC_CH_UA`]. Kept distinct so the login fingerprint stays pinned even if
/// the default UA is ever changed again.
const BROWSER_UA: &str = USER_AGENT;

/// Chrome client-hint brand string; the Chrome major MUST match [`BROWSER_UA`]
/// (a version mismatch between the UA and `sec-ch-ua` is a bot signal → beanfun
/// rejects the reCAPTCHA token and IP-locks for 15 min). Verified against real
/// Chrome 150.
const SEC_CH_UA: &str =
    "\"Not;A=Brand\";v=\"8\", \"Chromium\";v=\"150\", \"Google Chrome\";v=\"150\"";

/// Magic constant used in the OTP retrieval request.
/// beanfun TW.
const TW_HOST: &str = "tw.beanfun.com";

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
/// reCAPTCHA Enterprise tokens for the two TW Regular login challenge points.
///
/// The TW帳密 flow is gated twice: once at `CheckAccountType` (solved after the
/// account is entered) and once at `AccountLogin` (solved after the password).
/// Each is a distinct, single-use, action-bound Enterprise token.
#[derive(Debug, Clone, Default)]
pub struct RecaptchaTokens {
    /// Token for the `CheckAccountType` step.
    pub check: Option<String>,
    /// Token for the `AccountLogin` step.
    pub login: Option<String>,
}

pub async fn login(
    client: &Client,
    account: &str,
    password: &str,
    region: &Region,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    tokens: &RecaptchaTokens,
) -> Result<Session, LoginError> {
    match region {
        // HK login uses the advance-check captcha flow, not reCAPTCHA — tokens ignored.
        Region::HK => hk_login(client, account, password).await,
        Region::TW => tw_login(client, account, password, cookie_jar, tokens).await,
    }
}

/// Start a QR-code login flow (TW region only).
///
/// Gets a session key, then fetches the QR code image from the TW login API.
pub async fn qr_login_start(
    client: &Client,
    region: &Region,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<QrCodeData, LoginError> {
    match region {
        Region::TW => tw_qr_start(client, cookie_jar).await,
        Region::HK => Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: "QR login is only available for TW region".into(),
        })),
    }
}

pub fn parse_hk_session_key_html(html: &str) -> Result<String, LoginError> {
    let re = Regex::new(r#"<span id="ctl00_ContentPlaceHolder1_lblOtp1">(.*)</span>"#)
        .map_err(|_| parse_error_str("failed to compile session key regex"))?;

    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            LoginError::Auth(AuthError::InvalidCredentials {
                reason: "failed to extract session key (no OTP1 span)".into(),
            })
        })
}

pub fn parse_tw_session_key_url(url: &str) -> Result<String, LoginError> {
    extract_tw_session_key(url).ok_or_else(|| {
        LoginError::Auth(AuthError::InvalidCredentials {
            reason: "failed to extract TW session key from redirect URL".into(),
        })
    })
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

/// Fetch the creation time for a single service account on demand.
///
/// The list load intentionally no longer fetches this per account (it made one
/// beanfun request per account on every refresh — a rate-limit risk), so the
/// account-detail popup calls this lazily for just the account being viewed.
/// `sc`/`sr` come from the account's `game_type` (`"{sc}_{sr}"`). Empty on failure.
pub async fn fetch_account_create_time(
    client: &Client,
    region: &Region,
    sc: &str,
    sr: &str,
    sn: &str,
) -> String {
    let host = match region {
        Region::HK => "bfweb.hk.beanfun.com",
        Region::TW => "tw.beanfun.com",
    };
    let timestamp = get_current_time_method2();
    let url = format!(
        "https://{host}/beanfun_block/game_zone/game_start_step2.aspx\
         ?service_code={sc}&service_region={sr}&sotp={sn}&dt={timestamp}"
    );
    match http_get_text(client, &url).await {
        Ok(html) => Regex::new(r#"ServiceAccountCreateTime: "([^"]+)""#)
            .ok()
            .and_then(|re| {
                re.captures(&html)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
            })
            .unwrap_or_default(),
        Err(_) => String::new(),
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
    known_accounts: &[GameAccount],
) -> Result<GameCredentials, LoginError> {
    match session.region {
        Region::HK => hk_get_otp(client, session, account_id, cookie_jar).await,
        // TW's endpoint now answers only the Gamania Games Manager, so the
        // credentials come back through it. `tw_get_otp` is kept as the
        // fallback: if beanfun ever serves them to us again, it still works.
        // Two routes. The first is what the game manager does, done here: one
        // page and one POST, and the code comes back to the window that asked.
        // The second hands the launch to the manager itself and catches what it
        // produces — slower, and it needs the manager installed, but it
        // survives beanfun changing this API again, because the manager updates
        // itself.
        Region::TW => match tw_get_otp_v2(client, session, account_id, cookie_jar, known_accounts)
            .await
        {
            Ok(creds) => Ok(creds),
            Err(e) => {
                tracing::warn!("TW: the v2 route failed ({e}); handing the launch to the manager");
                crate::services::ggm_launch::credentials_via_ggm(
                    client, session, account_id, cookie_jar,
                )
                .await
            }
        },
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

impl From<LoginError> for crate::core::error::AppError {
    fn from(err: LoginError) -> Self {
        match err {
            LoginError::Auth(e) => e.into(),
            LoginError::Network(e) => e.into(),
        }
    }
}

/// A failure in the hand-off to the Gamania Games Manager.
pub fn launch_handoff_error(reason: &str) -> LoginError {
    LoginError::Auth(AuthError::InvalidCredentials {
        reason: reason.to_string(),
    })
}

#[derive(Debug, Clone)]
struct RegisteredDeviceContinuation {
    login_token: String,
}

// ---------------------------------------------------------------------------
// HK Login Implementation
// ---------------------------------------------------------------------------

/// Full HK regular login flow: GetSessionkey → HkRegularLogin → LoginCompleted.
async fn hk_login(client: &Client, account: &str, password: &str) -> Result<Session, LoginError> {
    tracing::info!(
        account = %mask_account(account),
        "HK regular login start"
    );

    // Step 1: Get session key
    let skey = hk_get_session_key(client).await?;
    tracing::info!(session_key_len = skey.len(), "HK session key obtained");

    hk_login_with_session_key(client, account, password, &skey).await
}

async fn hk_login_with_session_key(
    client: &Client,
    account: &str,
    password: &str,
    skey: &str,
) -> Result<Session, LoginError> {
    tracing::info!(
        session_key_len = skey.len(),
        "HK login using provided session key"
    );

    // Step 2: Login form submission
    let login_url =
        format!("https://login.hk.beanfun.com/login/id-pass_form_newBF.aspx?otp1={skey}");

    let page_html = http_get_text(client, &login_url).await?;
    tracing::info!(
        response_len = page_html.len(),
        preview = %preview_text(&page_html, 180),
        "HK login form fetched"
    );

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
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            region: Region::HK,
            account_name: account.to_string(),
            session_key: Some(skey.to_string()),
            totp_state: Some(TotpState {
                response_html: response_body,
                post_url: login_url,
            }),
        };
        return Err(LoginError::Auth(AuthError::TotpRequired {
            partial_session: Box::new(partial_session),
        }));
    }

    // Check for device-registration continuation before treating missing
    // `akey` as a hard failure. Legacy Beanfun keeps polling until approval.
    let web_token =
        if let Some(continuation) = extract_registered_device_continuation(&response_body) {
            tracing::info!("HK login requires registered-device approval");
            hk_wait_for_registered_device_approval(client, skey, &continuation.login_token).await?
        } else {
            let akey = extract_akey_from_url_or_body(&final_url, &response_body)?;
            hk_login_completed(client, skey, &akey).await?
        };

    Ok(Session {
        token: web_token,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
        region: Region::HK,
        account_name: account.to_string(),
        session_key: Some(skey.to_string()),
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

    let web_token =
        if let Some(continuation) = extract_registered_device_continuation(&response_body) {
            tracing::info!("HK TOTP verification requires registered-device approval");
            hk_wait_for_registered_device_approval(client, skey, &continuation.login_token).await?
        } else {
            let akey = extract_akey_from_url_or_body(&final_url, &response_body)?;
            hk_login_completed(client, skey, &akey).await?
        };

    Ok(Session {
        token: web_token,
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
            // Fetched on demand from the game_start_step2 page when launching
            // (avoids one beanfun request per account on every list load).
            created_at: String::new(),
        });
    }

    // Sort by sn
    accounts.sort_by(|a, b| a.sn.cmp(&b.sn));

    tracing::info!("HK: found {} game accounts", accounts.len());
    Ok(accounts)
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
        return Err(otp_failure_error(parts.get(1).unwrap_or(&"")));
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
    tracing::info!(url, "HK session-key request start");
    let html = http_get_text(client, url).await?;

    tracing::info!(
        response_len = html.len(),
        preview = %preview_text(&html, 220),
        "HK session-key page fetched"
    );

    parse_hk_session_key_html(&html).inspect_err(|_err| {
        tracing::error!("no OTP1 span found in response. Full HTML:\n{html}");
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

async fn hk_wait_for_registered_device_approval(
    client: &Client,
    skey: &str,
    login_token: &str,
) -> Result<String, LoginError> {
    const POLL_ATTEMPTS: usize = 90;
    const POLL_INTERVAL: Duration = Duration::from_secs(2);
    let poll_url = "https://tw.newlogin.beanfun.com/login/bfAPPAutoLogin.ashx";

    for attempt in 0..POLL_ATTEMPTS {
        let resp = client
            .post(poll_url)
            .header("User-Agent", USER_AGENT)
            .form(&[("LT", login_token)])
            .send()
            .await
            .map_err(|e| map_reqwest_error(poll_url, e))?;

        let body = resp
            .text()
            .await
            .map_err(|e| map_reqwest_error(poll_url, e))?;
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|_| parse_error_str("failed to parse bfAPPAutoLogin response"))?;

        let int_result = json["IntResult"].as_str().unwrap_or_default();
        let str_result = json["StrReslut"].as_str().unwrap_or_default();

        tracing::debug!(
            "registered-device poll attempt={} result={} str_result={}",
            attempt + 1,
            int_result,
            str_result
        );

        match int_result {
            "0" | "1" => {
                sleep(POLL_INTERVAL).await;
            }
            "2" => {
                let callback_url = normalize_registered_device_callback_url(str_result);
                let callback_body = http_get_text(client, &callback_url).await?;
                let akey = extract_akey_from_url_or_body(str_result, &callback_body)?;
                return hk_login_completed(client, skey, &akey).await;
            }
            "-1" => {
                return Err(LoginError::Auth(AuthError::InvalidCredentials {
                    reason: if str_result.is_empty() {
                        "registered-device login failed".into()
                    } else {
                        str_result.to_string()
                    },
                }));
            }
            "-2" => return Err(LoginError::Auth(AuthError::SessionExpired)),
            "-3" => {
                return Err(LoginError::Auth(AuthError::InvalidCredentials {
                    reason: "registered-device login request was rejected".into(),
                }));
            }
            _ => {
                return Err(LoginError::Auth(AuthError::InvalidCredentials {
                    reason: if str_result.is_empty() {
                        format!("unexpected registered-device status: {int_result}")
                    } else {
                        str_result.to_string()
                    },
                }));
            }
        }
    }

    Err(LoginError::Auth(AuthError::SessionExpired))
}

// ---------------------------------------------------------------------------
// TW Login Implementation
// ---------------------------------------------------------------------------

/// Get the TW session key (pSKey) from the first redirect of bflogin/default.aspx.
///
/// Beanfun puts the pSKey in the **first** redirect's `Location`. The session
/// client follows redirects, so it can only see the END of the chain — which
/// sometimes lands on a check-in / `BlockIPMessage` page with no key, producing
/// spurious "no session key" failures. So do this one request with a
/// non-following client (sharing the session cookie jar) and read that first
/// `Location` directly.
async fn tw_get_session_key(
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<String, LoginError> {
    let url = "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0";
    tracing::info!(url, "TW session-key request start");

    let noredirect = Client::builder()
        .cookie_provider(cookie_jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .http1_only()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| map_reqwest_error(url, e))?;

    // Match the session client's browser fingerprint so the request isn't
    // bot-flagged (UA + client hints), while overriding Accept/Accept-Encoding.
    let resp = noredirect
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("sec-ch-ua", SEC_CH_UA)
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", "\"Windows\"")
        .header("Accept-Language", "zh-TW,zh;q=0.9,en-US;q=0.8,en;q=0.7")
        .header("Accept", "text/html")
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .map_err(|e| map_reqwest_error(url, e))?;

    let status = resp.status();
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let final_url = resp.url().to_string();
    tracing::info!(
        status = %status,
        location = %location,
        final_url = %final_url,
        "TW session-key response received"
    );

    extract_tw_session_key(&location)
        .or_else(|| extract_tw_session_key(&final_url))
        .ok_or_else(|| {
            let target = if location.is_empty() {
                final_url.as_str()
            } else {
                location.as_str()
            };
            tracing::error!("no pSKey found in TW session-key response: {target}");

            let reason = if target.contains("BlockIPMessage") {
                "Beanfun temporarily blocked this IP (too many attempts) — please wait a few minutes and try again".into()
            } else {
                "failed to obtain TW session key".into()
            };

            LoginError::Auth(AuthError::InvalidCredentials { reason })
        })
}

fn extract_tw_session_key(url: &str) -> Option<String> {
    let re = Regex::new(r"[pP]?[sS][Kk]ey=([^&\s]+)").ok()?;
    re.captures(url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Full TW regular login flow using the new JSON API.
async fn tw_login(
    client: &Client,
    account: &str,
    password: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    tokens: &RecaptchaTokens,
) -> Result<Session, LoginError> {
    let skey = tw_get_session_key(cookie_jar).await?;
    tracing::debug!("TW session key: {}", &skey[..skey.len().min(20)]);

    tw_login_with_session_key(client, account, password, cookie_jar, &skey, tokens).await
}

async fn tw_login_with_session_key(
    client: &Client,
    account: &str,
    password: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    skey: &str,
    tokens: &RecaptchaTokens,
) -> Result<Session, LoginError> {
    let api_base = "https://login.beanfun.com";
    let index_url = format!("{api_base}/Login/Index?pSKey={skey}");

    // Step 1: Get index page and __RequestVerificationToken
    let index_html = http_get_text(client, &index_url).await?;
    let form_token = extract_request_verification_token(&index_html)?;
    tracing::debug!("TW form token obtained");

    // Step 2 + 3: CheckAccountType, then AccountLogin.
    let server_captcha =
        tw_check_account_type(client, skey, &form_token, account, tokens.check.as_deref()).await?;
    tw_account_login(
        client,
        skey,
        &form_token,
        account,
        password,
        tokens.login.as_deref(),
        &server_captcha,
        cookie_jar,
    )
    .await
}

/// Phase 1 of the two-phase TW Regular login.
///
/// Bootstraps a fresh session key, fetches the form token, and runs
/// `CheckAccountType` with the first reCAPTCHA token (solved right after the
/// account is entered — so the token is fresh and unexpired). Returns
/// `(skey, form_token)` for the caller to stash and feed to [`tw_login_submit`].
pub async fn tw_login_check(
    client: &Client,
    account: &str,
    check_token: Option<&str>,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<(String, String), LoginError> {
    let skey = tw_get_session_key(cookie_jar).await?;
    tracing::debug!("TW (phase 1) session key obtained");

    let api_base = "https://login.beanfun.com";
    let index_url = format!("{api_base}/Login/Index?pSKey={skey}");
    let index_html = http_get_text(client, &index_url).await?;
    let form_token = extract_request_verification_token(&index_html)?;

    // We don't need the echoed captcha here (phase 2 carries its own token),
    // but running CheckAccountType validates the account + first reCAPTCHA.
    let _ = tw_check_account_type(client, &skey, &form_token, account, check_token).await?;

    Ok((skey, form_token))
}

/// Phase 2 of the two-phase TW Regular login.
///
/// Submits the password with the second reCAPTCHA token, reusing the
/// `skey`/`form_token` produced by [`tw_login_check`].
#[allow(clippy::too_many_arguments)]
pub async fn tw_login_submit(
    client: &Client,
    skey: &str,
    form_token: &str,
    account: &str,
    password: &str,
    login_token: Option<&str>,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Session, LoginError> {
    tw_account_login(
        client,
        skey,
        form_token,
        account,
        password,
        login_token,
        "",
        cookie_jar,
    )
    .await
}

/// POST `CheckAccountType` with the first reCAPTCHA token (empty when the
/// account needs no captcha). Returns the captcha token the server echoes back
/// — used as the `AccountLogin` fallback in the no-reCAPTCHA case.
async fn tw_check_account_type(
    client: &Client,
    skey: &str,
    form_token: &str,
    account: &str,
    check_token: Option<&str>,
) -> Result<String, LoginError> {
    let api_base = "https://login.beanfun.com";
    let index_url = format!("{api_base}/Login/Index?pSKey={skey}");
    let check_url = format!("{api_base}/Login/CheckAccountType?pSKey={skey}");
    let check_body = serde_json::json!({
        "Account": account,
        "Captcha": check_token.unwrap_or(""),
        "__RequestVerificationToken": form_token,
    });

    let check_resp = with_browser_xhr_headers(
        client
            .post(&check_url)
            .header("User-Agent", BROWSER_UA)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("RequestVerificationToken", form_token)
            .header("Referer", &index_url)
            .header("Origin", api_base),
    )
    .json(&check_body)
    .send()
    .await
    .map_err(|e| map_reqwest_error(&check_url, e))?;

    let check_text = check_resp.text().await.unwrap_or_default();
    tracing::debug!(
        "TW CheckAccountType response: {}",
        &check_text[..check_text.len().min(500)]
    );
    let check_json = serde_json::from_str::<serde_json::Value>(&check_text).ok();

    let result_code = check_json
        .as_ref()
        .and_then(|j| j["ResultCode"].as_i64())
        .unwrap_or(1);
    let result_msg = check_json
        .as_ref()
        .and_then(|j| j["ResultMessage"].as_str())
        .unwrap_or("");

    // ResultCode 1 == Success (beanfun convention). Surface anything else.
    if result_code != 1 {
        // If beanfun is asking for a reCAPTCHA (account flagged IsRecaptcha) and
        // we didn't send a token, signal the caller to obtain one and retry —
        // most accounts don't need it, so we try without one first.
        if recaptcha_required(check_json.as_ref(), result_msg) && is_blank(check_token) {
            return Err(LoginError::Auth(AuthError::RecaptchaRequired));
        }
        return Err(LoginError::Auth(AuthError::InvalidCredentials {
            reason: map_beanfun_error(result_msg),
        }));
    }

    Ok(check_json
        .as_ref()
        .and_then(|j| j["ResultData"]["Captcha"].as_str().map(String::from))
        .unwrap_or_default())
}

/// Whether `token` is absent or empty.
fn is_blank(token: Option<&str>) -> bool {
    token.map(|t| t.trim().is_empty()).unwrap_or(true)
}

/// Add the `Accept` + `Sec-Fetch-*` + `sec-ch-ua` headers a real browser sends
/// on a same-origin `fetch`/XHR. Paired with [`BROWSER_UA`] on the two TW login
/// POSTs, this makes them look like the website — without it, beanfun's bot-risk
/// scoring tripped the ~5-min IP lock even after a human solved the reCAPTCHA.
/// These modern-browser headers are deliberately scoped to just those POSTs so
/// every other request stays on the legacy UA (see [`USER_AGENT`]).
fn with_browser_xhr_headers(rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    rb.header("Accept", "application/json, text/plain, */*")
        .header("sec-ch-ua", SEC_CH_UA)
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", "\"Windows\"")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
}

/// Whether a beanfun login response is demanding a reCAPTCHA: the account is
/// flagged `IsRecaptcha`, or the message is the "click I'm not a robot" prompt.
fn recaptcha_required(json: Option<&serde_json::Value>, result_msg: &str) -> bool {
    let is_recaptcha = json
        .and_then(|j| j["ResultData"]["IsRecaptcha"].as_bool())
        .unwrap_or(false);
    is_recaptcha || result_msg.contains("機器人") || result_msg.contains("recaptcha")
}

/// Map a beanfun API `ResultMessage` to a user-facing message using the
/// official login SDK's wording for known codes. Unknown messages pass through
/// (beanfun's `ResultMessage` is already user-facing); empty falls back.
fn map_beanfun_error(result_msg: &str) -> String {
    match result_msg.trim() {
        "" => "登入失敗，請稍後再試".to_string(),
        "AccountLock" => "帳號已被鎖定，可聯繫客服人員了解原因".to_string(),
        "Token Expired" => "連線逾時，請重新登入".to_string(),
        other => other.to_string(),
    }
}

/// Classify a failed `get_webstart_otp` server reply. beanfun's idle-timeout
/// wording ("閒置過久" / "請重新登入") means the web session is dead, so it maps
/// to [`AuthError::SessionExpired`] (→ `AUTH_SESSION_EXPIRED`) and the frontend
/// can route on the error code instead of sniffing the server's message text.
fn otp_failure_error(server_msg: &str) -> LoginError {
    if server_msg.contains("閒置過久") || server_msg.contains("重新登入") {
        LoginError::Auth(AuthError::SessionExpired)
    } else {
        LoginError::Auth(AuthError::InvalidCredentials {
            reason: format!("OTP retrieval failed: {server_msg}"),
        })
    }
}

/// POST `AccountLogin` with the second reCAPTCHA token, then complete via the
/// SendLogin flow and build the [`Session`].
#[allow(clippy::too_many_arguments)]
async fn tw_account_login(
    client: &Client,
    skey: &str,
    form_token: &str,
    account: &str,
    password: &str,
    login_token: Option<&str>,
    fallback_captcha: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<Session, LoginError> {
    let api_base = "https://login.beanfun.com";
    let index_url = format!("{api_base}/Login/Index?pSKey={skey}");

    // The reCAPTCHA token solved after the password is what AccountLogin
    // validates against; fall back to the server-echoed token (no-captcha case).
    let captcha_token = match login_token {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => fallback_captcha.to_string(),
    };

    let login_url = format!("{api_base}/Login/AccountLogin?pSKey={skey}");
    let login_body = serde_json::json!({
        "Account": account,
        "Pasw": password,
        "IsMobile": false,
        "Captcha": captcha_token,
        "__RequestVerificationToken": form_token,
    });

    let login_resp = with_browser_xhr_headers(
        client
            .post(&login_url)
            .header("User-Agent", BROWSER_UA)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("RequestVerificationToken", form_token)
            .header("Referer", &index_url)
            .header("Origin", api_base),
    )
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

    tracing::info!(
        result_code,
        result,
        msg = %result_msg,
        had_login_token = !is_blank(login_token),
        "TW AccountLogin result"
    );

    match result_code {
        1 => {
            if result == 1 {
                // AdvanceCheck required (no URL)
                return Err(LoginError::Auth(AuthError::AdvanceCheckRequired {
                    url: None,
                }));
            }
            // Success — complete via SendLogin flow
            let web_token = tw_send_login_flow(client, skey, cookie_jar).await?;
            Ok(Session {
                token: web_token,
                expires_at: chrono::Utc::now() + chrono::Duration::hours(6),
                region: Region::TW,
                account_name: account.to_string(),
                session_key: Some(skey.to_string()),
                totp_state: None,
            })
        }
        2 => {
            // Per the official login SDK, ResultCode 2 is either an account lock
            // or an advance-check redirect (ResultMessage = the URL).
            if result_msg == "AccountLock" {
                return Err(LoginError::Auth(AuthError::InvalidCredentials {
                    reason: "帳號已被鎖定，可聯繫客服人員了解原因".to_string(),
                }));
            }
            let url = if result_msg.starts_with("http") {
                Some(result_msg.to_string())
            } else {
                None
            };
            Err(LoginError::Auth(AuthError::AdvanceCheckRequired { url }))
        }
        _ => {
            // ResultCode 0 (and anything else) is a plain failure. If beanfun is
            // asking for a reCAPTCHA (IsRecaptcha) and we sent none, signal the
            // caller to obtain a token and retry — most accounts don't need it.
            if recaptcha_required(Some(&login_json), result_msg) && is_blank(login_token) {
                return Err(LoginError::Auth(AuthError::RecaptchaRequired));
            }
            Err(LoginError::Auth(AuthError::InvalidCredentials {
                reason: map_beanfun_error(result_msg),
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

    // The only client in the app that named no deadline at all, and it posts
    // to beanfun mid-login: a host that accepts the connection and then says
    // nothing would have held the login open indefinitely.
    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
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
async fn tw_qr_start(
    client: &Client,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<QrCodeData, LoginError> {
    let skey = tw_get_session_key(cookie_jar).await?;
    tw_qr_start_with_session_key(client, &skey).await
}

async fn tw_qr_start_with_session_key(
    client: &Client,
    skey: &str,
) -> Result<QrCodeData, LoginError> {
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
        session_key: skey.to_string(),
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
            // Fetched on demand from the game_start_step2 page when launching
            // (avoids one beanfun request per account on every list load).
            created_at: String::new(),
        });
    }

    accounts.sort_by(|a, b| a.sn.cmp(&b.sn));
    tracing::info!("TW: found {} game accounts", accounts.len());
    Ok(accounts)
}

/// Fetch the TW game-start page for `account_id` and read its GGM launch
/// ticket.
///
/// This is the path beanfun's own site now takes. It replaces nothing yet —
/// [`tw_get_otp`] still exists for the direct route — but it is the only route
/// that does not depend on impersonating GGM's integrity check.
pub async fn tw_ggm_ticket(
    client: &Client,
    session: &Session,
    account_id: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    known_accounts: &[GameAccount],
) -> Result<GgmTicket, LoginError> {
    let host = TW_HOST;

    // All this needs from the account is its `sn`, and the caller loaded the
    // list when it drew the account grid. Re-fetching it costs a round trip on
    // the path a user is waiting on, for a field that is already in hand.
    let fetched;
    let account = match known_accounts.iter().find(|a| a.id == account_id) {
        Some(account) => account,
        None => {
            fetched = tw_get_accounts(client, session, cookie_jar).await?;
            fetched.iter().find(|a| a.id == account_id).ok_or_else(|| {
                LoginError::Auth(AuthError::InvalidCredentials {
                    reason: format!("account {account_id} not found"),
                })
            })?
        }
    };

    let timestamp = get_current_time_method2();
    let url = game_start_step2_url(host, &account.sn, &timestamp);
    let html = http_get_text(client, &url).await?;

    let ticket = extract_ggm_ticket(&html)
        .ok_or_else(|| parse_error_str("no GGM launch ticket in the game-start page"))?;
    tracing::info!(
        region = %ticket.region,
        sn = %ticket.sn,
        data_len = ticket.data.len(),
        "TW: GGM launch ticket read"
    );

    // The page records the start alongside handing off to GGM, and beanfun's
    // "last played" is not something the user should wait for.
    let create_time = create_time_from(&html, account);
    let (bg_client, bg_html, bg_url, bg_account, bg_id) = (
        client.clone(),
        html.clone(),
        url.clone(),
        account.clone(),
        account_id.to_string(),
    );
    tokio::spawn(async move {
        tw_start_service(
            &bg_client,
            &bg_html,
            &bg_url,
            TW_HOST,
            &bg_account,
            &bg_id,
            &create_time,
        )
        .await;
    });

    Ok(ticket)
}

/// The create time to report for `account`, preferring what the account already
/// knows and falling back to what the page says.
fn create_time_from(html: &str, account: &GameAccount) -> String {
    if !account.created_at.is_empty() {
        return account.created_at.clone();
    }
    Regex::new(r#"ServiceAccountCreateTime: "([^"]+)""#)
        .ok()
        .and_then(|re| re.captures(html))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Tell beanfun the service is starting, the way the page does.
///
/// Best-effort: it is beanfun's own bookkeeping ("last played"), not something
/// the credentials depend on, so a failure here must not cost the user a launch.
/// The page appends a random-named anti-forgery field to this form, so we carry
/// whichever one this page load produced.
async fn tw_start_service(
    client: &Client,
    step2_html: &str,
    step2_url: &str,
    host: &str,
    account: &GameAccount,
    account_id: &str,
    create_time: &str,
) {
    let url = format!("https://{host}/beanfun_block/generic_handlers/record_service_start.ashx");
    let mut form: Vec<(&str, &str)> = vec![
        ("service_code", DEFAULT_SERVICE_CODE),
        ("service_region", DEFAULT_SERVICE_REGION),
        ("service_account_id", account_id),
        ("sotp", &account.sn),
        ("service_account_display_name", &account.display_name),
        ("service_account_create_time", create_time),
    ];

    let token = extract_service_start_token(step2_html);
    match &token {
        Some((name, value)) => form.push((name.as_str(), value.as_str())),
        None => tracing::warn!("TW: no service-start token in the game-start page"),
    }

    match client
        .post(&url)
        .header("User-Agent", USER_AGENT)
        .header(reqwest::header::REFERER, step2_url)
        .form(&form)
        .send()
        .await
    {
        Ok(response) => tracing::info!(status = %response.status(), "TW: service start recorded"),
        Err(e) => tracing::warn!("TW: could not record service start: {e}"),
    }
}

/// Game credentials in the shape the TW launcher expects.
fn tw_credentials(account_id: &str, otp: String) -> GameCredentials {
    GameCredentials {
        account_id: account_id.to_string(),
        otp,
        retrieved_at: chrono::Utc::now(),
        command_line_template: Some(
            "tw.login.maplestory.beanfun.com 8484 BeanFun %s %s".to_string(),
        ),
    }
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

fn extract_registered_device_continuation(body: &str) -> Option<RegisteredDeviceContinuation> {
    let re = Regex::new(r#"pollRequest\("([^"]*)","(\w+)","([^"]+)"\);"#).ok()?;
    let caps = re.captures(body)?;
    let login_token = caps.get(2)?.as_str().to_string();
    Some(RegisteredDeviceContinuation { login_token })
}

fn normalize_registered_device_callback_url(raw: &str) -> String {
    if raw.starts_with("https://") {
        raw.to_string()
    } else {
        format!(
            "https://tw.newlogin.beanfun.com/login/{}",
            raw.trim_start_matches('/')
        )
    }
}

// ---------------------------------------------------------------------------
// HTTP & Parsing Helpers
// ---------------------------------------------------------------------------

/// Perform a GET request and return the response body as text.
async fn http_get_text(client: &Client, url: &str) -> Result<String, LoginError> {
    http_get_text_from(client, url, None).await
}

/// [`http_get_text`], sent as though it came from the page at `referer`.
///
/// beanfun's `generic_handlers` now reject a request whose `Referer` is absent
/// or off-domain ("The URL referrer is null or from a different domain!"), so
/// the handlers a game-start page would call have to say which page they came
/// from. A browser fills this in on its own; we never did, which is why a flow
/// that had not otherwise changed started failing.
async fn http_get_text_from(
    client: &Client,
    url: &str,
    referer: Option<&str>,
) -> Result<String, LoginError> {
    assert_https(url)?;

    if is_auth_url(url) {
        tracing::info!(url, "HTTP GET start");
    }

    let mut request = client.get(url).header("User-Agent", USER_AGENT);
    if let Some(referer) = referer {
        request = request.header(reqwest::header::REFERER, referer);
    }

    let resp = request
        .send()
        .await
        .map_err(|e| map_reqwest_error(url, e))?;

    let status = resp.status();
    let final_url = resp.url().to_string();
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string();

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(
            url,
            %status,
            final_url = %final_url,
            location = %location,
            response_len = body.len(),
            preview = %preview_text(&body, 220),
            "HTTP GET failed with non-success status"
        );
        return Err(LoginError::Network(NetworkError::HttpError {
            status: status.as_u16(),
            url: url.to_string(),
        }));
    }

    let body = resp.text().await.map_err(|e| map_reqwest_error(url, e))?;

    if is_auth_url(url) {
        tracing::info!(
            url,
            %status,
            final_url = %final_url,
            location = %location,
            response_len = body.len(),
            preview = %preview_text(&body, 220),
            "HTTP GET success"
        );
    }

    Ok(body)
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

/// The game-start page URL for one service account.
///
/// Built in one place because it is easy to get wrong in a way nothing catches:
/// a line-continued format string that loses its backslash leaves literal
/// spaces in the middle of the query, and beanfun answers that with its generic
/// error page rather than anything that says what was wrong.
fn game_start_step2_url(host: &str, sotp: &str, timestamp: &str) -> String {
    format!(
        "https://{host}/beanfun_block/game_zone/game_start_step2.aspx\
         ?service_code={DEFAULT_SERVICE_CODE}&service_region={DEFAULT_SERVICE_REGION}\
         &sotp={sotp}&dt={timestamp}"
    )
}

/// The substitution tables the game manager's payload decoder uses.
///
/// The first character of the payload is a hex digit that picks one of these
/// and also says where in the result the DES key sits. Every other character
/// maps to its index in the chosen table, which turns the payload into ordinary
/// hex.
const TICKET_TABLES: [&str; 8] = [
    "bac987d65e432f10",
    "3bc4d5e6f2a79108",
    "cdbeaf9012456378",
    "4e6fb81a3c5d7092",
    "bdef1246789ac530",
    "5f82cb4093e71d6a",
    "df1468ace0357b92",
    "b50c61a4f93e82d7",
];

/// What the game-start page's payload carries once decoded.
///
/// Not credentials — a ticket that stands in for them, which the credential
/// endpoint trades for the real thing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchTicket {
    pub launch_ticket: String,
    pub service_account: String,
}

/// Decode the game-start payload into its launch ticket.
///
/// It looks encrypted to a key we don't have, and isn't: the key travels with
/// the payload, eight characters at an offset the payload's first character
/// names, and the rest is DES-ECB with no padding — the same cipher the
/// credential response has always used.
///
/// Which table that first character selects is not settled. `n % 4` decodes
/// every payload seen so far, but there are eight tables, and a payload whose
/// selector is 12 rules out `n % 8` while another selector might rule out
/// `n % 4`. Rather than pick a rule and be wrong for some accounts, each table
/// is tried until one yields a plaintext holding a `LaunchTicket` — an answer
/// that can't be reached by accident, since a wrong table gives noise. Eight
/// DES passes over 272 bytes costs nothing measurable, and it keeps working if
/// beanfun adds a ninth table.
fn decode_launch_ticket(data: &str) -> Option<LaunchTicket> {
    let selector = usize::from_str_radix(data.get(..1)?, 16).ok()?;
    let body = data.get(1..)?;

    // Most likely first, so the log usually names the same one.
    let mut order: Vec<usize> = vec![selector % 4, selector % TICKET_TABLES.len()];
    order.extend(0..TICKET_TABLES.len());
    order.dedup();

    let mut tried = Vec::new();
    for index in order {
        if tried.contains(&index) {
            continue;
        }
        tried.push(index);
        if let Some(ticket) = decode_with_table(body, selector, TICKET_TABLES[index]) {
            tracing::debug!(selector, table = index, "TW: launch ticket table");
            return Some(ticket);
        }
    }
    None
}

/// One attempt at decoding, with a table already chosen.
fn decode_with_table(body: &str, selector: usize, table: &str) -> Option<LaunchTicket> {
    let mut normalized = String::with_capacity(body.len());
    for c in body.chars() {
        normalized.push(std::char::from_digit(table.find(c)? as u32, 16)?);
    }

    let key_at = selector + 1;
    let key = normalized.get(key_at..key_at + 8)?;
    let ciphertext = format!(
        "{}{}",
        normalized.get(..key_at)?,
        normalized.get(key_at + 8..)?
    );

    let plaintext = des_ecb_decrypt_hex(&ciphertext, key)?;
    let field = |name: &str| -> Option<String> {
        plaintext
            .split(['&', ';'])
            .find_map(|kv| kv.trim().strip_prefix(&format!("{name}=")))
            .map(str::to_string)
    };

    // The field that says this decoded rather than merely decrypted.
    Some(LaunchTicket {
        launch_ticket: field("LaunchTicket")?,
        service_account: field("ServiceAccount").unwrap_or_default(),
    })
}

/// Trade a launch ticket for the game credentials.
///
/// The endpoint the game manager uses. It states which build is asking — a
/// version and the hash of one of the manager's files — and beanfun refuses
/// anything that doesn't. The answer is JSON, but the payload inside it is the
/// same eight characters of DES key followed by ciphertext that the older
/// endpoint returned, so only the wrapper is new.
async fn tw_otp_from_launch_ticket(
    client: &Client,
    sn: &str,
    ticket: &LaunchTicket,
) -> Result<GameCredentials, LoginError> {
    let integrity = crate::services::ggm_hotfix::client_integrity(client).await;
    let url = format!("https://{TW_HOST}/beanfun_block/generic_handlers/get_webstart_otp_v2.ashx");
    let body = serde_json::json!({
        "SN": sn,
        "LaunchTicket": ticket.launch_ticket,
        "CV": integrity.cv,
        "Hash": integrity.hash,
        "arch": integrity.arch,
    });

    let response = client
        .post(&url)
        .header("User-Agent", USER_AGENT)
        .json(&body)
        .send()
        .await
        .map_err(|e| map_reqwest_error(&url, e))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| map_reqwest_error(&url, e))?;
    if !status.is_success() {
        tracing::error!(%status, head = %preview_text(&text, 200), "TW: v2 OTP request failed");
        return Err(LoginError::Network(NetworkError::HttpError {
            status: status.as_u16(),
            url,
        }));
    }

    let body: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        tracing::error!(
            bytes = text.len(),
            head = %preview_text(&text, 300),
            "TW: v2 OTP response is not JSON"
        );
        parse_error_str("OTP response format invalid")
    })?;

    if body["result"].as_i64() != Some(1) {
        let message = body["message"].as_str().unwrap_or_default();
        tracing::error!(result = ?body["result"], message, "TW: v2 OTP request refused");
        return Err(otp_failure_error(message));
    }

    let payload = body["data"]
        .as_str()
        .ok_or_else(|| parse_error_str("OTP response carried no data"))?;
    let otp = decrypt_otp_payload(payload)?;
    Ok(tw_credentials(&ticket.service_account, otp))
}

/// Fetch TW credentials the way the game manager does.
///
/// One page, one POST. The game-start page carries a payload that decodes —
/// with no secret of beanfun's — into a launch ticket, and the credential
/// endpoint trades that ticket for the real credentials. Nothing is launched,
/// no registry key is touched, and the one-time password comes back here rather
/// than through a second process.
async fn tw_get_otp_v2(
    client: &Client,
    session: &Session,
    account_id: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    known_accounts: &[GameAccount],
) -> Result<GameCredentials, LoginError> {
    let ticket = tw_ggm_ticket(client, session, account_id, cookie_jar, known_accounts).await?;
    let launch = decode_launch_ticket(&ticket.data)
        .ok_or_else(|| parse_error_str("could not read the launch ticket"))?;
    tracing::info!(account_id, sn = %ticket.sn, "TW: launch ticket decoded, requesting credentials");

    let mut credentials = tw_otp_from_launch_ticket(client, &ticket.sn, &launch).await?;
    // The page reports the account it issued the ticket for; keep the caller's
    // id, which is what the rest of the app keys on.
    credentials.account_id = account_id.to_string();
    tracing::info!(account_id, "TW OTP retrieved");
    Ok(credentials)
}

/// The launch ticket beanfun's game-start page hands to the Gamania Games
/// Manager.
///
/// TW no longer fetches credentials from the web at all: the page emits an
/// `m_objData` object and passes it to GGM over a `gamaniagames://` URL, and
/// GGM — which can prove its own identity to beanfun's integrity check — does
/// the credential fetch and starts the game. `data` is opaque here; only GGM
/// holds the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GgmTicket {
    pub region: String,
    pub sn: String,
    pub data: String,
}

impl GgmTicket {
    /// The URL the page itself opens, byte-for-byte.
    ///
    /// `Cmd=06006` is SmartLaunch — install if needed, then start. The odd
    /// `&&&&` separator is GGM's, not a typo. The page omits `WebToken` and
    /// `SecretCode` (it takes `ggm.js`'s no-token branch), so this does too.
    pub fn launch_uri(&self) -> String {
        format!(
            "gamaniagames://Region={}&&&&SN={}&&&&Cmd=06006&&&&Data={}",
            self.region, self.sn, self.data
        )
    }
}

/// Read the launch ticket out of a game-start page.
fn extract_ggm_ticket(html: &str) -> Option<GgmTicket> {
    let field = |name: &str| -> Option<String> {
        let re = Regex::new(&format!(
            r#"(?s)m_objData\s*=\s*\{{.*?"{name}"\s*:\s*"([^"]*)""#
        ))
        .ok()?;
        Some(re.captures(html)?.get(1)?.as_str().to_string())
    };
    let ticket = GgmTicket {
        region: field("region")?,
        sn: field("sn")?,
        data: field("data")?,
    };
    (!ticket.sn.is_empty() && !ticket.data.is_empty()).then_some(ticket)
}

/// Turn an encrypted credential payload into game credentials.
///
/// The first eight characters are the DES key for the rest.
fn decrypt_otp_payload(payload: &str) -> Result<String, LoginError> {
    if payload.len() < 8 {
        return Err(parse_error_str("OTP data too short for DES key"));
    }
    des_ecb_decrypt_hex(&payload[8..], &payload[..8]).ok_or_else(|| {
        LoginError::Auth(AuthError::InvalidCredentials {
            reason: "OTP decryption failed".into(),
        })
    })
}

/// The anti-forgery pair the game-start page appends to its `record_service_start`
/// form, as (field name, value).
///
/// It is neither a hidden input nor a fixed name — the page builds the form body
/// in JavaScript and ends it with a random-named field, a different name on
/// every load (`et2eylbcup3aztj4xuayb4fn`, `0hiwgz55odgekffwp5x5yt55`, …). What
/// pins it down is its position: the last parameter of that string, right after
/// `service_account_create_time`.
fn extract_service_start_token(html: &str) -> Option<(String, String)> {
    let re = Regex::new(
        r#"service_account_create_time=["'\s+]*\+?\s*MyAccountData\.ServiceAccountCreateTime\s*\+\s*"&([A-Za-z0-9_]+)=([^"]+)""#,
    )
    .ok()?;
    let caps = re.captures(html)?;
    Some((
        caps.get(1)?.as_str().to_string(),
        caps.get(2)?.as_str().to_string(),
    ))
}

/// Map a `reqwest::Error` into our domain [`NetworkError`].
fn map_reqwest_error(url: &str, err: reqwest::Error) -> LoginError {
    tracing::error!(
        url,
        is_timeout = err.is_timeout(),
        is_connect = err.is_connect(),
        is_status = err.is_status(),
        source = %err,
        "HTTP request failed"
    );

    if err.is_timeout() {
        LoginError::Network(NetworkError::Timeout {
            url: format!("{url} ({err})"),
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

fn is_auth_url(url: &str) -> bool {
    url.contains("/beanfun_block/bflogin/")
        || url.contains("login.beanfun.com")
        || url.contains("login.hk.beanfun.com")
        || url.contains("tw.newlogin.beanfun.com")
}

fn preview_text(text: &str, max_chars: usize) -> String {
    let preview: String = text.chars().take(max_chars).collect();
    preview.replace(['\r', '\n', '\t'], " ")
}

fn mask_account(account: &str) -> String {
    let mut chars = account.chars();
    match (chars.next(), chars.last()) {
        (Some(first), Some(last)) if account.chars().count() > 2 => {
            format!("{first}***{last}")
        }
        _ if account.is_empty() => "<empty>".to_string(),
        _ => "***".to_string(),
    }
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

    /// The parts of `game_start_step2.aspx` this module reads, copied from a
    /// real page (payload and token shortened, shape kept).
    const GAME_START_PAGE: &str = r#"
        var MyAccountData = {ServiceCode: "610074", ServiceRegion: "T9",
            ServiceAccountCreateTime: "2012-02-16 18:30:07" };
        var m_objData = {
            "region": "TW;Production",
            "sn": "e612ef7c-dba7-414d-b789-1be5f784c334",
            "data": "ccb8f7e5f917451127b189251d22ea8c7f2d6a0ffc9e69ab"
        };
        var strFormData = "service_code=" + MyAccountData.ServiceCode +
        "&service_account_display_name=" + MyAccountData.ServiceAccountDisplayName + "&service_account_create_time=" + MyAccountData.ServiceAccountCreateTime + "&et2eylbcup3aztj4xuayb4fn=pbrVfA6GULglvDfgMk%2fUlZmF8BgO34hi5hAYAeNjQx4%3d";
    "#;

    #[test]
    fn the_game_start_url_has_no_stray_whitespace() {
        let url = game_start_step2_url("tw.beanfun.com", "1845983", "2026717141832357");
        // The bug this guards: a line-continued literal that lost its backslash
        // put spaces inside the query, and beanfun answered with its generic
        // error page — 2 KB of "程式發生錯誤" that parses as a page holding
        // nothing, so the failure surfaced nowhere near its cause.
        assert!(!url.contains(' '), "stray whitespace in {url}");
        assert_eq!(
            url,
            concat!(
                "https://tw.beanfun.com/beanfun_block/game_zone/game_start_step2.aspx",
                "?service_code=610074&service_region=T9&sotp=1845983&dt=2026717141832357",
            )
        );
    }

    /// Build a payload the way beanfun does, so the decoder is tested against
    /// the encoding rather than against a recorded secret.
    fn encode_launch_payload(plaintext: &str, key: &str, selector: usize) -> String {
        encode_launch_payload_with(plaintext, key, selector, TICKET_TABLES[selector % 4])
    }

    /// As above, with the table stated rather than derived.
    fn encode_launch_payload_with(
        plaintext: &str,
        key: &str,
        selector: usize,
        table: &str,
    ) -> String {
        use des::cipher::{BlockCipherEncrypt, KeyInit};

        let cipher = des::Des::new_from_slice(key.as_bytes()).unwrap();
        let mut padded = plaintext.as_bytes().to_vec();
        padded.resize(padded.len().div_ceil(8) * 8, 0);
        for chunk in padded.as_chunks_mut::<8>().0 {
            let block: &mut des::cipher::Array<u8, _> = chunk.as_mut_slice().try_into().unwrap();
            cipher.encrypt_block(block);
        }
        let cipher_hex: String = padded.iter().map(|b| format!("{b:02x}")).collect();

        // The key sits at `selector + 1` in the normalized hex.
        let key_at = selector + 1;
        let normalized = format!("{}{key}{}", &cipher_hex[..key_at], &cipher_hex[key_at..]);

        let body: String = normalized
            .chars()
            .map(|c| table.chars().nth(c.to_digit(16).unwrap() as usize).unwrap())
            .collect();
        format!("{:x}{body}", selector % 16)
    }

    #[test]
    fn decodes_a_launch_ticket() {
        // A ticket of the real width, built rather than typed out, so the test
        // can't disagree with itself about how long 64 characters is.
        let ticket_value: String = std::iter::repeat_n('a', 64).collect();
        let account = "T96581594b9840873995";
        let plaintext = format!(
            "LaunchTicket={ticket_value}&ServiceCode=610074&ServiceRegion=T9&ServiceAccount={account}"
        );
        // The key is eight characters cut out of the normalized hex, so it is
        // always hex itself — a non-hex key could never occur.
        let data = encode_launch_payload(&plaintext, "eec50e43", 12);

        let ticket = decode_launch_ticket(&data).expect("decodes");
        assert_eq!(ticket.launch_ticket, ticket_value);
        assert_eq!(ticket.service_account, account);
    }

    #[test]
    fn any_table_decodes_whatever_the_selector() {
        // Which table a selector picks is not settled — `n % 4` fits every
        // payload seen, but there are eight tables. The decoder answers that by
        // trying them, so encode with each table in turn, against selectors
        // that agree with neither rule, and require every one to come back.
        for (index, table) in TICKET_TABLES.iter().enumerate() {
            for selector in [index, index + 8, index + 3] {
                let data = encode_launch_payload_with(
                    "LaunchTicket=abc&ServiceAccount=acct",
                    "abcdef12",
                    selector,
                    table,
                );
                let ticket = decode_launch_ticket(&data).unwrap_or_else(|| {
                    panic!("table {index} with selector {selector} failed to decode")
                });
                assert_eq!(ticket.launch_ticket, "abc");
                assert_eq!(ticket.service_account, "acct");
            }
        }
    }

    #[test]
    fn a_payload_that_decrypts_to_nothing_useful_is_rejected() {
        // A wrong table still produces bytes; only the absence of a ticket in
        // them says the attempt failed. Without that check the first table
        // tried would always "succeed".
        let data = encode_launch_payload("NotATicket=abc", "abcdef12", 3);
        assert!(decode_launch_ticket(&data).is_none());
    }

    #[test]
    fn rubbish_decodes_to_nothing_rather_than_panicking() {
        assert!(decode_launch_ticket("").is_none());
        assert!(decode_launch_ticket("z").is_none());
        // Characters outside the chosen table can't be mapped.
        assert!(decode_launch_ticket("0zzzz").is_none());
    }

    #[test]
    fn the_v2_payload_splits_into_a_key_and_whole_blocks() {
        // A real reply's payload, values replaced: 40 hex characters, the first
        // eight being the DES key and the rest one 16-byte ciphertext — the
        // same shape the older endpoint returned after its `1;`.
        let data = "18103D179B09C6BEB8A4A37B9037A44E599760D8";
        let (key, cipher) = data.split_at(8);
        assert_eq!(key.len(), 8);
        assert_eq!(cipher.len() % 16, 0, "ciphertext is whole DES blocks");
        assert!(decrypt_otp_payload("short").is_err());
    }

    #[test]
    fn reads_the_ggm_launch_ticket() {
        let ticket = extract_ggm_ticket(GAME_START_PAGE).unwrap();
        assert_eq!(ticket.region, "TW;Production");
        assert_eq!(ticket.sn, "e612ef7c-dba7-414d-b789-1be5f784c334");
        assert_eq!(
            ticket.data,
            "ccb8f7e5f917451127b189251d22ea8c7f2d6a0ffc9e69ab"
        );
    }

    #[test]
    fn the_launch_uri_matches_what_the_page_opens() {
        // Byte-for-byte the URL beanfun's own page hands to GGM, `&&&&`
        // separators and all — captured from a live launch.
        let ticket = extract_ggm_ticket(GAME_START_PAGE).unwrap();
        assert_eq!(
            ticket.launch_uri(),
            concat!(
                "gamaniagames://Region=TW;Production",
                "&&&&SN=e612ef7c-dba7-414d-b789-1be5f784c334",
                "&&&&Cmd=06006",
                "&&&&Data=ccb8f7e5f917451127b189251d22ea8c7f2d6a0ffc9e69ab",
            )
        );
    }

    #[test]
    fn a_page_without_a_ticket_yields_none() {
        assert!(extract_ggm_ticket("<html>no launch data</html>").is_none());
    }

    #[test]
    fn reads_the_service_start_token_despite_its_random_name() {
        let (name, value) = extract_service_start_token(GAME_START_PAGE).unwrap();
        // The name differs on every page load, so it can only be found by
        // position — the field the form body ends with.
        assert_eq!(name, "et2eylbcup3aztj4xuayb4fn");
        assert_eq!(value, "pbrVfA6GULglvDfgMk%2fUlZmF8BgO34hi5hAYAeNjQx4%3d");
    }

    #[test]
    fn a_page_without_the_token_yields_none() {
        assert!(extract_service_start_token("<html>nothing here</html>").is_none());
    }

    #[test]
    fn a_payload_too_short_to_hold_a_key_is_rejected() {
        assert!(decrypt_otp_payload("abc").is_err());
    }

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
    fn extract_registered_device_continuation_parses_login_token() {
        let body = r#"<script>pollRequest("請至手機確認","ABC123TOKEN","callback.aspx?akey=HELLO");</script>"#;
        let continuation = extract_registered_device_continuation(body).unwrap();
        assert_eq!(continuation.login_token, "ABC123TOKEN");
    }

    #[test]
    fn normalize_registered_device_callback_url_supports_relative_path() {
        let url = normalize_registered_device_callback_url("callback.aspx?akey=HELLO");
        assert_eq!(
            url,
            "https://tw.newlogin.beanfun.com/login/callback.aspx?akey=HELLO"
        );
    }

    #[test]
    fn extract_tw_session_key_from_redirect_location() {
        let url =
            "https://tw.newlogin.beanfun.com/checkin.aspx?skey=2026059cc9e9a0c63141&display_mode=0";
        assert_eq!(
            extract_tw_session_key(url).as_deref(),
            Some("2026059cc9e9a0c63141")
        );
    }

    #[test]
    fn extract_tw_session_key_from_login_index_url() {
        let url = "https://login.beanfun.com/Login/Index?pSKey=ABC123xyz";
        assert_eq!(extract_tw_session_key(url).as_deref(), Some("ABC123xyz"));
    }

    #[test]
    fn extract_tw_session_key_missing_on_block_page() {
        assert!(extract_tw_session_key("https://tw.beanfun.com/TW/BlockIPMessage.htm").is_none());
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
