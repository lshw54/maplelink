//! Making paths safe to hand to code that still speaks ANSI.
//!
//! A few things MapleLink drives are not Unicode-clean:
//!
//! * Locale Remulator bridges 32/64-bit by reading its own DLL path back with
//!   `GetModuleFileNameA` and formatting it into `rundll32.exe "%hs",#1` — a
//!   narrow string, converted under the code page LR has *already* spoofed to
//!   950. A profile name outside that code page comes back mangled and
//!   rundll32 reports "the specified module could not be found".
//! * `cmd.exe` reads `.bat` files byte-by-byte under the console's OEM code
//!   page, not UTF-8.
//!
//! Neither can be fixed where it happens, so the paths we hand them have to be
//! ASCII. Windows keeps an ASCII alias for many files — the 8.3 short name — so
//! [`ascii_safe`] uses that when the real path isn't ASCII.
//!
//! Do not lean on it, though. Windows only bothers generating an 8.3 alias for a
//! name it cannot already express in the OEM code page, and on a Chinese Windows
//! (OEM 936/950) a Chinese folder name *is* expressible: `GetShortPathNameW`
//! then hands the name straight back. `C:\Users\小明` shortens on an English
//! install and does not on a Chinese one — which is to say, not on the machines
//! that need it. Callers need a second way out: somewhere else to put the file
//! (see `lr_service::lr_dir_candidates`) or, for text a narrow reader will
//! decode anyway, writing it in that reader's code page ([`oem_bytes`]).

use std::path::{Path, PathBuf};

/// Whether every character in `path` is ASCII, i.e. it survives a round-trip
/// through any Windows code page unchanged.
pub fn is_ascii(path: &Path) -> bool {
    path.as_os_str().to_string_lossy().is_ascii()
}

/// The Windows 8.3 short form of `path` (`C:\Users\ABCDEF~1\...`).
///
/// Returns `None` when the path doesn't exist or the volume has 8.3 name
/// creation turned off — in which case Windows hands back the long name
/// unchanged and there is no ASCII alias to be had.
#[cfg(target_os = "windows")]
pub fn short_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // First call sizes the buffer (result includes the NUL), second fills it.
    let needed = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut buf = vec![0u16; needed as usize];
    let written = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), needed) };
    if written == 0 || written >= needed {
        return None;
    }

    Some(PathBuf::from(OsString::from_wide(&buf[..written as usize])))
}

#[cfg(not(target_os = "windows"))]
pub fn short_path(_path: &Path) -> Option<PathBuf> {
    None
}

/// `path` itself when it is already ASCII, otherwise its 8.3 short form if that
/// one is. `None` means this path cannot be expressed in ASCII at all and the
/// caller needs a different location entirely.
///
/// The short form names the *same* directory entry, so nothing has to be copied
/// or migrated — files written through the long path are visible through it.
pub fn ascii_safe(path: &Path) -> Option<PathBuf> {
    if is_ascii(path) {
        return Some(path.to_path_buf());
    }
    short_path(path).filter(|p| is_ascii(p))
}

/// [`ascii_safe`] as a string, falling back to the original path when there is
/// no ASCII alias. For callers that would rather try and fail loudly than not
/// try at all.
pub fn ascii_safe_str(path: &Path) -> String {
    ascii_safe(path)
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// Encode `text` in the OEM code page — the encoding `cmd.exe` assumes when it
/// reads a `.bat`, and the one a console inherits by default.
///
/// Writing a batch file as UTF-8 mangles every non-ASCII byte in it; a game
/// folder or an install path under a Chinese profile name then never reaches
/// `start`. Pure-ASCII text encodes identically either way, so this is a no-op
/// for almost every user.
///
/// Returns `None` when the text cannot be represented in that code page at all,
/// which is the caller's cue that no batch file will carry it.
#[cfg(target_os = "windows")]
pub fn oem_bytes(text: &str) -> Option<Vec<u8>> {
    use windows_sys::Win32::Globalization::{GetOEMCP, WideCharToMultiByte};

    if text.is_ascii() {
        return Some(text.as_bytes().to_vec());
    }

    let wide: Vec<u16> = text.encode_utf16().collect();
    let code_page = unsafe { GetOEMCP() };

    // A UTF code page rejects the substitution-character arguments below, and
    // needs none of them — everything round-trips.
    const CP_UTF7: u32 = 65000;
    const CP_UTF8: u32 = 65001;
    let unicode_cp = matches!(code_page, CP_UTF7 | CP_UTF8);

    let len = unsafe {
        WideCharToMultiByte(
            code_page,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            std::ptr::null_mut(),
        )
    };
    if len <= 0 {
        return None;
    }

    let mut buf = vec![0u8; len as usize];
    // Ask whether anything had to be replaced by a substitute character: a
    // lossy encoding is a path that will not resolve, so report it as failure
    // rather than write a file that silently points nowhere.
    let mut used_default: i32 = 0;
    let written = unsafe {
        WideCharToMultiByte(
            code_page,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            buf.as_mut_ptr(),
            len,
            std::ptr::null(),
            if unicode_cp {
                std::ptr::null_mut()
            } else {
                &mut used_default
            },
        )
    };
    if written <= 0 || used_default != 0 {
        return None;
    }

    buf.truncate(written as usize);
    Some(buf)
}

#[cfg(not(target_os = "windows"))]
pub fn oem_bytes(text: &str) -> Option<Vec<u8>> {
    Some(text.as_bytes().to_vec())
}

/// An environment-variable directory root, normalised for [`Path::join`].
///
/// `%SystemDrive%` is `C:` with no separator; joining onto that yields the
/// drive-relative `C:MapleLink`, not `C:\MapleLink`.
pub fn env_root(var: &str) -> Option<PathBuf> {
    let value = std::env::var(var).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(if value.ends_with(':') {
        format!("{value}\\")
    } else {
        value.to_string()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii_paths_are_returned_unchanged() {
        let p = Path::new(r"C:\Nexon\MapleStory");
        assert!(is_ascii(p));
        assert_eq!(ascii_safe(p).as_deref(), Some(p));
    }

    #[test]
    fn non_ascii_paths_are_not_ascii() {
        assert!(!is_ascii(Path::new(r"C:\Users\小明\AppData")));
    }

    #[test]
    fn a_missing_non_ascii_path_has_no_ascii_alias() {
        // Nothing to shorten — Windows can only produce an 8.3 name for a file
        // that exists, so this must not silently return the mangled long path.
        let p = Path::new(r"C:\maplelink-does-not-exist-小明\lr");
        assert_eq!(ascii_safe(p), None);
        assert_eq!(ascii_safe_str(p), p.to_string_lossy());
    }

    #[test]
    fn the_ascii_alias_names_the_same_directory() {
        let base = std::env::temp_dir().join("maplelink_ascii_path_測試小明");
        let dir = base.join("lr");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_ascii(&dir));

        match ascii_safe(&dir) {
            Some(alias) => {
                assert!(is_ascii(&alias), "alias must be ASCII: {}", alias.display());
                // Nothing is copied — a file written through the long path is
                // readable through the alias.
                std::fs::write(dir.join("probe.bin"), b"ok").unwrap();
                assert_eq!(std::fs::read(alias.join("probe.bin")).unwrap(), b"ok");
            }
            // 8.3 name creation is off on this volume; `lr_dir_candidates` then
            // falls back to an ASCII root instead.
            None => eprintln!("no 8.3 name on {}; skipped", base.display()),
        }

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn ascii_text_encodes_byte_for_byte() {
        // Every code page agrees on ASCII, so the common case must be untouched.
        let script = "@echo off\r\nstart \"\" \"C:\\Program Files\\MapleLink\\maplelink.exe\"\r\n";
        assert_eq!(oem_bytes(script).unwrap(), script.as_bytes());
    }

    #[test]
    fn a_non_ascii_script_is_encoded_or_refused_never_mangled() {
        // What `register` writes when MapleLink sits under a Chinese profile.
        let script = "start \"\" \"C:\\Users\\简体（管理員）\\maplelink.exe\" --web-launch %*\r\n";
        // Latin Windows (OEM 437/850) refuses outright, so the caller reports a
        // failure instead of writing a dud helper. Chinese Windows (OEM 936/950)
        // encodes it, and then cmd must read the ASCII scaffolding as written.
        if let Some(bytes) = oem_bytes(script) {
            assert!(bytes.starts_with(b"start \"\" \"C:\\Users\\"));
            assert!(bytes.ends_with(b"\\maplelink.exe\" --web-launch %*\r\n"));
            // A substituted character would be a path that resolves to nothing;
            // the input has no '?' of its own.
            assert!(!bytes.contains(&b'?'), "lossy encoding slipped through");
        }
    }

    #[test]
    fn env_root_gives_a_joinable_drive_root() {
        std::env::set_var("MAPLELINK_TEST_ROOT", "C:");
        assert_eq!(
            env_root("MAPLELINK_TEST_ROOT").unwrap().join("MapleLink"),
            PathBuf::from(r"C:\MapleLink"),
        );
        std::env::remove_var("MAPLELINK_TEST_ROOT");
    }

    #[test]
    fn env_root_ignores_unset_and_blank_variables() {
        assert_eq!(env_root("MAPLELINK_TEST_UNSET_VAR"), None);
        std::env::set_var("MAPLELINK_TEST_BLANK", "   ");
        assert_eq!(env_root("MAPLELINK_TEST_BLANK"), None);
        std::env::remove_var("MAPLELINK_TEST_BLANK");
    }
}
