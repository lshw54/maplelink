//! Serializable error DTOs sent to the frontend.

use serde::{Deserialize, Serialize};

/// Serializable error returned to the frontend via Tauri IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDto {
    /// Machine-readable error code, e.g. `"AUTH_INVALID_CREDENTIALS"`.
    pub code: String,
    /// English fallback message.
    pub message: String,
    /// Error domain category.
    pub category: ErrorCategory,
    /// Optional extra details (e.g. file path for FS errors).
    pub details: Option<String>,
}

/// Error domain categories for frontend routing/display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorCategory {
    Authentication,
    Network,
    FileSystem,
    Process,
    Configuration,
    Update,
}
