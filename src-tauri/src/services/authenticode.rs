//! Authenticode: who signed an executable, as Windows itself judges it.
//!
//! The client manager uses this for the one file beanfun publishes without a
//! hash: `ExePatch.dat`, the game's current executable, served over plain HTTP
//! from the patch CDN. Its signature is what makes it safe to put in place.

use std::path::Path;

/// Verify `path`'s embedded Authenticode signature and return the leaf
/// signer's display name (the certificate's CN, e.g. `"NEXON Korea
/// Corporation"`).
///
/// The chain must end at a root this machine trusts. Revocation is not looked
/// up online: a scan or repair must not hang on a CRL server a player's
/// accelerator cannot reach, and the name check that follows is what actually
/// pins the publisher.
#[cfg(target_os = "windows")]
pub fn verified_signer(path: &Path) -> Result<String, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Security::Cryptography::{
        CertGetNameStringW, CERT_NAME_SIMPLE_DISPLAY_TYPE,
    };
    use windows_sys::Win32::Security::WinTrust::{
        WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust,
        WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
        WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide.as_ptr(),
        hFile: INVALID_HANDLE_VALUE,
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    data.Anonymous.pFile = &mut file;
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;

    // SAFETY: `data` points at `file`, which points at `wide`; all three live
    // until the CLOSE call below, which releases the state VERIFY allocated.
    // The provider structures read in between belong to that state and are
    // only dereferenced after a null check.
    unsafe {
        let status = WinVerifyTrust(
            std::ptr::null_mut(),
            &mut action,
            &mut data as *mut WINTRUST_DATA as *mut core::ffi::c_void,
        );
        let signer = if status != 0 {
            Err(format!(
                "signature does not verify (0x{:08X})",
                status as u32
            ))
        } else {
            let prov = WTHelperProvDataFromStateData(data.hWVTStateData);
            let sgnr = match prov.is_null() {
                true => std::ptr::null_mut(),
                false => WTHelperGetProvSignerFromChain(prov, 0, 0, 0),
            };
            if sgnr.is_null() || (*sgnr).csCertChain == 0 || (*sgnr).pasCertChain.is_null() {
                Err("signature carries no signer certificate".to_string())
            } else {
                let cert = (*(*sgnr).pasCertChain).pCert;
                let mut name = [0u16; 256];
                let len = CertGetNameStringW(
                    cert,
                    CERT_NAME_SIMPLE_DISPLAY_TYPE,
                    0,
                    std::ptr::null(),
                    name.as_mut_ptr(),
                    name.len() as u32,
                );
                // `len` counts the terminating NUL; 1 means an empty name.
                match len > 1 {
                    true => Ok(String::from_utf16_lossy(&name[..len as usize - 1])),
                    false => Err("signer certificate has no name".to_string()),
                }
            }
        };
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(
            std::ptr::null_mut(),
            &mut action,
            &mut data as *mut WINTRUST_DATA as *mut core::ffi::c_void,
        );
        signer
    }
}

#[cfg(not(target_os = "windows"))]
pub fn verified_signer(_path: &Path) -> Result<String, String> {
    Err("Authenticode is only checked on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsigned_file_has_no_signer() {
        let path = std::env::temp_dir().join(format!("ml_unsigned_{}.exe", std::process::id()));
        std::fs::write(&path, b"MZ not really a program").unwrap();
        assert!(verified_signer(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_file_has_no_signer() {
        assert!(verified_signer(Path::new("Z:/definitely/not/here.exe")).is_err());
    }

    /// Needs an installed client; run with `--ignored`.
    #[test]
    #[ignore]
    fn live_the_installed_client_is_signed_by_nexon() {
        let exe = Path::new(r"C:\Program Files\gamania Games\MapleStory\MapleStory.exe");
        assert_eq!(verified_signer(exe).unwrap(), "NEXON Korea Corporation");
    }
}
