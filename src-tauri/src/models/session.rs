//! Session and region models for Beanfun authentication.

use serde::{Deserialize, Serialize};

/// Authenticated Beanfun session containing tokens and expiry information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub token: String,
    pub refresh_token: Option<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub region: Region,
    pub account_name: String,
    /// The session key obtained from `GetSessionkey()` (HK: parsed from HTML span).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
    /// Intermediate TOTP state saved when login returns `need_totp`.
    /// Holds the HTML response body and the POST URL needed for TOTP submission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totp_state: Option<TotpState>,
}

/// Intermediate state saved when HK login requires TOTP verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpState {
    /// The HTML response body containing viewstate fields.
    pub response_html: String,
    /// The URL to POST the TOTP form to.
    pub post_url: String,
}

/// Beanfun platform region.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Region {
    TW,
    HK,
}
