//! MapleStory Classic (懷舊服 / "mstc") login + launch.
//!
//! Classic runs on the Gamania "galaxy" login gateway, not the regular game's
//! LR/OTP path. The user authenticates through the normal beanfun login (which
//! leaves a `bfWebToken` in the session cookie jar), then a cookie-seeded webview
//! drives the galaxy SSO through to `maplestoryclassic.beanfun.com/Main`.
//!
//! The Main page auto-fires its own `ngm://` launch, which WebView2 would show a
//! "open Nexon Game Manager" prompt for. We intercept that at the WebView2 layer
//! (`LaunchingExternalUriScheme`), cancel the prompt, and start Nexon Game
//! Manager ourselves from its registered handler — so the whole thing runs in a
//! hidden window with no manual click. If interception isn't available (old
//! runtime) or NGM isn't installed, the portal is revealed for a manual launch.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::models::app_state::AppState;
use crate::models::error::{ErrorCategory, ErrorDto};
use crate::models::session::Region;
use crate::services::cookie_native;
use crate::services::webview_util::WEBVIEW_USER_AGENT;

/// Galaxy classic (mstc) login entry. Issues a fresh OTT, stores it in the page's
/// localStorage and redirects to the init page (whose HK button we auto-click);
/// SSO via the seeded `bfWebToken` then flows through to the portal, which fires
/// its own `ngm://` launch on arrival.
const CLASSIC_ENTRY_URL: &str = "https://galaxy.games.gamania.com/webapi/view/login/mstc?redirect_url=https://maplestoryclassic.beanfun.com/Main?af_click_id=";

/// Injected on every navigation. On the OTT init page it clicks the right login
/// button (HK beanfun vs GamePass) to drive the SSO; on GamaPass's game-account
/// chooser it either auto-continues (single account) or hands the list to the app
/// for a native picker; on the portal Main page it watches for the NGM install
/// guide (which appears instead of the launch when NGM is missing) and flags it
/// via the title. A no-op elsewhere.
fn auto_login_script(region: Region) -> String {
    // HK accounts use the gamania (HK) button; TW / GamePass accounts use Gama Pass.
    let selector = match region {
        Region::HK => ".btnLogin-beanfun",
        Region::TW => ".btnLogin-gamapass",
    };
    format!(
        r#"
(function () {{
  var clicked = false, flagged = false, reported = false, chosen = false;
  var needsLogin = false;

  function radios() {{
    return [].slice.call(document.querySelectorAll('input[type=radio][name=account]'));
  }}

  // The chooser is a JS-driven page: tick the radio, then press 繼續. Both are
  // real clicks so the page's own handlers run.
  function choose(value) {{
    var rs = radios();
    for (var i = 0; i < rs.length; i++) {{
      if (rs[i].value !== value) continue;
      chosen = true;
      rs[i].click();
      setTimeout(function () {{
        var btns = [].slice.call(
          document.querySelectorAll('.bottom-fixed-action-area a.ui-btn, a.ui-btn')
        );
        var go = btns[btns.length - 1];
        if (go) go.click();
      }}, 150);
      return true;
    }}
    return false;
  }}
  // Called from the app once the user picks in the native dialog.
  window.__mlPickClassicAccount = choose;

  // Report state to the app over IPC. Setting document.title looks tempting but
  // does not reach the native window title, so the app never sees it.
  function report(cmd) {{
    if (window.__TAURI_INTERNALS__) window.__TAURI_INTERNALS__.invoke(cmd);
  }}

  function tick() {{
    var href = location.href;
    if (href.indexOf('/login/init/mstc/') !== -1) {{
      if (!clicked) {{
        var btn = document.querySelector('{selector}');
        if (btn) {{ btn.click(); clicked = true; }}
      }}
    }} else if (href.indexOf('/GamaPassLogin/SelectGameAccount') !== -1) {{
      if (chosen) return;
      var rs = radios();
      if (!rs.length) return;
      // One account is not a choice — go straight through, no dialog.
      if (rs.length === 1) {{ choose(rs[0].value); return; }}
      if (reported) return;
      reported = true;
      var list = rs.map(function (r) {{
        var lbl = r.closest('label');
        var name = ((lbl && lbl.innerText) || '').trim();
        return {{ value: r.value, label: name || r.value }};
      }});
      if (window.__TAURI_INTERNALS__) {{
        window.__TAURI_INTERNALS__.invoke('classic_accounts_found', {{ accounts: list }});
      }}
    }} else if (document.querySelector('input[type=password]')) {{
      // A real password field is on screen, so the session didn't carry.
      // Flag on the field itself, never on the URL: the sign-in hops
      // redirect straight through when there is a session to reuse.
      if (!needsLogin) {{ needsLogin = true; report('classic_needs_login'); }}
    }} else if (href.indexOf('maplestoryclassic.beanfun.com/Main') !== -1) {{
      if (!flagged &&
          (document.getElementById('ngmBtnStart') ||
           document.getElementById('ngmInstallLayerClose'))) {{
        flagged = true;
        report('classic_ngm_missing');
      }}
    }}
  }}
  setInterval(tick, 300);
}})();
"#
    )
}

/// A selectable GamaPass game account from the chooser page.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassicAccount {
    /// The radio's value — what the page POSTs as `OpidSelAccount`.
    pub value: String,
    /// Display name shown next to the radio.
    pub label: String,
}

/// Set while the user is choosing a game account in the native dialog, so the
/// launch poll doesn't age out waiting for a human. Only one classic window can
/// exist at a time (the label is fixed and any previous one is destroyed first),
/// so a single global is enough.
static AWAITING_ACCOUNT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set by the portal when it puts a password form on screen, and when it shows
/// the NGM install guide. Reported over IPC because a page's `document.title`
/// never reaches the native window title, so polling `win.title()` for markers
/// silently saw nothing.
static NEEDS_LOGIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static NGM_MISSING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The portal is asking for a sign-in of its own.
pub fn mark_needs_login() {
    NEEDS_LOGIN.store(true, Ordering::SeqCst);
}

/// The portal showed the "install Nexon Game Manager" guide.
pub fn mark_ngm_missing() {
    NGM_MISSING.store(true, Ordering::SeqCst);
}

/// Hand the GamaPass account list to the frontend and pause the launch timeout.
/// Invoked from the injected script when the chooser lists more than one account.
pub fn accounts_found(accounts: Vec<ClassicAccount>, app: &tauri::AppHandle) {
    tracing::info!(
        "classic: GamaPass chooser listed {} accounts",
        accounts.len()
    );
    AWAITING_ACCOUNT.store(true, Ordering::SeqCst);
    let _ = app.emit("classic-select-account", accounts);
}

/// Apply the user's pick in the classic window and resume the launch timeout.
pub fn pick_account(value: &str, app: &tauri::AppHandle) -> Result<(), ErrorDto> {
    let win = app
        .get_webview_window("classic-login")
        .ok_or_else(|| ErrorDto {
            code: "CLASSIC_WINDOW_GONE".to_string(),
            message: "The classic portal window is no longer open".to_string(),
            category: ErrorCategory::Process,
            details: None,
        })?;
    // serde_json renders a correctly escaped JS string literal for any value.
    let literal = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string());
    win.eval(format!("window.__mlPickClassicAccount({literal});"))
        .map_err(|e| ErrorDto {
            code: "CLASSIC_PICK_FAILED".to_string(),
            message: format!("Failed to select the game account: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })?;
    AWAITING_ACCOUNT.store(false, Ordering::SeqCst);
    Ok(())
}

// Launch state shared between the intercept callback and the poll task.
const PENDING: u8 = 0;
const LAUNCHED: u8 = 1;
const FAILED: u8 = 2;

/// Start Nexon Game Manager for a captured `ngm://` URL by invoking its
/// registered handler directly (`HKCR\ngm\shell\open\command`).
///
/// Deliberately no shell fallback: we're called from inside the intercept that
/// just cancelled WebView2's prompt, and handing the URL to the shell would only
/// pop the prompt straight back. If NGM isn't registered this fails, and the
/// caller reveals the portal so the user can install / launch it by hand.
#[cfg(target_os = "windows")]
fn launch_ngm(url: &str, manual_path: Option<&str>) -> Result<(), String> {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    // A user-provided NGM path (set when auto-detection failed) wins.
    if let Some(path) = manual_path {
        if !path.is_empty() && std::path::Path::new(path).exists() {
            std::process::Command::new(path)
                .arg(url)
                .spawn()
                .map_err(|e| format!("failed to launch NGM ({path}): {e}"))?;
            tracing::info!("classic: launched NGM (manual path {path})");
            return Ok(());
        }
    }

    let command: String = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"ngm\shell\open\command")
        .and_then(|k| k.get_value(""))
        .map_err(|e| {
            format!("ngm handler not registered (is Nexon Game Manager installed?): {e}")
        })?;

    let (exe, args) = parse_handler_command(&command, url)
        .ok_or_else(|| format!("could not parse ngm handler command: {command}"))?;

    std::process::Command::new(&exe)
        .args(&args)
        .spawn()
        .map_err(|e| format!("failed to launch NGM ({exe}): {e}"))?;
    tracing::info!("classic: launched NGM directly ({exe})");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_ngm(_url: &str, _manual_path: Option<&str>) -> Result<(), String> {
    Err("ngm launch is only supported on Windows".to_string())
}

/// Parse a registered protocol handler command (`"exe" "%1"` / `exe %1`) into the
/// executable and its arguments, substituting the URL for every `%1`.
#[cfg(target_os = "windows")]
fn parse_handler_command(command: &str, url: &str) -> Option<(String, Vec<String>)> {
    let command = command.trim();
    let (exe, rest) = if let Some(after) = command.strip_prefix('"') {
        let end = after.find('"')?;
        (after[..end].to_string(), &after[end + 1..])
    } else {
        let end = command.find(' ').unwrap_or(command.len());
        (command[..end].to_string(), &command[end..])
    };
    if exe.is_empty() {
        return None;
    }
    let args = rest
        .split_whitespace()
        .map(|a| a.trim_matches('"').replace("%1", url))
        .collect::<Vec<_>>();
    // If the handler declares no %1 slot, pass the URL as a trailing argument.
    let args = if args.iter().any(|a| a.contains(url)) {
        args
    } else {
        vec![url.to_string()]
    };
    Some((exe, args))
}

/// Result of the classic-readiness self-check, shown so users can tell whether
/// the pieces the launch relies on are present.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassicCheck {
    /// Nexon Game Manager's `ngm://` protocol handler is registered.
    pub ngm_registered: bool,
    /// The handler's executable path, if we could read it.
    pub ngm_exe: Option<String>,
    /// That executable actually exists on disk.
    pub ngm_exe_exists: bool,
    /// Installed WebView2 runtime version (drives whether the launch prompt can
    /// be auto-suppressed), if detectable.
    pub webview2_version: Option<String>,
    /// The Classic client's executable, wherever it turned out to be installed.
    pub game_exe: Option<String>,
}

/// The Classic client's executable, inside whichever folder it was installed to.
const CLASSIC_EXE: &str = "Maplestory_Classic.exe";

/// Locate the installed Classic client.
///
/// Nexon Game Manager records the install folder as `RootPath` under a per-title
/// subkey of `Nexon` — the subkey name is an encoded id (base64 of something like
/// `2982_2141_live_837`), so the keys are enumerated and matched on actually
/// holding the client, rather than assuming an id or an install drive. Falls back
/// to the usual install folders across the machine's drives, since a user who
/// moved the game may also have lost the registry entry.
#[cfg(target_os = "windows")]
pub fn detect_game_exe() -> Option<String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    let exe_in = |dir: &str| {
        let exe = std::path::Path::new(dir).join(CLASSIC_EXE);
        exe.is_file().then(|| exe.to_string_lossy().to_string())
    };

    // NGM is 32-bit, so its keys land under WOW6432Node; read the 64-bit view too
    // in case a future build registers there instead.
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for flags in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
        let Ok(nexon) = hklm.open_subkey_with_flags(r"SOFTWARE\Nexon", KEY_READ | flags) else {
            continue;
        };
        for title in nexon.enum_keys().flatten() {
            let root = nexon
                .open_subkey_with_flags(&title, KEY_READ | flags)
                .and_then(|k| k.get_value::<String, _>("RootPath"));
            if let Ok(root) = root {
                if let Some(exe) = exe_in(&root) {
                    tracing::info!("classic: client found via Nexon\\{title}: {exe}");
                    return Some(exe);
                }
            }
        }
    }

    for drive in ('C'..='Z').map(|d| format!("{d}:")) {
        for base in [
            r"\Program Files\Gamania",
            r"\Program Files (x86)\Gamania",
            r"\Gamania",
        ] {
            let dir = format!("{drive}{base}\\maplestory_classic");
            if let Some(exe) = exe_in(&dir) {
                tracing::info!("classic: client found at a well-known path: {exe}");
                return Some(exe);
            }
        }
    }

    tracing::debug!("classic: no installed client found");
    None
}

#[cfg(not(target_os = "windows"))]
pub fn detect_game_exe() -> Option<String> {
    None
}

/// Check that the local prerequisites for the classic launch are in place.
/// A non-empty, existing `manual_path` counts as NGM being available.
#[cfg(target_os = "windows")]
pub fn self_check(manual_path: &str) -> ClassicCheck {
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let mut check = ClassicCheck::default();

    if let Ok(command) = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"ngm\shell\open\command")
        .and_then(|k| k.get_value::<String, _>(""))
    {
        check.ngm_registered = true;
        if let Some((exe, _)) = parse_handler_command(&command, "") {
            check.ngm_exe_exists = std::path::Path::new(&exe).exists();
            check.ngm_exe = Some(exe);
        }
    }

    // Fall back to a user-provided path when auto-detection came up empty.
    let auto_ok = check.ngm_registered && check.ngm_exe_exists;
    if !auto_ok && !manual_path.is_empty() && std::path::Path::new(manual_path).exists() {
        check.ngm_registered = true;
        check.ngm_exe = Some(manual_path.to_string());
        check.ngm_exe_exists = true;
    }

    // WebView2 Evergreen Runtime version, machine-wide then per-user.
    const WV2: &str =
        r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const WV2_USER: &str =
        r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    check.webview2_version = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(WV2)
        .and_then(|k| k.get_value::<String, _>("pv"))
        .or_else(|_| {
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(WV2_USER)
                .and_then(|k| k.get_value::<String, _>("pv"))
        })
        .ok();

    check.game_exe = detect_game_exe();

    check
}

#[cfg(not(target_os = "windows"))]
pub fn self_check(_manual_path: &str) -> ClassicCheck {
    ClassicCheck::default()
}

/// Open the classic portal for an already-authenticated session and auto-launch
/// the game once it lands, reusing the session's cookies so no re-login is needed.
pub async fn open_classic_login(
    session_id: String,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), ErrorDto> {
    use tauri::WebviewWindowBuilder;

    // TW keeps Classic and the regular server on separate logins — signing in to
    // one gets you nowhere on the other — so a TW classic sign-in starts from
    // scratch on the galaxy side. Called with no session id we seed nothing and
    // let the portal run its own GamaPass flow (the password form is detected and
    // the window revealed). HK is interoperable, so it keeps passing its session
    // and riding the SSO through without interaction.
    let ss = if session_id.is_empty() {
        None
    } else {
        Some(state.require_session(&session_id).await?)
    };
    // No session means no cookies to ride in on, so a sign-in is certain rather
    // than something to detect: show the portal straight away instead of hiding
    // a login form behind the launch spinner.
    let needs_manual = ss.is_none();
    let label = "classic-login";

    // The portal offers both HK-beanfun and GamaPass sign-in; auto-click the one
    // matching this session's region. Sessionless means the TW GamaPass path.
    let region = match ss.as_ref() {
        Some(ss) => ss
            .session
            .read()
            .await
            .as_ref()
            .map(|s| s.region.clone())
            .unwrap_or(Region::HK),
        None => Region::TW,
    };
    let init_script = auto_login_script(region);

    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    // A GamaPass classic login runs straight off the back of the GamaPass login
    // window closing. Creating a webview while another one is still tearing down
    // fails with ERROR_INVALID_STATE (0x8007139F) — and Tauri still hands back a
    // window, just one with no webview inside, so every handler below then times
    // out and nothing ever loads. Wait for those windows to actually be gone,
    // then let the runtime settle.
    let mut waited = false;
    for _ in 0..30 {
        let busy = ["gamepass-login", "web-login", "recaptcha_window"]
            .iter()
            .any(|l| app.get_webview_window(l).is_some());
        if !busy {
            break;
        }
        waited = true;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    if waited {
        tracing::info!("classic: waited for a closing login webview before opening the portal");
    }
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // Seed the session's beanfun cookies so the HK SSO step skips re-login.
    let seed_cookies = match ss.as_ref() {
        None => Vec::new(),
        Some(ss) => cookie_native::cookies_from_jar(
            &ss.cookie_jar,
            &[
                "https://bfweb.hk.beanfun.com/",
                "https://login.hk.beanfun.com/",
                "https://beanfun.com/",
                "https://login.beanfun.com/",
                "https://tw.beanfun.com/",
                "https://tw.newlogin.beanfun.com/",
                // Classic's GamaPass button goes to openid.beanfun.com/login/index
                // (clientid 17599671-…, redirecting to galaxy .../mstc/beanfun).
                // Without this origin's cookies that hop starts logged out and the
                // portal falls back to asking for the password again.
                "https://openid.beanfun.com/",
            ],
        ),
    };

    let data_dir = app.path().app_data_dir().map_err(|e| ErrorDto {
        code: "SYS_PATH_ERROR".to_string(),
        message: format!("Failed to get app data dir: {e}"),
        category: ErrorCategory::Process,
        details: None,
    })?;

    let build_portal = |data_dir: std::path::PathBuf| {
        WebviewWindowBuilder::new(
            &app,
            label,
            tauri::WebviewUrl::External("about:blank".parse().unwrap()),
        )
        .title("新楓之谷：經典版")
        .inner_size(1024.0, 720.0)
        .min_inner_size(400.0, 300.0)
        .decorations(true)
        .resizable(true)
        .center()
        .visible(false)
        .data_directory(data_dir)
        .user_agent(WEBVIEW_USER_AGENT)
        .initialization_script(&init_script)
        .build()
        .map_err(|e| ErrorDto {
            code: "SYS_POPUP_FAILED".to_string(),
            message: format!("Failed to open classic portal: {e}"),
            category: ErrorCategory::Process,
            details: None,
        })
    };

    let mut win = build_portal(data_dir.clone())?;

    // This talks to the webview over COM, so it failing is how a
    // window-without-a-webview shows up (webview creation is logged by the
    // runtime but still yields a window). Rebuild once rather than run the whole
    // flow against a dead window and time out much later. It doubles as the
    // liveness probe for the sessionless path, where there are no cookies to
    // seed and seeding would return Ok without touching the webview at all.
    if let Err(e) = cookie_native::register_new_window_handler(&win) {
        tracing::warn!("classic portal: webview looks dead ({e}) — rebuilding the window");
        let _ = win.destroy();
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        win = build_portal(data_dir)?;
        if let Err(e) = cookie_native::register_new_window_handler(&win) {
            tracing::warn!("classic portal: NewWindowRequested handler failed again: {e}");
        }
    }
    if let Err(e) = cookie_native::seed_cookies_native(&win, &seed_cookies) {
        tracing::warn!("classic portal: native cookie seeding failed: {e}");
    }

    // Intercept the portal's own ngm:// launch: cancel WebView2's prompt and start
    // NGM ourselves. The flag lets the poll task react (close on success, reveal
    // for manual launch on failure).
    let manual_ngm = state.config.read().await.classic_ngm_path.clone();
    let flag = Arc::new(AtomicU8::new(PENDING));
    let flag_cb = flag.clone();
    let intercept_ok = cookie_native::register_external_uri_handler(&win, move |url| {
        if !(url.starts_with("ngm:") || url.starts_with("nexonplug:")) {
            return;
        }
        let manual = (!manual_ngm.is_empty()).then_some(manual_ngm.as_str());
        let outcome = match launch_ngm(url, manual) {
            Ok(()) => LAUNCHED,
            Err(e) => {
                tracing::warn!("classic: ngm launch failed: {e}");
                FAILED
            }
        };
        flag_cb.store(outcome, Ordering::SeqCst);
    })
    .inspect_err(|e| tracing::warn!("classic: external-uri interception unavailable: {e}"))
    .is_ok();

    let _ = win.eval(format!("window.location.href = '{CLASSIC_ENTRY_URL}';"));

    if needs_manual {
        tracing::info!("classic: no session — showing the portal for its own sign-in");
        let _ = app.emit("classic-needs-login", ());
        let _ = win.show();
        let _ = win.set_focus();
    }

    // Without interception the prompt can't be suppressed — reveal the window so
    // the user can complete the launch by hand.
    if !intercept_ok {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    // Hidden auto-launch: wait for the intercept to fire, then close (success) or
    // reveal for manual completion (failure / timeout).
    AWAITING_ACCOUNT.store(false, Ordering::SeqCst);
    NEEDS_LOGIN.store(false, Ordering::SeqCst);
    NGM_MISSING.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn(async move {
        tracing::info!("classic portal running (hidden), waiting for launch");
        // Ticks are 500ms. The galaxy SSO → GamaPass → portal chain routinely
        // takes well past half a minute, so stay hidden for a good while before
        // giving up on it; revealing early only puts a half-loaded page in the
        // user's face. After revealing we keep watching, because the portal often
        // does fire its launch late — that has to end in success, not a stale
        // "failed". A separate allowance covers time spent in the account picker.
        const HIDDEN_TICKS: u32 = 180; // 90s before showing the portal
        const LATE_TICKS: u32 = 240; // then 120s more, still watching
        const AWAIT_TICKS: u32 = 600; // 5 min of the user picking an account
        let mut ticks: u32 = 0;
        let mut waiting_ticks: u32 = 0;
        let mut revealed = needs_manual;
        let mut manual_login = needs_manual;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if win.title().is_err() {
                return; // window gone
            }
            // NGM isn't installed — the portal shows the official install guide.
            // Reveal it (so the user gets that guide) and report the failure now
            // instead of waiting out the timeout.
            if NGM_MISSING.load(Ordering::SeqCst) {
                tracing::warn!("classic: NGM install guide shown — not installed");
                let _ = win.app_handle().emit("classic-launch-failed", ());
                let _ = win.show();
                let _ = win.set_focus();
                return;
            }
            // Classic wants a sign-in of its own (on TW it is a separate login
            // from the regular server, so this is normal, not a failure). Show
            // the portal at once and let the user finish it there — the launch
            // carries on from this same loop afterwards.
            if NEEDS_LOGIN.load(Ordering::SeqCst) && !manual_login {
                manual_login = true;
                revealed = true;
                tracing::info!("classic: portal is asking for a sign-in — revealing it");
                let _ = win.app_handle().emit("classic-needs-login", ());
                let _ = win.show();
                let _ = win.set_focus();
            }
            match flag.load(Ordering::SeqCst) {
                LAUNCHED => {
                    // Also the late path: a launch after the reveal replaces the
                    // reported timeout with success and closes the portal.
                    tracing::info!("classic: launch detected after {}s", ticks / 2);
                    let _ = win.app_handle().emit("classic-launched", ());
                    let _ = win.destroy();
                    return;
                }
                FAILED => {
                    let _ = win.app_handle().emit("classic-launch-failed", ());
                    let _ = win.show();
                    let _ = win.set_focus();
                    return;
                }
                _ => {}
            }
            // The account chooser is up: hold the launch countdown, the user is
            // deciding. Only the (much longer) waiting budget ticks down.
            // Both "a human is typing" states draw on the same long allowance
            // rather than the launch countdown.
            if manual_login || AWAITING_ACCOUNT.load(Ordering::SeqCst) {
                waiting_ticks += 1;
                if waiting_ticks >= AWAIT_TICKS {
                    break;
                }
                continue;
            }
            ticks += 1;
            if !revealed && ticks >= HIDDEN_TICKS {
                revealed = true;
                tracing::warn!("classic: no launch yet — revealing portal, still watching");
                let _ = win.app_handle().emit("classic-launch-timeout", ());
                let _ = win.show();
                let _ = win.set_focus();
            }
            if ticks >= HIDDEN_TICKS + LATE_TICKS {
                break;
            }
        }
        AWAITING_ACCOUNT.store(false, Ordering::SeqCst);
        if !revealed {
            tracing::warn!("classic: gave up waiting — revealing portal");
            let _ = win.app_handle().emit("classic-launch-timeout", ());
            let _ = win.show();
            let _ = win.set_focus();
        }
    });

    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_and_bare_handler_commands() {
        let (exe, args) = parse_handler_command(r#""C:\NGM\ngm.exe" "%1""#, "ngm://x").unwrap();
        assert_eq!(exe, r"C:\NGM\ngm.exe");
        assert_eq!(args, vec!["ngm://x".to_string()]);

        // No %1 slot → the URL is appended as a trailing argument.
        let (exe, args) = parse_handler_command(r#""C:\NGM\ngm.exe""#, "ngm://y").unwrap();
        assert_eq!(exe, r"C:\NGM\ngm.exe");
        assert_eq!(args, vec!["ngm://y".to_string()]);
    }
}
