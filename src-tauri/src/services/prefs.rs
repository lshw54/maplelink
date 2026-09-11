//! Small persisted key/value store for UI preferences.
//!
//! `AppConfig` is the place for settings the player manages, and every field
//! there has to be threaded through parsing, serialising and a handful of
//! exhaustive literals. A one-off answer like "run the check automatically,
//! and stop asking" does not deserve that, and it does not belong in an
//! exported backup either — so it lives here instead.

use std::collections::BTreeMap;
use std::path::Path;

const FILE: &str = "prefs.json";

fn path(dir: &Path) -> std::path::PathBuf {
    dir.join(FILE)
}

fn read(dir: &Path) -> BTreeMap<String, String> {
    match std::fs::read_to_string(path(dir)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => BTreeMap::new(),
    }
}

pub fn get(dir: &Path, key: &str) -> Option<String> {
    read(dir).get(key).cloned()
}

pub fn set(dir: &Path, key: &str, value: &str) -> Result<(), String> {
    let mut all = read(dir);
    all.insert(key.to_string(), value.to_string());
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    std::fs::write(path(dir), json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "maplelink_prefs_{tag}_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn a_value_survives_a_round_trip_and_can_be_changed() {
        let dir = temp_dir("roundtrip");
        assert_eq!(get(&dir, "client.auto_check"), None);

        set(&dir, "client.auto_check", "on").unwrap();
        assert_eq!(get(&dir, "client.auto_check").as_deref(), Some("on"));

        // Unlike the seen-flags file, an answer here can be taken back.
        set(&dir, "client.auto_check", "off").unwrap();
        assert_eq!(get(&dir, "client.auto_check").as_deref(), Some("off"));

        // Other keys are left alone.
        set(&dir, "client.asked", "1").unwrap();
        assert_eq!(get(&dir, "client.auto_check").as_deref(), Some("off"));
        assert_eq!(get(&dir, "client.asked").as_deref(), Some("1"));
    }

    #[test]
    fn an_unreadable_file_reads_as_empty_rather_than_failing() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(path(&dir), b"{ not json").unwrap();
        assert_eq!(get(&dir, "anything"), None);
        // And writing repairs it.
        set(&dir, "anything", "1").unwrap();
        assert_eq!(get(&dir, "anything").as_deref(), Some("1"));
    }
}
