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

/// Filename written next to the game (read by external scripts) holding the
/// intercepted account + OTP — only used when invoked directly (not via .bat).
const CREDS_FILENAME: &str = "maplelink_launch.ini";

/// Helper batch file written INTO the game folder. beanfun's launcher only
/// reliably runs a script sitting in the game folder (a plain exe elsewhere is
/// ignored — this matches the community `.bat`), so we drop this there and point
/// the registry at it. It echoes the account/OTP to a console (script- and
/// human-readable) and hands off to MapleLink for the game launch + auto-paste.
#[cfg(target_os = "windows")]
const HELPER_BAT: &str = "maplelink_web_launch.bat";

/// Absolute path of the helper `.bat` inside the game folder (needs game_path).
#[cfg(target_os = "windows")]
fn helper_bat_path() -> Option<std::path::PathBuf> {
    let game_path = load_game_path()?;
    std::path::Path::new(&game_path)
        .parent()
        .map(|dir| dir.join(HELPER_BAT))
}

// ---------------------------------------------------------------------------
// Registry toggle (opt-in)
// ---------------------------------------------------------------------------

/// Write the helper `.bat` into the game folder and point
/// `HKCU\SOFTWARE\Gamania\MapleStory\PATH` at it, so beanfun web launches route
/// through us. Beanfun's original value is backed up once.
#[cfg(target_os = "windows")]
pub fn register() -> std::io::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let bat = helper_bat_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "game path not set — set the game path first",
        )
    })?;
    let exe = std::env::current_exe()?.to_string_lossy().into_owned();

    // beanfun passes: <server> <port> BeanFun <account> <otp> → %4/%5. We echo
    // those (console, for scripts) and forward everything to MapleLink, tagged
    // with --web-launch so it launches the game + auto-pastes without its own
    // popup.
    let script = format!(
        "@echo off\r\n\
         start \"\" \"{exe}\" --web-launch %*\r\n\
         echo ===Account===\r\n\
         echo %4\r\n\
         echo ===Password===\r\n\
         echo %5\r\n\
         echo.\r\n\
         echo MapleLink is launching the game...\r\n\
         pause>nul\r\n"
    );
    std::fs::write(&bat, script)?;
    let bat_str = bat.to_string_lossy().into_owned();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(GAMANIA_SUBKEY)?;
    if let Ok(current) = key.get_value::<String, _>(PATH_VALUE) {
        if !current.eq_ignore_ascii_case(&bat_str)
            && key.get_value::<String, _>(BACKUP_VALUE).is_err()
        {
            key.set_value(BACKUP_VALUE, &current)?;
        }
    }
    key.set_value(PATH_VALUE, &bat_str)?;
    tracing::info!("web-launch interception registered: {bat_str}");
    Ok(())
}

/// Restore beanfun's original PATH (or remove ours), drop the backup, and
/// delete the helper `.bat`.
#[cfg(target_os = "windows")]
pub fn unregister() -> std::io::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    if let Some(bat) = helper_bat_path() {
        let _ = std::fs::remove_file(bat);
    }

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

/// Whether `PATH` currently points at our helper `.bat`.
#[cfg(target_os = "windows")]
pub fn is_registered() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    let Some(bat) = helper_bat_path() else {
        return false;
    };
    let bat_str = bat.to_string_lossy();

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(GAMANIA_SUBKEY)
        .and_then(|k| k.get_value::<String, _>(PATH_VALUE))
        .map(|p| p.eq_ignore_ascii_case(&bat_str))
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

/// Handle a beanfun web game-launch: launch the real game and auto-paste into
/// its login window.
///
/// `quiet` is set when we were invoked by the helper `.bat` (the normal path):
/// the .bat already shows the account/OTP in a console, so we skip our own
/// popup + credentials file and just do the launch + auto-paste. When invoked
/// directly (no .bat) we also write the file and show the copyable popup.
///
/// Runs synchronously and returns; the caller then exits without starting the
/// normal UI.
pub fn run_intercept(creds: InterceptCreds, quiet: bool) {
    tracing::info!(
        account = %creds.account,
        otp_len = creds.otp.len(),
        quiet,
        raw_args = ?creds.raw_args,
        "web-launch interception: handling beanfun game start"
    );

    let game_path = load_game_path();

    if !quiet {
        // Expose the account + OTP for the user's own script (best effort).
        write_creds_file(&creds, game_path.as_deref());
    }

    // Launch the real game with the exact args beanfun gave us.
    let launched = match &game_path {
        Some(path) => launch_game(path, &creds.raw_args),
        None => {
            tracing::warn!("web-launch: game_path not configured; cannot launch game");
            false
        }
    };

    // Auto-paste in the background so any popup can appear immediately.
    let paste_handle = if launched {
        let account = creds.account.clone();
        let otp = creds.otp.clone();
        Some(std::thread::spawn(move || {
            auto_paste_when_ready(&account, &otp)
        }))
    } else {
        None
    };

    // Only prompt when invoked directly; the .bat's console handles this.
    if !quiet {
        show_creds_popup(&creds.account, &creds.otp);
    }

    // Block until a running auto-paste finishes (its own timeout caps this) so
    // the process stays alive long enough to type into the login window.
    if let Some(handle) = paste_handle {
        let _ = handle.join();
    }
}

/// Show a copyable popup with the account + OTP, so the user doesn't have to
/// open the credentials file. Blocks until dismissed.
#[cfg(target_os = "windows")]
fn show_creds_popup(account: &str, otp: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    };

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let body = format!(
        "帳號 Account:\n{account}\n\nOTP（一次性密碼 / one-time）:\n{otp}\n\n（可按 Ctrl+C 複製整個視窗內容）"
    );
    let title = wide("MapleLink — 網頁登入帳號 / OTP");
    let text = wide(&body);
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_creds_popup(_account: &str, _otp: &str) {}

/// Read `game_path` from the on-disk config without spinning up Tauri.
fn load_game_path() -> Option<String> {
    let appdata = match std::env::var("APPDATA") {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("web-launch: APPDATA not set ({e}); cannot locate config");
            return None;
        }
    };
    let config_path = std::path::Path::new(&appdata)
        .join("com.maplelink.app")
        .join("config.ini");
    let text = match std::fs::read_to_string(&config_path) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(
                "web-launch: cannot read config {}: {e}",
                config_path.display()
            );
            return None;
        }
    };
    let config = match crate::core::config_parser::parse_ini(&text) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("web-launch: failed to parse config: {e}");
            return None;
        }
    };
    let path = config.game_path.trim().to_string();
    if path.is_empty() {
        tracing::warn!("web-launch: game_path is empty in config — set the game path in MapleLink");
        return None;
    }
    if !std::path::Path::new(&path).exists() {
        tracing::warn!("web-launch: configured game_path does not exist on disk: {path}");
        return None;
    }
    tracing::info!("web-launch: resolved game_path = {path}");
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

/// Launch the real game with the beanfun-supplied args. When the system locale
/// isn't Traditional Chinese, route through Locale Remulator (LRProc.exe) — the
/// same as the normal launch path — otherwise MapleStory TW won't start.
fn launch_game(game_path: &str, raw_args: &[String]) -> bool {
    let zh_tw = crate::services::lr_service::is_system_locale_chinese_traditional();
    tracing::info!(
        system_zh_tw = zh_tw,
        exe = game_path,
        args = ?raw_args,
        "web-launch: launching game"
    );
    let use_lr = !zh_tw;
    if use_lr {
        if let Some(lrproc) = lr_proc_path() {
            // LRProc args: <profile-guid> <game exe> <beanfun args…>
            let mut args = vec![
                crate::services::lr_service::LR_PROFILE_GUID.to_string(),
                game_path.to_string(),
            ];
            args.extend(raw_args.iter().cloned());
            return spawn_game(&lrproc, &args, game_path, "LR");
        }
        tracing::warn!(
            "web-launch: non-zh-TW locale but LRProc.exe not found — launching directly (may fail)"
        );
    }
    spawn_game(
        std::path::Path::new(game_path),
        raw_args,
        game_path,
        "direct",
    )
}

/// Locate the extracted `LRProc.exe` without a Tauri handle (it lives under the
/// app data dir once any normal launch has run).
fn lr_proc_path() -> Option<std::path::PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    let p = std::path::Path::new(&appdata)
        .join("com.maplelink.app")
        .join("lr")
        .join("LRProc.exe");
    p.exists().then_some(p)
}

/// Spawn `program` with `args`, cwd = the game folder. Returns whether spawn
/// succeeded.
fn spawn_game(program: &std::path::Path, args: &[String], game_path: &str, how: &str) -> bool {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    if let Some(dir) = std::path::Path::new(game_path).parent() {
        cmd.current_dir(dir);
    }
    match cmd.spawn() {
        Ok(child) => {
            tracing::info!("web-launch: launched game ({how}) pid={}", child.id());
            true
        }
        Err(e) => {
            tracing::error!(
                "web-launch: failed to launch game ({how}) '{}': {e}",
                program.display()
            );
            false
        }
    }
}

/// Poll for the MapleStory login window and auto-paste the credentials.
/// Best effort: gives up after a fixed window so we never hang.
fn auto_paste_when_ready(account: &str, otp: &str) {
    tracing::info!("web-launch: waiting for game login window to auto-paste…");
    // Poll for up to ~30s (the game + anti-cheat can take a while to show the
    // login window). Stop as soon as a paste lands.
    for attempt in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if crate::services::autopaste_service::auto_paste_credentials(account, otp, false) {
            tracing::info!(
                "web-launch: auto-pasted credentials into game window after ~{}ms",
                (attempt + 1) * 500
            );
            return;
        }
        // Heartbeat every ~5s so a stuck wait is visible in the log.
        if attempt > 0 && attempt % 10 == 0 {
            tracing::info!(
                "web-launch: still waiting for game login window (~{}s elapsed)",
                (attempt + 1) / 2
            );
        }
    }
    tracing::warn!("web-launch: game login window not found within 30s; auto-paste skipped");
}
