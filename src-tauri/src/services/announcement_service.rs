//! Announcement "seen" state.
//!
//! Tracks which announcement the user has read-and-dismissed. Stored in its own
//! file under the app data dir — deliberately NOT in the user-facing `config.ini`
//! so a user can't pre-dismiss the mandatory announcement by editing config. Keyed
//! by an announcement id so publishing a new announcement re-triggers the forced
//! read.

use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = "announcement.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct AnnouncementState {
    #[serde(default)]
    seen_id: Option<String>,
}

fn read_state(dir: &Path) -> AnnouncementState {
    match std::fs::read_to_string(dir.join(FILE)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => AnnouncementState::default(),
    }
}

/// Whether the given announcement id has already been read-and-dismissed.
pub fn is_seen(dir: &Path, id: &str) -> bool {
    read_state(dir).seen_id.as_deref() == Some(id)
}

/// Persist that the given announcement id has been read-and-dismissed.
pub fn mark_seen(dir: &Path, id: &str) -> Result<(), String> {
    let _ = std::fs::create_dir_all(dir);
    let state = AnnouncementState {
        seen_id: Some(id.to_string()),
    };
    let json = serde_json::to_string(&state).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(FILE), json).map_err(|e| e.to_string())
}
