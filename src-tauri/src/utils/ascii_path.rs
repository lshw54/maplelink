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
//! ASCII. Windows already keeps an ASCII alias for most files — the 8.3 short
//! name — so [`ascii_safe`] uses that when the real path isn't ASCII.

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
