//! Windows DPAPI wrapper for encrypting/decrypting data tied to the current user.
//!
//! Uses `CryptProtectData` / `CryptUnprotectData` from the Windows API,
//! matching the original Beanfun client's ProtectedData.Protect/Unprotect
//! with `DataProtectionScope.CurrentUser`.

use rand::RngExt;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::LocalFree;

/// Encrypt plaintext bytes using Windows DPAPI (CurrentUser scope).
///
/// Returns `(ciphertext, entropy)` where entropy is a random 16-byte key
/// that must be stored separately and provided for decryption.
#[cfg(target_os = "windows")]
pub fn protect(plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let entropy_bytes: Vec<u8> = {
        let mut rng = rand::rng();
        (0..16).map(|_| rng.random::<u8>()).collect()
    };

    let ciphertext = protect_with_entropy(plaintext, &entropy_bytes)?;
    Ok((ciphertext, entropy_bytes))
}

/// Encrypt plaintext bytes with a given entropy using Windows DPAPI.
#[cfg(target_os = "windows")]
pub fn protect_with_entropy(plaintext: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr() as *mut u8,
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            cbData: entropy.len() as u32,
            pbData: entropy.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        let result = CryptProtectData(
            &data_in,
            std::ptr::null(), // description
            &entropy_blob,
            std::ptr::null_mut(), // reserved
            std::ptr::null_mut(), // prompt struct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        );

        if result == 0 {
            return Err("CryptProtectData failed".into());
        }

        let ciphertext =
            std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
        LocalFree(data_out.pbData as _);
        Ok(ciphertext)
    }
}

/// Decrypt ciphertext bytes using Windows DPAPI with the given entropy.
#[cfg(target_os = "windows")]
pub fn unprotect(ciphertext: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            cbData: entropy.len() as u32,
            pbData: entropy.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        let result = CryptUnprotectData(
            &data_in,
            std::ptr::null_mut(), // description
            &entropy_blob,
            std::ptr::null_mut(), // reserved
            std::ptr::null_mut(), // prompt struct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        );

        if result == 0 {
            return Err("CryptUnprotectData failed".into());
        }

        let plaintext =
            std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
        LocalFree(data_out.pbData as _);
        Ok(plaintext)
    }
}

/// Non-Windows stub — returns error (DPAPI is Windows-only).
#[cfg(not(target_os = "windows"))]
pub fn protect(_plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    Err("DPAPI is only available on Windows".into())
}

/// Non-Windows stub.
#[cfg(not(target_os = "windows"))]
pub fn protect_with_entropy(_plaintext: &[u8], _entropy: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI is only available on Windows".into())
}

/// Non-Windows stub.
#[cfg(not(target_os = "windows"))]
pub fn unprotect(_ciphertext: &[u8], _entropy: &[u8]) -> Result<Vec<u8>, String> {
    Err("DPAPI is only available on Windows".into())
}
