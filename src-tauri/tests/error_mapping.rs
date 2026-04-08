//! Feature: maplelink-rewrite, Property 12: Backend errors map to valid ErrorDto with category
//!
//! For any domain error type, converting to ErrorDto produces a non-empty code,
//! non-empty message, and correct category.

use maplelink_lib::core::error::*;
use maplelink_lib::models::error::{ErrorCategory, ErrorDto};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Arbitrary generators for each error type
// ---------------------------------------------------------------------------

fn arb_auth_error() -> impl Strategy<Value = AuthError> {
    prop_oneof![
        any::<String>().prop_map(|reason| AuthError::InvalidCredentials { reason }),
        Just(AuthError::SessionExpired),
        Just(AuthError::TotpFailed),
        Just(AuthError::QrExpired),
        Just(AuthError::NotAuthenticated),
    ]
}

fn arb_network_error() -> impl Strategy<Value = NetworkError> {
    prop_oneof![
        any::<String>().prop_map(|url| NetworkError::ConnectionFailed { url }),
        any::<String>().prop_map(|url| NetworkError::Timeout { url }),
        (any::<u16>(), any::<String>())
            .prop_map(|(status, url)| NetworkError::HttpError { status, url }),
    ]
}

fn arb_fs_error() -> impl Strategy<Value = FsError> {
    prop_oneof![
        any::<String>().prop_map(|path| FsError::NotFound { path }),
        any::<String>().prop_map(|path| FsError::PermissionDenied { path }),
        (any::<String>(), any::<String>()).prop_map(|(path, reason)| FsError::Io { path, reason }),
    ]
}

fn arb_process_error() -> impl Strategy<Value = ProcessError> {
    prop_oneof![(any::<String>(), any::<String>())
        .prop_map(|(path, reason)| ProcessError::SpawnFailed { path, reason }),]
}

fn arb_config_error() -> impl Strategy<Value = ConfigError> {
    prop_oneof![
        any::<String>().prop_map(|reason| ConfigError::ParseError { reason }),
        any::<String>().prop_map(|reason| ConfigError::WriteError { reason }),
    ]
}

fn arb_update_error() -> impl Strategy<Value = UpdateError> {
    prop_oneof![
        any::<String>().prop_map(|reason| UpdateError::CheckFailed { reason }),
        any::<String>().prop_map(|reason| UpdateError::DownloadFailed { reason }),
        Just(UpdateError::CorruptDownload),
    ]
}

fn arb_app_error() -> impl Strategy<Value = AppError> {
    prop_oneof![
        arb_auth_error().prop_map(AppError::Auth),
        arb_network_error().prop_map(AppError::Network),
        arb_fs_error().prop_map(AppError::FileSystem),
        arb_process_error().prop_map(AppError::Process),
        arb_config_error().prop_map(AppError::Config),
        arb_update_error().prop_map(AppError::Update),
    ]
}

// ---------------------------------------------------------------------------
// Property test
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn error_dto_has_nonempty_code_message_and_correct_category(err in arb_app_error()) {
        // Determine expected category before conversion (moves err).
        let expected_category = match &err {
            AppError::Auth(_) => ErrorCategory::Authentication,
            AppError::Network(_) => ErrorCategory::Network,
            AppError::FileSystem(_) => ErrorCategory::FileSystem,
            AppError::Process(_) => ErrorCategory::Process,
            AppError::Config(_) => ErrorCategory::Configuration,
            AppError::Update(_) => ErrorCategory::Update,
        };

        let dto: ErrorDto = err.into();

        prop_assert!(!dto.code.is_empty(), "code must be non-empty");
        prop_assert!(!dto.message.is_empty(), "message must be non-empty");
        prop_assert_eq!(dto.category, expected_category);
    }
}
