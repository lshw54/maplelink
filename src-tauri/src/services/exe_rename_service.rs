//! Rename-to-`Beanfun.exe` helper for mainland-China users.
//!
//! Chinese game accelerators (网游加速器) route traffic by matching the process
//! name — their MapleStory rule targets `Beanfun.exe`. A user running the
//! portable exe under any other name (e.g. `MapleLink.exe`) won't be
//! accelerated, so login / reCAPTCHA can fail behind the GFW. On startup we
//! geo-check the IP and, when it looks like China, offer to rename the exe to
//! `Beanfun.exe` and relaunch so the accelerator picks it up.
//!
//! Windows allows renaming a running executable's file (unlike deleting it), so
//! we rename our own exe, spawn the renamed copy, and exit.

use crate::services::network_service;

/// The canonical name accelerators match on.
pub const BEANFUN_EXE_NAME: &str = "Beanfun.exe";

/// Result of the startup rename check, surfaced to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeanfunRenameCheck {
    /// Offer the rename prompt (China IP, not already Beanfun.exe, not dismissed,
    /// and no name collision).
    pub suggest: bool,
    /// A different `Beanfun.exe` already exists in the folder — we won't touch it;
    /// the frontend should warn the user to resolve it manually.
    pub collision: bool,
    /// The current exe file name (e.g. `MapleLink.exe`).
    pub current_name: String,
    /// The target name we would rename to.
    pub target_name: String,
}

/// Whether the running exe is already named `beanfun.exe` (case-insensitive).
pub fn is_already_beanfun() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
        })
        .is_some_and(|n| n == "beanfun.exe")
}

fn current_exe_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// The sibling `Beanfun.exe` path next to the current exe, and whether a file is
/// already there.
fn beanfun_target() -> Option<(std::path::PathBuf, bool)> {
    let cur = std::env::current_exe().ok()?;
    let target = cur.with_file_name(BEANFUN_EXE_NAME);
    let exists = target.exists();
    Some((target, exists))
}

/// Pure decision: given the geo country, the user's dismiss flag, whether we're
/// already Beanfun.exe, and whether a target file already exists, decide what to
/// surface. Only mainland-China (`CN`) triggers the offer.
fn decide(
    country: &str,
    dismissed: bool,
    already_beanfun: bool,
    target_exists: bool,
    current_name: String,
) -> BeanfunRenameCheck {
    let target_name = BEANFUN_EXE_NAME.to_string();

    // Already Beanfun.exe, opted out, or not a mainland-China IP → nothing to do.
    if already_beanfun || dismissed || country != "CN" {
        return BeanfunRenameCheck {
            suggest: false,
            collision: false,
            current_name,
            target_name,
        };
    }

    // A different Beanfun.exe already occupies the target name → warn instead of
    // auto-renaming.
    BeanfunRenameCheck {
        suggest: !target_exists,
        collision: target_exists,
        current_name,
        target_name,
    }
}

/// Decide whether to offer the rename. `dismissed` is the user's persisted
/// "don't ask again" choice. Best-effort: any lookup failure yields no suggestion.
pub async fn check(client: &reqwest::Client, dismissed: bool) -> BeanfunRenameCheck {
    let current_name = current_exe_name();
    let already = is_already_beanfun();

    // Short-circuit before the network call when the answer is already known.
    if already || dismissed {
        return decide("", dismissed, already, false, current_name);
    }

    let (_ip, country) = network_service::geo_lookup(client).await;
    let target_exists = beanfun_target().map(|(_, exists)| exists).unwrap_or(false);
    decide(&country, dismissed, already, target_exists, current_name)
}

/// Rename the running exe to `Beanfun.exe`, spawn the renamed copy, and signal the
/// caller to exit. Refuses (returns `Err`) if a different `Beanfun.exe` already
/// exists — the caller should surface a "resolve it manually" warning.
pub fn rename_to_beanfun_and_relaunch() -> Result<(), String> {
    if is_already_beanfun() {
        return Ok(());
    }

    let (target, exists) = beanfun_target().ok_or("cannot resolve current exe path")?;
    if exists {
        return Err(format!(
            "{BEANFUN_EXE_NAME} already exists in the folder; not overwriting"
        ));
    }

    let current = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;

    // Windows permits renaming a running executable's file.
    std::fs::rename(&current, &target).map_err(|e| format!("rename failed: {e}"))?;

    // Launch the renamed copy; the new process is named Beanfun.exe.
    std::process::Command::new(&target).spawn().map_err(|e| {
        // Best-effort rollback so the user isn't left without a runnable exe.
        let _ = std::fs::rename(&target, &current);
        format!("failed to relaunch {BEANFUN_EXE_NAME}: {e}")
    })?;

    tracing::info!("renamed exe to {BEANFUN_EXE_NAME} and relaunched");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name() -> String {
        "MapleLink.exe".to_string()
    }

    // Mock a Taiwan IP: not mainland China → never suggest the rename.
    #[test]
    fn tw_ip_does_not_suggest() {
        let r = decide("TW", false, false, false, name());
        assert!(!r.suggest);
        assert!(!r.collision);
    }

    // Mainland-China IP, clean folder → suggest the rename.
    #[test]
    fn cn_ip_suggests_when_clean() {
        let r = decide("CN", false, false, false, name());
        assert!(r.suggest);
        assert!(!r.collision);
        assert_eq!(r.target_name, BEANFUN_EXE_NAME);
    }

    // China IP but a different Beanfun.exe already exists → warn, don't suggest.
    #[test]
    fn cn_ip_with_collision_warns_not_suggests() {
        let r = decide("CN", false, false, true, name());
        assert!(!r.suggest);
        assert!(r.collision);
    }

    // "Don't ask again" wins even for a China IP.
    #[test]
    fn dismissed_never_suggests() {
        let r = decide("CN", true, false, false, name());
        assert!(!r.suggest);
        assert!(!r.collision);
    }

    // Already Beanfun.exe → nothing to do.
    #[test]
    fn already_beanfun_never_suggests() {
        let r = decide("CN", false, true, false, "Beanfun.exe".to_string());
        assert!(!r.suggest);
        assert!(!r.collision);
    }

    // Other regions (e.g. HK, US) also don't trigger — only CN does.
    #[test]
    fn non_cn_regions_do_not_suggest() {
        for c in ["HK", "US", "JP", ""] {
            assert!(!decide(c, false, false, false, name()).suggest, "country {c}");
        }
    }
}
