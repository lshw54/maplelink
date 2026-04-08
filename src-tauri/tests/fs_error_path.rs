//! Feature: maplelink-rewrite, Property 13: File system errors include the relevant path
//!
//! For any FsError with a file path, the resulting ErrorDto's message or details
//! field contains that path as a substring.

use maplelink_lib::core::error::{AppError, FsError};
use maplelink_lib::models::error::ErrorDto;
use proptest::prelude::*;

fn arb_fs_error() -> impl Strategy<Value = FsError> {
    prop_oneof![
        "\\PC{1,200}".prop_map(|path| FsError::NotFound { path }),
        "\\PC{1,200}".prop_map(|path| FsError::PermissionDenied { path }),
        ("\\PC{1,200}", any::<String>()).prop_map(|(path, reason)| FsError::Io { path, reason }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn fs_error_dto_contains_path(err in arb_fs_error()) {
        let path = match &err {
            FsError::NotFound { path }
            | FsError::PermissionDenied { path }
            | FsError::Io { path, .. } => path.clone(),
        };

        let dto: ErrorDto = AppError::FileSystem(err).into();

        let in_message = dto.message.contains(&path);
        let in_details = dto.details.as_ref().is_some_and(|d| d.contains(&path));

        prop_assert!(
            in_message || in_details,
            "path {:?} not found in message {:?} or details {:?}",
            path, dto.message, dto.details
        );
    }
}
