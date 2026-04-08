//! Auto-update related models.

use serde::{Deserialize, Serialize};

/// Information about an available update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub changelog: String,
    pub download_url: String,
}

/// Current status of an update operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UpdateStatus {
    /// No update activity.
    Idle,
    /// Checking for updates.
    Checking,
    /// An update is available.
    Available,
    /// Downloading the update.
    Downloading,
    /// Ready to install.
    ReadyToInstall,
    /// Update failed.
    Failed,
}
