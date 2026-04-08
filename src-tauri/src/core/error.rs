//! Domain error types using `thiserror` and mapping to serializable [`ErrorDto`].

use crate::models::error::{ErrorCategory, ErrorDto};

/// Top-level application error that wraps all domain-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Network(#[from] NetworkError),
    #[error(transparent)]
    FileSystem(#[from] FsError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Update(#[from] UpdateError),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials: {reason}")]
    InvalidCredentials { reason: String },
    #[error("Session expired")]
    SessionExpired,
    #[error("TOTP verification required")]
    TotpRequired {
        /// Partial session containing the TOTP state needed for verification.
        partial_session: Box<crate::models::session::Session>,
    },
    #[error("TOTP verification failed")]
    TotpFailed,
    #[error("QR code expired")]
    QrExpired,
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Advance check verification required")]
    AdvanceCheckRequired {
        /// Optional URL for the advance check page.
        url: Option<String>,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum NetworkError {
    #[error("Connection failed: {url}")]
    ConnectionFailed { url: String },
    #[error("Request timeout: {url}")]
    Timeout { url: String },
    #[error("HTTP error {status}: {url}")]
    HttpError { status: u16, url: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum FsError {
    #[error("File not found: {path}")]
    NotFound { path: String },
    #[error("Permission denied: {path}")]
    PermissionDenied { path: String },
    #[error("IO error on {path}: {reason}")]
    Io { path: String, reason: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProcessError {
    #[error("Failed to start process: {path} — {reason}")]
    SpawnFailed { path: String, reason: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to parse config: {reason}")]
    ParseError { reason: String },
    #[error("Failed to write config: {reason}")]
    WriteError { reason: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum UpdateError {
    #[error("Update check failed: {reason}")]
    CheckFailed { reason: String },
    #[error("Download failed: {reason}")]
    DownloadFailed { reason: String },
    #[error("Update file corrupt")]
    CorruptDownload,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the variant name from a `thiserror` enum as UPPER_SNAKE_CASE.
fn error_variant_name<E: std::fmt::Debug>(err: &E) -> String {
    let dbg = format!("{err:?}");
    // Debug output starts with the variant name, e.g. `InvalidCredentials { ... }`
    let variant = dbg.split([' ', '(']).next().unwrap_or("UNKNOWN");
    // Convert PascalCase to UPPER_SNAKE_CASE
    let mut result = String::with_capacity(variant.len() + 4);
    for (i, ch) in variant.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_uppercase());
    }
    result
}

/// Extract the file path from an `FsError`, if present.
fn extract_path(err: &FsError) -> Option<String> {
    match err {
        FsError::NotFound { path }
        | FsError::PermissionDenied { path }
        | FsError::Io { path, .. } => Some(path.clone()),
    }
}

// ---------------------------------------------------------------------------
// AppError → ErrorDto conversion
// ---------------------------------------------------------------------------

impl From<AppError> for ErrorDto {
    fn from(err: AppError) -> Self {
        match err {
            AppError::Auth(ref e) => ErrorDto {
                code: format!("AUTH_{}", error_variant_name(e)),
                message: err.to_string(),
                category: ErrorCategory::Authentication,
                details: None,
            },
            AppError::Network(ref e) => ErrorDto {
                code: format!("NET_{}", error_variant_name(e)),
                message: err.to_string(),
                category: ErrorCategory::Network,
                details: None,
            },
            AppError::FileSystem(ref e) => ErrorDto {
                code: format!("FS_{}", error_variant_name(e)),
                message: err.to_string(),
                category: ErrorCategory::FileSystem,
                details: extract_path(e),
            },
            AppError::Process(ref e) => ErrorDto {
                code: format!("PROC_{}", error_variant_name(e)),
                message: err.to_string(),
                category: ErrorCategory::Process,
                details: None,
            },
            AppError::Config(ref e) => ErrorDto {
                code: format!("CFG_{}", error_variant_name(e)),
                message: err.to_string(),
                category: ErrorCategory::Configuration,
                details: None,
            },
            AppError::Update(ref e) => ErrorDto {
                code: format!("UPD_{}", error_variant_name(e)),
                message: err.to_string(),
                category: ErrorCategory::Update,
                details: None,
            },
        }
    }
}
