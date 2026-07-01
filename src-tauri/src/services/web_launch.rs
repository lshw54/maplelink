//! Web-login game-launch interception (Windows only).
//!
//! Lets users who can only log in through beanfun's website still launch the
//! game — with auto-paste — through MapleLink. See [`crate::core::game_intercept`]
//! for the command-line format. This module owns the registry (opt-in) toggle
//! and the headless execution path that runs when beanfun invokes us as the
//! "game".

use crate::core::game_intercept::InterceptCreds;

/// Registry location beanfun reads to find the game launcher.
#[cfg(target_os = "windows")]
const GAMANIA_SUBKEY: &str = r"SOFTWARE\Gamania\MapleStory";
#[cfg(target_os = "windows")]
const PATH_VALUE: &str = "PATH";
/// Where we stash beanfun's original PATH so unregister can restore it.
#[cfg(target_os = "windows")]
const BACKUP_VALUE: &str = "PATH_MapleLinkBackup";

/// Filename written next to the game (and read by external scripts) holding the
/// intercepted account + OTP.
const CREDS_FILENAME: &str = "maplelink_launch.ini";

// ---------------------------------------------------------------------------
// Registry toggle (opt-in)
// ---------------------------------------------------------------------------

/// Point `HKCU\SOFTWARE\Gamania\MapleStory\PATH` at MapleLink so beanfun web
/// launches route through us. Beanfun's original value is backed up once.
#[cfg(target_os = "windows")]
pub fn register() -> std::io::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let exe = std::env::current_exe()?.to_string_lossy().into_owned();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(GAMANIA_SUBKEY)?;

    // Back up beanfun's original PATH once, so we can restore it later.
    if let Ok(current) = key.get_value::<String, _>(PATH_VALUE) {
        if !current.eq_ignore_ascii_case(&exe) && key.get_value::<String, _>(BACKUP_VALUE).is_err()
        {
            key.set_value(BACKUP_VALUE, &current)?;
        }
    }

    key.set_value(PATH_VALUE, &exe)?;
    tracing::info!("web-launch interception registered: {exe}");
    Ok(())
}

/// Restore beanfun's original PATH (or remove ours) and drop the backup.
#[cfg(target_os = "windows")]
pub fn unregister() -> std::io::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey_with_flags(GAMANIA_SUBKEY, KEY_ALL_ACCESS) else {
        return Ok(());
    };

    if let Ok(backup) = key.get_value::<String, _>(BACKUP_VALUE) {
        key.set_value(PATH_VALUE, &backup)?;
        let _ = key.delete_value(BACKUP_VALUE);
    } else {
        let _ = key.delete_value(PATH_VALUE);
    }
    tracing::info!("web-launch interception unregistered");
    Ok(())
}

/// Whether `PATH` currently points at this MapleLink executable.
#[cfg(target_os = "windows")]
pub fn is_registered() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe = exe.to_string_lossy();

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(GAMANIA_SUBKEY)
        .and_then(|k| k.get_value::<String, _>(PATH_VALUE))
        .map(|p| p.eq_ignore_ascii_case(&exe))
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub fn register() -> std::io::Result<()> {
    Ok(())
}
#[cfg(not(target_os = "windows"))]
pub fn unregister() -> std::io::Result<()> {
    Ok(())
}
#[cfg(not(target_os = "windows"))]
pub fn is_registered() -> bool {
    false
}

// ---------------------------------------------------------------------------
// Interception execution (headless — runs before the Tauri UI starts)
// ---------------------------------------------------------------------------

/// Handle a beanfun web game-launch: expose the credentials for external
/// scripts, launch the real game, and auto-paste into its login window.
///
/// Runs synchronously and returns; the caller then exits without starting the
/// normal UI.
pub fn run_intercept(creds: InterceptCreds) {
    tracing::info!(
        account = %creds.account,
        otp_len = creds.otp.len(),
        "web-launch interception: handling beanfun game start"
    );

    let game_path = load_game_path();

    // 1. Expose the account + OTP for the user's own script (best effort).
    write_creds_file(&creds, game_path.as_deref());

    // 2. Launch the real game with the exact args beanfun gave us.
    let launched = match &game_path {
        Some(path) => launch_game(path, &creds.raw_args),
        None => {
            tracing::warn!("web-launch: game_path not configured; credentials written only");
            false
        }
    };

    // 3. Auto-paste account + OTP into the game's login window (best effort).
    if launched {
        auto_paste_when_ready(&creds.account, &creds.otp);
    }
}

/// Read `game_path` from the on-disk config without spinning up Tauri.
fn load_game_path() -> Option<String> {
    let appdata = std::env::var("APPDATA").ok()?;
    let config_path = std::path::Path::new(&appdata)
        .join("com.maplelink.app")
        .join("config.ini");
    let text = std::fs::read_to_string(&config_path).ok()?;
    let config = crate::core::config_parser::parse_ini(&text).ok()?;
    let path = config.game_path.trim().to_string();
    if path.is_empty() || !std::path::Path::new(&path).exists() {
        return None;
    }
    Some(path)
}

/// Write `account`/`otp` to a file next to the game (or temp) so an external
/// script can read them. Overwritten each launch; the OTP is single-use.
fn write_creds_file(creds: &InterceptCreds, game_path: Option<&str>) {
    let dir = game_path
        .and_then(|p| std::path::Path::new(p).parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(std::env::temp_dir);
    let file = dir.join(CREDS_FILENAME);
    let body = format!("account={}\notp={}\n", creds.account, creds.otp);
    match std::fs::write(&file, body) {
        Ok(()) => tracing::info!("web-launch: wrote credentials to {}", file.display()),
        Err(e) => tracing::warn!("web-launch: failed to write credentials file: {e}"),
    }
}

/// Spawn the real game with the beanfun-supplied args. Returns whether spawn
/// succeeded.
fn launch_game(game_path: &str, raw_args: &[String]) -> bool {
    let mut cmd = std::process::Command::new(game_path);
    cmd.args(raw_args);
    if let Some(dir) = std::path::Path::new(game_path).parent() {
        cmd.current_dir(dir);
    }
    match cmd.spawn() {
        Ok(child) => {
            tracing::info!("web-launch: launched game pid={}", child.id());
            true
        }
        Err(e) => {
            tracing::error!("web-launch: failed to launch game '{game_path}': {e}");
            false
        }
    }
}

/// Poll for the MapleStory login window and auto-paste the credentials.
/// Best effort: gives up after a fixed window so we never hang.
fn auto_paste_when_ready(account: &str, otp: &str) {
    // Poll for up to ~30s (the game + anti-cheat can take a while to show the
    // login window). Stop as soon as a paste lands.
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if crate::services::autopaste_service::auto_paste_credentials(account, otp, false) {
            tracing::info!("web-launch: auto-pasted credentials into game window");
            return;
        }
    }
    tracing::warn!("web-launch: game login window not found within timeout; auto-paste skipped");
}
