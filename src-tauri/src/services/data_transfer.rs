//! Portable export / import of user data (saved accounts + display overrides).
//!
//! The on-disk stores (`accounts.dat`, `display_overrides.dat`) are DPAPI-
//! encrypted and tied to the current Windows user, so they can't just be copied
//! to another machine. This module serializes the DECRYPTED data into a portable
//! JSON envelope for backup / migration, either:
//!   - **plaintext** (`encrypted: false`) — readable, but contains passwords, or
//!   - **passphrase-protected** — the payload is encrypted with AES-256-GCM using
//!     a key derived from the user's passphrase via Argon2id (salt + nonce stored
//!     in the envelope). On import the same passphrase decrypts it.

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::services::account_storage::{DisplayOverrides, SavedAccount};

const APP_TAG: &str = "MapleLink";
const FORMAT_VERSION: u32 = 1;

/// The decrypted payload carried by both envelope kinds.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPayload {
    #[serde(default)]
    pub accounts: Vec<SavedAccount>,
    #[serde(default)]
    pub display_overrides: DisplayOverrides,
}

/// Build the export file contents. `passphrase = None` → plaintext JSON;
/// `Some(pass)` → passphrase-encrypted envelope.
pub fn build_export(payload: &ExportPayload, passphrase: Option<&str>) -> Result<String, String> {
    let now = chrono::Utc::now().to_rfc3339();

    match passphrase {
        None => {
            let mut value =
                serde_json::to_value(payload).map_err(|e| format!("serialize failed: {e}"))?;
            if let Some(obj) = value.as_object_mut() {
                obj.insert("app".into(), APP_TAG.into());
                obj.insert("version".into(), FORMAT_VERSION.into());
                obj.insert("encrypted".into(), false.into());
                obj.insert("exportedAt".into(), now.into());
            }
            serde_json::to_string_pretty(&value).map_err(|e| format!("serialize failed: {e}"))
        }
        Some(pass) => {
            if pass.is_empty() {
                return Err("passphrase must not be empty".into());
            }
            let plain =
                serde_json::to_vec(payload).map_err(|e| format!("serialize failed: {e}"))?;
            let salt = random_bytes(16);
            let nonce = random_bytes(12);
            let key = derive_key(pass, &salt)?;
            let ciphertext = aes_encrypt(&key, &nonce, &plain)?;

            let b64 = base64::engine::general_purpose::STANDARD;
            let envelope = serde_json::json!({
                "app": APP_TAG,
                "version": FORMAT_VERSION,
                "encrypted": true,
                "exportedAt": now,
                "kdf": "argon2id",
                "cipher": "aes-256-gcm",
                "salt": b64.encode(&salt),
                "nonce": b64.encode(&nonce),
                "ciphertext": b64.encode(&ciphertext),
            });
            serde_json::to_string_pretty(&envelope).map_err(|e| format!("serialize failed: {e}"))
        }
    }
}

/// Whether the given export-file contents are passphrase-encrypted.
pub fn is_encrypted(data: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v.get("encrypted").and_then(|e| e.as_bool()))
        .unwrap_or(false)
}

/// Parse an export file into the payload. Returns `Err("PASSPHRASE_REQUIRED")`
/// if the file is encrypted and no passphrase was supplied, or
/// `Err("WRONG_PASSPHRASE")` if decryption fails.
pub fn parse_import(data: &str, passphrase: Option<&str>) -> Result<ExportPayload, String> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| format!("not a valid backup file: {e}"))?;

    let encrypted = value
        .get("encrypted")
        .and_then(|e| e.as_bool())
        .unwrap_or(false);

    if !encrypted {
        // Beanfun's own export uses parallel arrays — detect + convert it.
        if value.get("accountList").is_some() {
            return parse_beanfun(&value).ok_or_else(|| "bad Beanfun export file".to_string());
        }
        // Otherwise our own plaintext envelope (extra fields like app/version are
        // ignored by ExportPayload).
        return serde_json::from_value(value).map_err(|e| format!("bad backup payload: {e}"));
    }

    let pass = passphrase.ok_or_else(|| "PASSPHRASE_REQUIRED".to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD;
    let get = |k: &str| -> Result<Vec<u8>, String> {
        let s = value
            .get(k)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("backup missing field '{k}'"))?;
        b64.decode(s)
            .map_err(|e| format!("bad base64 for '{k}': {e}"))
    };
    let salt = get("salt")?;
    let nonce = get("nonce")?;
    let ciphertext = get("ciphertext")?;

    let key = derive_key(pass, &salt)?;
    let plain = aes_decrypt(&key, &nonce, &ciphertext)?;
    serde_json::from_slice(&plain).map_err(|e| format!("bad backup payload: {e}"))
}

/// Convert Beanfun's own export (parallel arrays) into our payload. Fields we
/// don't have (`accountNameList`, `methodList`, `autoLoginList`, `lastLoginAtList`)
/// are ignored.
fn parse_beanfun(value: &serde_json::Value) -> Option<ExportPayload> {
    let accounts = value.get("accountList")?.as_array()?;
    let arr = |k: &str| value.get(k).and_then(|v| v.as_array());
    let regions = arr("regionList");
    let passwords = arr("passwdList");
    let verifies = arr("verifyList");
    let str_at = |a: Option<&Vec<serde_json::Value>>, i: usize| -> String {
        a.and_then(|a| a.get(i))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let mut out = Vec::new();
    for (i, acc) in accounts.iter().enumerate() {
        let account = acc.as_str().unwrap_or("").trim().to_string();
        if account.is_empty() {
            continue;
        }
        let region = {
            let r = str_at(regions, i);
            if r == "HK" || r == "TW" {
                r
            } else {
                "HK".to_string()
            }
        };
        let password = str_at(passwords, i);
        let verify = str_at(verifies, i);
        out.push(SavedAccount {
            region,
            account,
            remember_password: !password.is_empty(),
            password,
            verify_info: if verify.is_empty() {
                None
            } else {
                Some(verify)
            },
            // Imported records carry no login history of their own.
            last_used_at: None,
        });
    }
    Some(ExportPayload {
        accounts: out,
        display_overrides: DisplayOverrides::default(),
    })
}

fn random_bytes(n: usize) -> Vec<u8> {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..n).map(|_| rng.random::<u8>()).collect()
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("key derivation failed: {e}"))?;
    Ok(key)
}

fn aes_encrypt(key: &[u8; 32], nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|e| format!("encryption failed: {e}"))
}

fn aes_decrypt(key: &[u8; 32], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "WRONG_PASSPHRASE".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ExportPayload {
        ExportPayload {
            accounts: vec![SavedAccount {
                region: "TW".into(),
                account: "user@example.com".into(),
                password: "s3cret".into(),
                remember_password: true,
                verify_info: Some("email".into()),
                last_used_at: None,
            }],
            display_overrides: DisplayOverrides::default(),
        }
    }

    #[test]
    fn plaintext_round_trip() {
        let p = sample();
        let s = build_export(&p, None).unwrap();
        assert!(s.contains("user@example.com"));
        assert!(!is_encrypted(&s));
        let back = parse_import(&s, None).unwrap();
        assert_eq!(back.accounts.len(), 1);
        assert_eq!(back.accounts[0].password, "s3cret");
    }

    #[test]
    fn imports_beanfun_export() {
        let beanfun = r#"{
            "regionList": ["HK", "TW"],
            "accountList": ["a@x.com", "user2"],
            "accountNameList": ["", ""],
            "passwdList": ["pw1", "pw2"],
            "verifyList": ["", "vf@x.com"],
            "methodList": [0, 0],
            "autoLoginList": [false, false],
            "lastLoginAtList": ["2026-07-10T13:59:44.877Z", ""]
        }"#;
        let p = parse_import(beanfun, None).unwrap();
        assert_eq!(p.accounts.len(), 2);
        assert_eq!(p.accounts[0].region, "HK");
        assert_eq!(p.accounts[0].account, "a@x.com");
        assert_eq!(p.accounts[0].password, "pw1");
        assert_eq!(p.accounts[0].verify_info, None);
        assert_eq!(p.accounts[1].region, "TW");
        assert_eq!(p.accounts[1].verify_info.as_deref(), Some("vf@x.com"));
    }

    #[test]
    fn encrypted_round_trip() {
        let p = sample();
        let s = build_export(&p, Some("hunter2")).unwrap();
        assert!(is_encrypted(&s));
        assert!(!s.contains("s3cret")); // password not in plaintext
                                        // Missing passphrase → required
        assert_eq!(parse_import(&s, None).unwrap_err(), "PASSPHRASE_REQUIRED");
        // Wrong passphrase → rejected
        assert_eq!(
            parse_import(&s, Some("nope")).unwrap_err(),
            "WRONG_PASSPHRASE"
        );
        // Correct passphrase → round-trips
        let back = parse_import(&s, Some("hunter2")).unwrap();
        assert_eq!(back.accounts[0].password, "s3cret");
    }
}
