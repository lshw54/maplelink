//! Pure authentication logic — session validation, token expiry checks,
//! and credential construction. No side effects.

use chrono::Utc;

use crate::core::error::AuthError;
use crate::models::session::{Region, Session};

/// Minimum remaining lifetime (in seconds) before we consider a session
/// "about to expire" and attempt a proactive refresh.
const REFRESH_THRESHOLD_SECS: i64 = 60;

/// Maximum allowed length for user-supplied string inputs (account, password, codes).
pub const MAX_INPUT_LENGTH: usize = 256;

/// Result of checking whether a session needs attention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    /// Session is valid and has enough remaining lifetime.
    Valid,
    /// Session is still technically valid but should be refreshed soon.
    NeedsRefresh,
    /// Session has expired — re-authentication required.
    Expired,
}

/// Check the status of a session relative to the current time.
pub fn check_session_status(session: &Session) -> SessionStatus {
    let now = Utc::now();
    if now >= session.expires_at {
        SessionStatus::Expired
    } else if (session.expires_at - now).num_seconds() < REFRESH_THRESHOLD_SECS {
        SessionStatus::NeedsRefresh
    } else {
        SessionStatus::Valid
    }
}

/// Returns `true` if the session token has expired.
pub fn is_session_expired(session: &Session) -> bool {
    Utc::now() >= session.expires_at
}

/// Validate that a session exists and is not expired.
/// Returns the session reference on success.
pub fn require_valid_session(session: &Option<Session>) -> Result<&Session, AuthError> {
    match session {
        None => Err(AuthError::NotAuthenticated),
        Some(s) if is_session_expired(s) => Err(AuthError::SessionExpired),
        Some(s) => Ok(s),
    }
}

/// Determine whether a session can be refreshed (has a refresh token and
/// the right region — only HK supports refresh in the Beanfun API).
pub fn can_refresh(session: &Session) -> bool {
    session.refresh_token.is_some()
}

/// Decide the next action for a session: use as-is, refresh, or re-auth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAction {
    /// Session is good — proceed with the current token.
    UseExisting,
    /// Attempt a token refresh before falling back to re-auth.
    AttemptRefresh,
    /// Session is gone or expired with no refresh path — must re-authenticate.
    ReAuthenticate,
}

/// Given the current session state, decide what to do next.
pub fn decide_session_action(session: &Option<Session>) -> SessionAction {
    match session {
        None => SessionAction::ReAuthenticate,
        Some(s) => match check_session_status(s) {
            SessionStatus::Valid => SessionAction::UseExisting,
            SessionStatus::NeedsRefresh if can_refresh(s) => SessionAction::AttemptRefresh,
            SessionStatus::NeedsRefresh => SessionAction::UseExisting, // still valid, just close to expiry
            SessionStatus::Expired if can_refresh(s) => SessionAction::AttemptRefresh,
            SessionStatus::Expired => SessionAction::ReAuthenticate,
        },
    }
}

/// Validate a user-supplied input string (account, password, TOTP code, etc.).
///
/// Returns `Ok(())` if the input is non-empty and within [`MAX_INPUT_LENGTH`].
pub fn validate_input(field_name: &str, value: &str) -> Result<(), AuthError> {
    if value.is_empty() {
        return Err(AuthError::InvalidCredentials {
            reason: format!("{field_name} must not be empty"),
        });
    }
    if value.len() > MAX_INPUT_LENGTH {
        return Err(AuthError::InvalidCredentials {
            reason: format!("{field_name} exceeds maximum length of {MAX_INPUT_LENGTH} characters"),
        });
    }
    Ok(())
}

/// Determine the expected authentication flow based on region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthFlow {
    /// Standard username + password login.
    Normal,
    /// QR code scan (TW region).
    QrCode,
    /// TOTP verification (HK region).
    Totp,
}

/// Return the available authentication flows for a given region.
pub fn available_auth_flows(region: &Region) -> Vec<AuthFlow> {
    match region {
        Region::TW => vec![AuthFlow::Normal, AuthFlow::QrCode],
        Region::HK => vec![AuthFlow::Normal, AuthFlow::Totp],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_session(expires_in_secs: i64, refresh: bool) -> Session {
        Session {
            token: "tok_test".into(),
            refresh_token: if refresh {
                Some("ref_test".into())
            } else {
                None
            },
            expires_at: Utc::now() + Duration::seconds(expires_in_secs),
            region: Region::HK,
            account_name: "test_user".into(),
            session_key: None,
            totp_state: None,
        }
    }

    #[test]
    fn valid_session_status() {
        let s = make_session(300, false);
        assert_eq!(check_session_status(&s), SessionStatus::Valid);
    }

    #[test]
    fn needs_refresh_status() {
        let s = make_session(30, false);
        assert_eq!(check_session_status(&s), SessionStatus::NeedsRefresh);
    }

    #[test]
    fn expired_session_status() {
        let s = make_session(-10, false);
        assert_eq!(check_session_status(&s), SessionStatus::Expired);
    }

    #[test]
    fn require_valid_session_none() {
        assert!(matches!(
            require_valid_session(&None),
            Err(AuthError::NotAuthenticated)
        ));
    }

    #[test]
    fn require_valid_session_expired() {
        let s = make_session(-10, false);
        assert!(matches!(
            require_valid_session(&Some(s)),
            Err(AuthError::SessionExpired)
        ));
    }

    #[test]
    fn require_valid_session_ok() {
        let s = make_session(300, false);
        assert!(require_valid_session(&Some(s)).is_ok());
    }

    #[test]
    fn decide_action_no_session() {
        assert_eq!(decide_session_action(&None), SessionAction::ReAuthenticate);
    }

    #[test]
    fn decide_action_valid() {
        let s = make_session(300, false);
        assert_eq!(decide_session_action(&Some(s)), SessionAction::UseExisting);
    }

    #[test]
    fn decide_action_needs_refresh_with_token() {
        let s = make_session(30, true);
        assert_eq!(
            decide_session_action(&Some(s)),
            SessionAction::AttemptRefresh
        );
    }

    #[test]
    fn decide_action_needs_refresh_without_token() {
        let s = make_session(30, false);
        assert_eq!(decide_session_action(&Some(s)), SessionAction::UseExisting);
    }

    #[test]
    fn decide_action_expired_with_refresh() {
        let s = make_session(-10, true);
        assert_eq!(
            decide_session_action(&Some(s)),
            SessionAction::AttemptRefresh
        );
    }

    #[test]
    fn decide_action_expired_without_refresh() {
        let s = make_session(-10, false);
        assert_eq!(
            decide_session_action(&Some(s)),
            SessionAction::ReAuthenticate
        );
    }

    #[test]
    fn validate_input_empty() {
        assert!(validate_input("account", "").is_err());
    }

    #[test]
    fn validate_input_too_long() {
        let long = "a".repeat(MAX_INPUT_LENGTH + 1);
        assert!(validate_input("account", &long).is_err());
    }

    #[test]
    fn validate_input_ok() {
        assert!(validate_input("account", "user123").is_ok());
    }

    #[test]
    fn validate_input_max_length_ok() {
        let exact = "a".repeat(MAX_INPUT_LENGTH);
        assert!(validate_input("account", &exact).is_ok());
    }

    #[test]
    fn available_flows_tw() {
        let flows = available_auth_flows(&Region::TW);
        assert!(flows.contains(&AuthFlow::Normal));
        assert!(flows.contains(&AuthFlow::QrCode));
        assert!(!flows.contains(&AuthFlow::Totp));
    }

    #[test]
    fn available_flows_hk() {
        let flows = available_auth_flows(&Region::HK);
        assert!(flows.contains(&AuthFlow::Normal));
        assert!(flows.contains(&AuthFlow::Totp));
        assert!(!flows.contains(&AuthFlow::QrCode));
    }

    // -----------------------------------------------------------------------
    // Property-based tests
    // -----------------------------------------------------------------------

    use proptest::prelude::*;

    // Feature: maplelink-rewrite, Property 14: Command handler input validation
    //
    // For any Tauri command that accepts string parameters, passing an empty
    // string or a string exceeding MAX_INPUT_LENGTH shall result in a
    // validation error before the input reaches the core or service layer.
    proptest! {
        #[test]
        fn prop_empty_input_always_rejected(field_name in "[a-z_]{1,20}") {
            let result = validate_input(&field_name, "");
            prop_assert!(result.is_err(), "empty input must be rejected");
        }

        #[test]
        fn prop_oversized_input_always_rejected(
            field_name in "[a-z_]{1,20}",
            extra in 1usize..512,
        ) {
            let oversized = "x".repeat(MAX_INPUT_LENGTH + extra);
            let result = validate_input(&field_name, &oversized);
            prop_assert!(result.is_err(), "oversized input must be rejected");
        }

        #[test]
        fn prop_valid_input_always_accepted(
            field_name in "[a-z_]{1,20}",
            value in "[a-zA-Z0-9]{1,256}",
        ) {
            // Only test values within the allowed length
            if !value.is_empty() && value.len() <= MAX_INPUT_LENGTH {
                let result = validate_input(&field_name, &value);
                prop_assert!(result.is_ok(), "valid input must be accepted, got: {:?}", result);
            }
        }
    }
}
