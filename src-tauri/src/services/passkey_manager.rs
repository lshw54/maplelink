//! Password-manager extensions as the GamaPass passkey authenticator.
//!
//! GamaPass asks for the passkey on every login (`prompt=login`), and the
//! Windows prompt that WebView2 shows for it needs Windows Hello (a PIN) or
//! Bluetooth for a phone. Players who keep their passkeys in a password
//! manager can instead let that manager's browser extension answer inside the
//! GamaPass window. This module owns the extension's lifecycle on disk:
//!
//! 1. download the CRX from the Chrome Web Store update endpoint,
//! 2. parse the CRX3 header and check that its signing key hashes to the
//!    store id we expect (the id *is* SHA-256 of the key, so a package for
//!    any other extension cannot pass),
//! 3. unpack the zip, write the key into `manifest.json` so WebView2 gives
//!    the extension its store id, and apply the manager's shims (small text
//!    patches that paper over WebView2's gaps — see [`shim`]),
//! 4. record what was installed.
//!
//! Loading the folder into the GamaPass profile and hosting the extension's
//! own pages is [`super::gamepass_extensions`]'s job.

use std::path::{Path, PathBuf};

/// Prefs key holding the player's choice: `windows` or a [`Manager::key`].
pub const PREF_SOURCE: &str = "gamepass_passkey_source";

/// Where GamaPass passkeys are answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasskeySource {
    /// The browser's own prompt (Windows Hello, phone, security key).
    Windows,
    Manager(Manager),
}

impl PasskeySource {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some(v) if !v.is_empty() && v != "windows" => {
                Manager::from_key(v).map_or(PasskeySource::Windows, PasskeySource::Manager)
            }
            _ => PasskeySource::Windows,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            PasskeySource::Windows => "windows",
            PasskeySource::Manager(m) => m.key(),
        }
    }

    pub fn manager(self) -> Option<Manager> {
        match self {
            PasskeySource::Windows => None,
            PasskeySource::Manager(m) => Some(m),
        }
    }
}

/// A supported password manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manager {
    Bitwarden,
}

impl Manager {
    pub const ALL: &'static [Manager] = &[Manager::Bitwarden];

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "bitwarden" => Some(Manager::Bitwarden),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Manager::Bitwarden => "bitwarden",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Manager::Bitwarden => "Bitwarden",
        }
    }

    /// Chrome Web Store id; also the only id WebView2 may load it under.
    pub fn store_id(self) -> &'static str {
        match self {
            Manager::Bitwarden => "nngceckbapebfimnlniiiahkandclblb",
        }
    }

    /// The extension page a player opens to sign in to / unlock the vault.
    pub fn vault_page_url(self) -> String {
        match self {
            Manager::Bitwarden => format!(
                "chrome-extension://{}/popup/index.html?uilocation=popout",
                self.store_id()
            ),
        }
    }

    /// Where the unpacked, shimmed extension lives.
    pub fn install_dir(self, app_data: &Path) -> PathBuf {
        app_data.join("extensions").join(self.key())
    }
}

/// What is on disk for a manager.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecord {
    pub id: String,
    pub version: String,
    pub installed_at: String,
}

const RECORD_FILE: &str = "maplelink-install.json";

/// The install record, if the folder is complete.
pub fn status(app_data: &Path, manager: Manager) -> Option<InstallRecord> {
    let dir = manager.install_dir(app_data);
    let record = std::fs::read_to_string(dir.join(RECORD_FILE)).ok()?;
    let record: InstallRecord = serde_json::from_str(&record).ok()?;
    dir.join("manifest.json").is_file().then_some(record)
}

/// Delete the installed folder. The profile-side removal is
/// [`super::gamepass_extensions::ensure_removed`].
pub fn uninstall(app_data: &Path, manager: Manager) -> Result<(), String> {
    let dir = manager.install_dir(app_data);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("remove {}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Chrome Web Store update endpoint for one extension. The parameter set
/// matters: other combinations answer `204 No Content`.
pub fn download_url(manager: Manager) -> String {
    format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&prodversion=153.0.0.0&acceptformat=crx3&x=id%3D{}%26installsource%3Dondemand%26uc",
        manager.store_id()
    )
}

/// Largest CRX we are willing to take (Bitwarden is ~24 MB).
const MAX_CRX_BYTES: u64 = 120 * 1024 * 1024;

/// Download, verify, unpack and shim `manager` into its install dir.
pub async fn install(
    client: &reqwest::Client,
    app_data: &Path,
    manager: Manager,
) -> Result<InstallRecord, String> {
    let url = download_url(manager);
    let response = client
        .get(&url)
        .header(
            "User-Agent",
            crate::services::webview_util::WEBVIEW_USER_AGENT,
        )
        .send()
        .await
        .map_err(|e| format!("download: {}", crate::services::http_util::with_causes(&e)))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("download: the store answered {status}"));
    }
    let bytes = crate::services::http_util::read_capped(response, MAX_CRX_BYTES).await?;
    tracing::info!(
        "passkey manager: downloaded {} ({} bytes)",
        manager.display_name(),
        bytes.len()
    );

    let crx = crx::parse(&bytes)?;
    if crx.id != manager.store_id() {
        return Err(format!(
            "the package is for extension {}, not {}",
            crx.id,
            manager.store_id()
        ));
    }

    let dir = manager.install_dir(app_data);
    let staging = app_data
        .join("extensions")
        .join(format!("{}.staging", manager.key()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("create {}: {e}", staging.display()))?;

    let result = unpack_and_prepare(&staging, &crx, manager);
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let record = result?;

    // Swap the finished folder in. A half-written folder is never visible
    // under the install path.
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("replace {}: {e}", dir.display()))?;
    }
    std::fs::rename(&staging, &dir).map_err(|e| format!("move into {}: {e}", dir.display()))?;
    tracing::info!(
        "passkey manager: installed {} {} as {}",
        manager.display_name(),
        record.version,
        record.id
    );
    Ok(record)
}

fn unpack_and_prepare(
    dir: &Path,
    crx: &crx::Crx,
    manager: Manager,
) -> Result<InstallRecord, String> {
    unzip_into(&crx.zip, dir)?;

    // Manifest: keep everything, add the key that gives the store id.
    let manifest_path = dir.join("manifest.json");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|e| format!("manifest: {e}"))?;
    let mut manifest: serde_json::Value =
        serde_json::from_str(&manifest).map_err(|e| format!("manifest is not JSON: {e}"))?;
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    {
        use base64::Engine;
        let key = base64::engine::general_purpose::STANDARD.encode(&crx.public_key);
        manifest
            .as_object_mut()
            .ok_or("manifest is not an object")?
            .insert("key".to_string(), serde_json::Value::String(key));
    }
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write manifest: {e}"))?;
    let derived = super::gamepass_extensions::expected_id(dir).ok_or("key did not round-trip")?;
    if derived != crx.id {
        return Err(format!(
            "manifest key derives id {derived}, expected {}",
            crx.id
        ));
    }

    shim::apply(manager, dir)?;

    let record = InstallRecord {
        id: crx.id.clone(),
        version,
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    std::fs::write(
        dir.join(RECORD_FILE),
        serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write record: {e}"))?;
    Ok(record)
}

/// Extract a zip archive under `dir`, refusing entries that would escape it.
fn unzip_into(zip_bytes: &[u8], dir: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("zip entry {:?} escapes the folder", entry.name()));
        };
        let out = dir.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        let mut file =
            std::fs::File::create(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
        std::io::copy(&mut entry, &mut file)
            .map_err(|e| format!("write {}: {e}", out.display()))?;
    }
    Ok(())
}

/// CRX3 container parsing: just enough protobuf to reach the signing keys
/// and the embedded id. See
/// <https://chromium.googlesource.com/chromium/src/+/main/components/crx_file/crx3.proto>.
pub mod crx {
    /// What [`parse`] pulls out of a CRX3 file.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Crx {
        /// The extension id from the signed header, as `a`–`p` letters.
        pub id: String,
        /// SPKI (DER) of the RSA key whose hash is `id`.
        pub public_key: Vec<u8>,
        /// The zip archive after the header.
        pub zip: Vec<u8>,
    }

    const MAGIC: &[u8; 4] = b"Cr24";
    const FIELD_SHA256_WITH_RSA: u64 = 2;
    const FIELD_SIGNED_HEADER_DATA: u64 = 10000;
    const FIELD_PUBLIC_KEY: u64 = 1;
    const FIELD_CRX_ID: u64 = 1;

    pub fn parse(bytes: &[u8]) -> Result<Crx, String> {
        if bytes.len() < 12 || &bytes[..4] != MAGIC {
            return Err("not a CRX file".to_string());
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != 3 {
            return Err(format!("CRX version {version} is not supported"));
        }
        let header_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let header_end = 12usize
            .checked_add(header_len)
            .filter(|&end| end <= bytes.len())
            .ok_or("CRX header length is out of range")?;
        let header = &bytes[12..header_end];
        let zip = bytes[header_end..].to_vec();

        let mut keys: Vec<Vec<u8>> = Vec::new();
        let mut crx_id: Option<Vec<u8>> = None;
        for (field, value) in fields(header)? {
            match field {
                FIELD_SHA256_WITH_RSA => {
                    for (f, v) in fields(value)? {
                        if f == FIELD_PUBLIC_KEY {
                            keys.push(v.to_vec());
                        }
                    }
                }
                FIELD_SIGNED_HEADER_DATA => {
                    for (f, v) in fields(value)? {
                        if f == FIELD_CRX_ID {
                            crx_id = Some(v.to_vec());
                        }
                    }
                }
                _ => {}
            }
        }
        let crx_id = crx_id.ok_or("CRX header has no id")?;
        if crx_id.len() != 16 {
            return Err("CRX id has the wrong length".to_string());
        }
        let id = letters(&crx_id);
        let public_key = keys
            .into_iter()
            .find(|k| letters(&sha256_16(k)) == id)
            .ok_or("no signing key in the CRX header matches its id")?;
        Ok(Crx {
            id,
            public_key,
            zip,
        })
    }

    /// Chromium's id alphabet: each nibble becomes `a`–`p`.
    pub fn letters(bytes16: &[u8]) -> String {
        bytes16
            .iter()
            .flat_map(|b| [b >> 4, b & 0x0f])
            .map(|n| (b'a' + n) as char)
            .collect()
    }

    fn sha256_16(data: &[u8]) -> [u8; 16] {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(data);
        let mut out = [0u8; 16];
        out.copy_from_slice(&digest[..16]);
        out
    }

    fn varint(buf: &[u8], pos: &mut usize) -> Result<u64, String> {
        let mut result = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = *buf.get(*pos).ok_or("truncated varint")?;
            *pos += 1;
            result |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift > 63 {
                return Err("varint too long".to_string());
            }
        }
    }

    /// Length-delimited fields of a protobuf message as `(field, bytes)`;
    /// varint fields are skipped, other wire types are rejected.
    fn fields(buf: &[u8]) -> Result<Vec<(u64, &[u8])>, String> {
        let mut out = Vec::new();
        let mut pos = 0usize;
        while pos < buf.len() {
            let key = varint(buf, &mut pos)?;
            let (field, wire) = (key >> 3, key & 7);
            match wire {
                2 => {
                    let len = varint(buf, &mut pos)? as usize;
                    let end = pos
                        .checked_add(len)
                        .filter(|&e| e <= buf.len())
                        .ok_or("truncated field")?;
                    out.push((field, &buf[pos..end]));
                    pos = end;
                }
                0 => {
                    varint(buf, &mut pos)?;
                }
                other => return Err(format!("unexpected protobuf wire type {other}")),
            }
        }
        Ok(out)
    }

    /// Build a minimal CRX3 for tests: one RSA proof carrying `public_key`
    /// (any bytes), the signed header with the id derived from it, then `zip`.
    #[cfg(test)]
    pub fn build_for_test(public_key: &[u8], zip: &[u8]) -> Vec<u8> {
        fn len_delimited(field: u64, data: &[u8]) -> Vec<u8> {
            let mut out = vec![((field << 3) | 2) as u8];
            let mut len = data.len();
            loop {
                let byte = (len & 0x7f) as u8;
                len >>= 7;
                if len == 0 {
                    out.push(byte);
                    break;
                }
                out.push(byte | 0x80);
            }
            out.extend_from_slice(data);
            out
        }
        let proof = len_delimited(FIELD_PUBLIC_KEY, public_key);
        let signed = len_delimited(FIELD_CRX_ID, &sha256_16(public_key));
        let mut header = len_delimited(FIELD_SHA256_WITH_RSA, &proof);
        // field 10000 needs a two-byte tag; encode the key as a varint.
        let tag = (FIELD_SIGNED_HEADER_DATA << 3) | 2;
        header.push((tag & 0x7f) as u8 | 0x80);
        header.push(((tag >> 7) & 0x7f) as u8 | 0x80);
        header.push((tag >> 14) as u8);
        header.push(signed.len() as u8);
        header.extend_from_slice(&signed);
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&3u32.to_le_bytes());
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(zip);
        out
    }
}

/// Text patches that make a manager's extension work inside WebView2. Each
/// anchor must match exactly once, or the install is refused: a manager
/// update that moved the code is better reported than half-shimmed.
pub mod shim {
    use super::Manager;
    use std::path::Path;

    pub fn apply(manager: Manager, dir: &Path) -> Result<(), String> {
        match manager {
            Manager::Bitwarden => bitwarden(dir),
        }
    }

    /// Bitwarden, verified on 2026.9.2:
    ///
    /// - `chrome.windows.create` in WebView2 resolves with no window, and
    ///   the popout opener reads `.id` from it, which threw and left the
    ///   passkey prompt empty. Give it a stand-in id.
    /// - Register a synchronous top-level message listener so a restarted
    ///   service worker receives the prompt's first message.
    /// - WebView2 never disconnects a content script's port, so a second
    ///   injection ran two copies of the fido2 scripts and answered every
    ///   request twice. A new copy now tears the old one down first — without
    ///   `messenger.destroy()`, whose DisconnectRequest is broadcast over
    ///   `window.postMessage` and would make the *new* page script tear
    ///   itself down as well.
    fn bitwarden(dir: &Path) -> Result<(), String> {
        patch_regex(
            &dir.join("background.js"),
            r"return yield (\w+)\.createWindow\((\w+)\)\}\)\}",
            "return (yield $1.createWindow($2))||{id:-1}})}",
        )?;
        prepend(
            &dir.join("background.js"),
            "chrome.runtime.onMessage.addListener(function(){});\n",
        )?;
        patch_literal(
            &dir.join("content/fido2-content-script.js"),
            &[
                (
                    "const messenger = Messenger.forDOMCommunication(globalContext.window);",
                    "if (typeof globalContext.__mlBwFido2CS === \"function\") { try { globalContext.__mlBwFido2CS(); } catch (_e) { /* empty */ } }\n    const messenger = Messenger.forDOMCommunication(globalContext.window);",
                ),
                (
                    "port.onDisconnect.addListener(handlePortOnDisconnect);",
                    "port.onDisconnect.addListener(handlePortOnDisconnect);\n    globalContext.__mlBwFido2CS = function () { try { port.onDisconnect.removeListener(handlePortOnDisconnect); port.disconnect(); } catch (_e) { /* empty */ } try { messenger.onDestroy.dispatchEvent(new Event(\"destroy\")); if (messenger.messageEventListener) { messenger.broadcastChannel.removeEventListener(messenger.messageEventListener); messenger.messageEventListener = null; } } catch (_e) { /* empty */ } globalContext.__mlBwFido2CS = undefined; };",
                ),
            ],
        )?;
        patch_literal(
            &dir.join("content/fido2-page-script.js"),
            &[
                (
                    "const BrowserPublicKeyCredential = globalContext.PublicKeyCredential;",
                    "if (typeof globalContext.__mlBwFido2PS === \"function\") { try { globalContext.__mlBwFido2PS(); } catch (_e) { /* empty */ } }\n    const BrowserPublicKeyCredential = globalContext.PublicKeyCredential;",
                ),
                (
                    "navigator.credentials.get = getWebAuthnCredential;",
                    "navigator.credentials.get = getWebAuthnCredential;\n    globalContext.__mlBwFido2PS = function () { destroy(); globalContext.__mlBwFido2PS = undefined; };",
                ),
            ],
        )?;
        Ok(())
    }

    fn read(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("shim: read {}: {e}", path.display()))
    }

    fn write(path: &Path, text: &str) -> Result<(), String> {
        std::fs::write(path, text).map_err(|e| format!("shim: write {}: {e}", path.display()))
    }

    fn patch_regex(path: &Path, pattern: &str, replacement: &str) -> Result<(), String> {
        let text = read(path)?;
        let re = regex::Regex::new(pattern).map_err(|e| format!("shim: bad pattern: {e}"))?;
        let count = re.find_iter(&text).count();
        if count != 1 {
            return Err(format!(
                "shim: pattern {pattern:?} matched {count} times in {}, expected 1",
                path.display()
            ));
        }
        write(path, &re.replacen(&text, 1, replacement))
    }

    fn patch_literal(path: &Path, pairs: &[(&str, &str)]) -> Result<(), String> {
        let mut text = read(path)?;
        for (anchor, replacement) in pairs {
            let count = text.matches(anchor).count();
            if count != 1 {
                return Err(format!(
                    "shim: anchor {anchor:?} found {count} times in {}, expected 1",
                    path.display()
                ));
            }
            text = text.replacen(anchor, replacement, 1);
        }
        write(path, &text)
    }

    fn prepend(path: &Path, head: &str) -> Result<(), String> {
        let text = read(path)?;
        write(path, &format!("{head}{text}"))
    }
}

/// MapleLink's own helper extension.
///
/// WebView2 only renders an extension page that the *extension side* opened
/// (`chrome.windows.create`); a page the host navigates to stays blank or is
/// reported as blocked. The vault page a player signs in on therefore has to
/// be opened by an extension, and the manager itself offers no hook for
/// that. This helper watches for a sentinel URL
/// ([`SENTINEL_PREFIX`]`<manager key>`) in any tab and opens that manager's
/// vault page as a popup, which [`super::super::gamepass_extensions`] then
/// hosts. It is written out on demand and loaded next to the manager.
pub mod helper {
    use super::Manager;
    use std::path::{Path, PathBuf};

    /// Navigating a tab here asks the helper to open that manager's vault.
    pub const SENTINEL_PREFIX: &str = "https://maplelink.invalid/open-vault/";
    /// Same, but the page is meant to stay hidden: it only keeps the
    /// manager's service worker alive while GamaPass is open. The helper
    /// marks the URL with [`KEEPALIVE_MARK`] so the host knows not to show it.
    pub const SENTINEL_HIDDEN_PREFIX: &str = "https://maplelink.invalid/keepalive/";
    pub const KEEPALIVE_MARK: &str = "maplelink=keepalive";

    /// Fixed `key` so the helper keeps one id across installs; its only
    /// purpose is a stable id, so any bytes do.
    const KEY_B64: &str = "TWFwbGVMaW5rIHBhc3NrZXkgaGVscGVyIGV4dGVuc2lvbiBrZXkgdjE=";

    pub fn install_dir(app_data: &Path) -> PathBuf {
        app_data.join("extensions").join("maplelink-helper")
    }

    pub fn sentinel_url(manager: Manager) -> String {
        format!("{SENTINEL_PREFIX}{}", manager.key())
    }

    pub fn keepalive_url(manager: Manager) -> String {
        format!("{SENTINEL_HIDDEN_PREFIX}{}", manager.key())
    }

    /// Write the helper (idempotent) and return its folder.
    pub fn ensure_written(app_data: &Path) -> Result<PathBuf, String> {
        let dir = install_dir(app_data);
        std::fs::create_dir_all(&dir).map_err(|e| format!("helper dir: {e}"))?;
        let targets: Vec<String> = Manager::ALL
            .iter()
            .map(|m| format!("{:?}:{:?}", m.key(), m.vault_page_url()))
            .collect();
        let manifest = serde_json::json!({
            "manifest_version": 3,
            "name": "MapleLink passkey helper",
            "description": "Opens the password manager's vault page for MapleLink.",
            "version": "1.0.2",
            "key": KEY_B64,
            "permissions": ["tabs", "windows"],
            "background": { "service_worker": "background.js" }
        });
        let background = format!(
            r#"// Written by MapleLink; see services/passkey_manager.rs.
var TARGETS = {{{}}};
var PREFIX = {prefix:?};
var HIDDEN_PREFIX = {hidden:?};
var MARK = {mark:?};
var seen = {{}};
chrome.tabs.onUpdated.addListener(function (tabId, info, tab) {{
  var url = (info && info.url) || (tab && tab.url) || "";
  var hidden = url.indexOf(HIDDEN_PREFIX) === 0;
  if (!hidden && url.indexOf(PREFIX) !== 0) return;
  var key = url.slice((hidden ? HIDDEN_PREFIX : PREFIX).length);
  var target = TARGETS[key];
  if (!target) return;
  // onUpdated fires several times per navigation; open once per tab.
  if (seen[tabId] === url) return;
  seen[tabId] = url;
  if (hidden) target += (target.indexOf("?") >= 0 ? "&" : "?") + MARK;
  chrome.windows.create({{ url: target, type: "popup", width: 420, height: 640 }}, function () {{
    void chrome.runtime.lastError;
  }});
}});
"#,
            targets.join(","),
            prefix = SENTINEL_PREFIX,
            hidden = SENTINEL_HIDDEN_PREFIX,
            mark = KEEPALIVE_MARK
        );
        write_if_changed(
            &dir.join("manifest.json"),
            &serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
        )?;
        write_if_changed(&dir.join("background.js"), &background)?;
        Ok(dir)
    }

    fn write_if_changed(path: &Path, content: &str) -> Result<(), String> {
        if std::fs::read_to_string(path)
            .map(|c| c == content)
            .unwrap_or(false)
        {
            return Ok(());
        }
        std::fs::write(path, content).map_err(|e| format!("write {}: {e}", path.display()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn helper_is_written_with_a_stable_key_and_the_sentinel() {
            let app_data =
                std::env::temp_dir().join(format!("maplelink_helper_{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&app_data);
            let dir = ensure_written(&app_data).unwrap();
            let id = crate::services::gamepass_extensions::expected_id(&dir).unwrap();
            assert_eq!(id.len(), 32);
            let bg = std::fs::read_to_string(dir.join("background.js")).unwrap();
            assert!(bg.contains(SENTINEL_PREFIX));
            assert!(bg.contains("nngceckbapebfimnlniiiahkandclblb"));
            // Second write is a no-op with identical content.
            let before = std::fs::metadata(dir.join("background.js"))
                .unwrap()
                .modified()
                .unwrap();
            ensure_written(&app_data).unwrap();
            let after = std::fs::metadata(dir.join("background.js"))
                .unwrap()
                .modified()
                .unwrap();
            assert_eq!(before, after);
            assert_eq!(
                sentinel_url(Manager::Bitwarden),
                "https://maplelink.invalid/open-vault/bitwarden"
            );
            let _ = std::fs::remove_dir_all(&app_data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_parses_known_values_and_falls_back_to_windows() {
        assert_eq!(PasskeySource::parse(None), PasskeySource::Windows);
        assert_eq!(
            PasskeySource::parse(Some("windows")),
            PasskeySource::Windows
        );
        assert_eq!(
            PasskeySource::parse(Some("bitwarden")),
            PasskeySource::Manager(Manager::Bitwarden)
        );
        assert_eq!(
            PasskeySource::parse(Some("1password")),
            PasskeySource::Windows
        );
        assert_eq!(
            PasskeySource::Manager(Manager::Bitwarden).key(),
            "bitwarden"
        );
    }

    #[test]
    fn crx_parse_extracts_id_key_and_zip() {
        let key = b"synthetic public key bytes".to_vec();
        let zip = b"PK..payload".to_vec();
        let bytes = crx::build_for_test(&key, &zip);
        let parsed = crx::parse(&bytes).unwrap();
        assert_eq!(parsed.public_key, key);
        assert_eq!(parsed.zip, zip);
        assert_eq!(parsed.id.len(), 32);
        assert!(parsed.id.chars().all(|c| ('a'..='p').contains(&c)));
    }

    #[test]
    fn crx_parse_rejects_wrong_magic_version_and_lengths() {
        assert!(crx::parse(b"nope").is_err());
        let mut bad_version = crx::build_for_test(b"k", b"z");
        bad_version[4] = 2;
        assert!(crx::parse(&bad_version).unwrap_err().contains("version"));
        let mut bad_len = crx::build_for_test(b"k", b"z");
        bad_len[8] = 0xff;
        assert!(crx::parse(&bad_len).is_err());
    }

    #[test]
    fn crx_parse_rejects_a_key_that_does_not_match_the_id() {
        let mut bytes = crx::build_for_test(b"the real key", b"zip");
        // Flip a byte inside the public key: the id in the signed header no
        // longer hashes from it.
        let idx = bytes
            .windows(12)
            .position(|w| w == b"the real key")
            .unwrap();
        bytes[idx] ^= 0x01;
        assert!(crx::parse(&bytes).unwrap_err().contains("matches its id"));
    }

    #[test]
    fn letters_use_the_a_to_p_alphabet() {
        assert_eq!(crx::letters(&[0x00, 0xff, 0x12]), "aappbc");
    }

    #[test]
    fn download_url_names_the_store_id() {
        let url = download_url(Manager::Bitwarden);
        assert!(url.contains("id%3Dnngceckbapebfimnlniiiahkandclblb"));
        assert!(url.contains("installsource%3Dondemand"));
    }

    #[test]
    fn bitwarden_shim_refuses_when_an_anchor_is_missing() {
        let dir = std::env::temp_dir().join(format!("maplelink_shim_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content")).unwrap();
        std::fs::write(dir.join("background.js"), "nothing here").unwrap();
        let err = shim::apply(Manager::Bitwarden, &dir).unwrap_err();
        assert!(err.contains("expected 1"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bitwarden_shim_patches_a_minimal_lookalike() {
        let dir = std::env::temp_dir().join(format!("maplelink_shim_ok_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content")).unwrap();
        std::fs::write(
            dir.join("background.js"),
            "x(){if(a)return yield wU.createWindow(h)})}y()",
        )
        .unwrap();
        std::fs::write(
            dir.join("content/fido2-content-script.js"),
            "    const messenger = Messenger.forDOMCommunication(globalContext.window);\n    port.onDisconnect.addListener(handlePortOnDisconnect);\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("content/fido2-page-script.js"),
            "    const BrowserPublicKeyCredential = globalContext.PublicKeyCredential;\n    navigator.credentials.get = getWebAuthnCredential;\n",
        )
        .unwrap();
        shim::apply(Manager::Bitwarden, &dir).unwrap();
        let bg = std::fs::read_to_string(dir.join("background.js")).unwrap();
        assert!(bg.starts_with("chrome.runtime.onMessage.addListener(function(){});"));
        assert!(bg.contains("return (yield wU.createWindow(h))||{id:-1}})}"));
        let cs = std::fs::read_to_string(dir.join("content/fido2-content-script.js")).unwrap();
        assert!(cs.contains("__mlBwFido2CS = function"));
        let ps = std::fs::read_to_string(dir.join("content/fido2-page-script.js")).unwrap();
        assert!(ps.contains("__mlBwFido2PS = function"));
        // Applying twice must fail: the anchors are now doubled.
        assert!(shim::apply(Manager::Bitwarden, &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_requires_both_record_and_manifest() {
        let app_data =
            std::env::temp_dir().join(format!("maplelink_pm_status_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&app_data);
        assert_eq!(status(&app_data, Manager::Bitwarden), None);
        let dir = Manager::Bitwarden.install_dir(&app_data);
        std::fs::create_dir_all(&dir).unwrap();
        let record = InstallRecord {
            id: "x".repeat(32),
            version: "1.0".into(),
            installed_at: "now".into(),
        };
        std::fs::write(
            dir.join(RECORD_FILE),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
        assert_eq!(status(&app_data, Manager::Bitwarden), None);
        std::fs::write(dir.join("manifest.json"), "{}").unwrap();
        assert_eq!(status(&app_data, Manager::Bitwarden), Some(record));
        uninstall(&app_data, Manager::Bitwarden).unwrap();
        assert_eq!(status(&app_data, Manager::Bitwarden), None);
        let _ = std::fs::remove_dir_all(&app_data);
    }
}
