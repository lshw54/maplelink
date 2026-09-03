//! Pure authentication logic — session validation and input checks. No side effects.

use crate::core::error::AuthError;
use crate::models::session::Session;

/// Maximum allowed length for user-supplied string inputs (account, password, codes).
pub const MAX_INPUT_LENGTH: usize = 256;

/// Validate that a session exists.
/// Returns the session reference on success.
///
/// Does not check expires_at — session validity is managed server-side via ping keep-alive.
pub fn require_valid_session(session: &Option<Session>) -> Result<&Session, AuthError> {
    match session {
        None => Err(AuthError::NotAuthenticated),
        Some(s) => Ok(s),
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session() -> Session {
        Session {
            token: "tok_test".into(),
            expires_at: chrono::Utc::now(),
            region: crate::models::session::Region::HK,
            account_name: "test_user".into(),
            session_key: None,
            totp_state: None,
        }
    }

    #[test]
    fn require_valid_session_none() {
        assert!(matches!(
            require_valid_session(&None),
            Err(AuthError::NotAuthenticated)
        ));
    }

    #[test]
    fn require_valid_session_with_old_expiry_still_works() {
        // Session with past expires_at should still be valid
        // (we rely on server-side expiration, not local)
        let s = make_session();
        assert!(require_valid_session(&Some(s)).is_ok());
    }

    #[test]
    fn require_valid_session_ok() {
        let s = make_session();
        assert!(require_valid_session(&Some(s)).is_ok());
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
