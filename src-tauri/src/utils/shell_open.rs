//! Opening external URLs / folders without handing them our admin token.
//!
//! MapleLink self-elevates at startup (needed for auto-paste and LR DLL
//! injection), so every process it spawns inherits the elevated token. A browser
//! launched that way writes its profile as Administrator, which leaves
//! admin-owned files in the user's real profile — the browser then fails to read
//! them back when it next runs normally, and presents as a wiped profile.
//!
//! Handing the target to `explorer.exe` instead lets the already-running,
//! non-elevated shell open it as the desktop user, so the browser stays
//! unelevated.

/// Open `target` via the desktop shell so it does NOT inherit our elevated token.
///
/// `target` is passed to the shell verbatim, so callers must only pass trusted
/// values (our own paths) — anything user- or server-supplied must go through
/// [`open_external_url`], which enforces an http(s) scheme.
///
/// Note: `explorer.exe` returns immediately and its exit code does not reflect
/// whether the target opened, so only a spawn failure can be reported.
#[cfg(target_os = "windows")]
pub fn shell_open(target: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    match std::process::Command::new("explorer.exe")
        .arg(target)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
    {
        Ok(child) => {
            tracing::info!(
                "shell_open: handed to explorer.exe (pid={}) so it opens unelevated: {target}",
                child.id()
            );
            Ok(())
        }
        Err(e) => {
            tracing::warn!("shell_open: could not launch explorer.exe for {target}: {e}");
            Err(format!("failed to launch explorer.exe: {e}"))
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn shell_open(target: &str) -> Result<(), String> {
    tracing::info!("shell_open: {target}");
    open::that(target).map_err(|e| format!("failed to open target: {e}"))
}

/// Open an external web link, rejecting anything that isn't http(s).
///
/// The shell would happily launch a local executable or a `file:` path, and some
/// callers forward URLs parsed out of server responses, so the scheme is checked
/// before the string ever reaches the shell.
pub fn open_external_url(url: &str) -> Result<(), String> {
    if !is_http_url(url) {
        tracing::warn!("open_external_url: refusing non-http(s) target: {url}");
        return Err(format!("refusing to open non-http(s) URL: {url}"));
    }
    shell_open(url)
}

/// Whether `url` is an `http://` or `https://` URL.
fn is_http_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::is_http_url;

    #[test]
    fn accepts_http_and_https_any_case() {
        assert!(is_http_url("http://example.com"));
        assert!(is_http_url("https://example.com/a?b=c"));
        assert!(is_http_url("HTTPS://EXAMPLE.COM"));
    }

    #[test]
    fn rejects_everything_else() {
        // A local executable is the case that matters: the shell would run it.
        assert!(!is_http_url(r"C:\Windows\System32\calc.exe"));
        assert!(!is_http_url("file:///C:/Windows/System32/calc.exe"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url("\\\\evil-share\\payload.exe"));
        assert!(!is_http_url(""));
        // Scheme must be at the start, not merely present.
        assert!(!is_http_url(" https://example.com"));
        assert!(!is_http_url("nothttps://example.com"));
    }
}
