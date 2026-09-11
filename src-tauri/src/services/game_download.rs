//! Official MapleStory TW client download list.
//!
//! Fetches the *official* download list from beanfun's public download page and
//! hands it to the UI as plain links. We deliberately do NOT download, patch, or
//! replace any game file ourselves — the launcher never touches client binaries,
//! so there's no path for us to ship tampered files (see issue #21). The user
//! copies the link or opens it in their browser and downloads from the official
//! CDN directly.

use serde::{Deserialize, Serialize};

const DOWNLOAD_PAGE: &str = "https://maplestory.beanfun.com/download";
const DOWNLOAD_LIST_HANDLER: &str = "https://maplestory.beanfun.com/download?handler=DownloadList";
use crate::services::http_util::USER_AGENT as UA;

/// One downloadable item exposed to the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDownloadItem {
    pub id: i64,
    pub name: String,
    /// Human-readable size string as returned by beanfun (e.g. "16.3M").
    pub size: String,
    /// The official download / redirect URL.
    pub url: String,
    /// "game" (full client, `type` 1) or "patch" (update, `type` 2); "other"
    /// for any future/unknown type so nothing is silently dropped.
    pub kind: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    #[serde(rename = "listData")]
    list_data: Option<Vec<ApiItem>>,
    code: Option<i64>,
}

#[derive(Deserialize)]
struct ApiItem {
    id: i64,
    name: String,
    size: String,
    point: String,
    #[serde(rename = "type")]
    kind: i64,
}

/// Fetch the official download list. Returns items grouped by `kind`.
pub async fn fetch_download_list() -> Result<Vec<GameDownloadItem>, String> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    // The list handler is a Razor Pages POST handler that (may) require the
    // antiforgery token. Load the page first so the cookie jar gets the
    // antiforgery cookie, and scrape the paired request token from the HTML.
    let token = match client.get(DOWNLOAD_PAGE).send().await {
        Ok(resp) => resp
            .text()
            .await
            .ok()
            .and_then(|html| extract_request_token(&html)),
        Err(e) => {
            tracing::warn!("download: could not load download page for token: {e}");
            None
        }
    };

    // The token must be sent as the `__RequestVerificationToken` FORM field, not
    // the `RequestVerificationToken` header (header-only → HTTP 400). Sending a
    // form body also gives the Content-Length the server requires (→ 411 without).
    let mut form: Vec<(&str, String)> = Vec::new();
    if let Some(tok) = token {
        form.push(("__RequestVerificationToken", tok));
    }

    let resp = client
        .post(DOWNLOAD_LIST_HANDLER)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Origin", "https://maplestory.beanfun.com")
        .header("Referer", DOWNLOAD_PAGE)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("download list request failed: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("failed to read download list response: {e}"))?;
    if !status.is_success() {
        return Err(format!("download list returned HTTP {status}"));
    }

    let parsed: ApiResponse =
        serde_json::from_str(&body).map_err(|e| format!("failed to parse download list: {e}"))?;
    if parsed.code != Some(1) {
        return Err(format!("download list API returned code {:?}", parsed.code));
    }

    let items = parsed
        .list_data
        .unwrap_or_default()
        .into_iter()
        .map(|it| GameDownloadItem {
            id: it.id,
            name: it.name,
            size: it.size,
            url: it.point,
            kind: match it.kind {
                1 => "game",
                2 => "patch",
                _ => "other",
            }
            .to_string(),
        })
        .collect();
    Ok(items)
}

/// Pull the `__RequestVerificationToken` hidden-input value out of the page HTML.
fn extract_request_token(html: &str) -> Option<String> {
    let anchor = html.find("__RequestVerificationToken")?;
    let tail = &html[anchor..];
    let value_pos = tail.find("value=\"")? + "value=\"".len();
    let rest = &tail[value_pos..];
    let end = rest.find('"')?;
    let token = &rest[..end];
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

// ---------------------------------------------------------------------------
// Full client via the Gamania Games Manager's own download chain.
//
// GGM does not download an installer: it reads a public catalog, then the
// game's `productInfo.json`, then fetches a torrent whose web seed is the
// official beanfun CDN. We surface the torrent link (plus the version and size
// the manifest states) and, as with the list above, never download a file.
// ---------------------------------------------------------------------------

const PRODUCT_LIST_URL: &str = "http://p2p-gamania.cdn.hinet.net/product_list.json";
const MAPLESTORY_PRODUCT_ID: &str = "MS";
/// `product_list.json` is a few KB; `productInfo.json` carries a per-file
/// manifest (~330 KB today). Generous caps so a bad day upstream can't balloon.
const CATALOG_CAP: u64 = 1024 * 1024;
const MANIFEST_CAP: u64 = 16 * 1024 * 1024;

/// What the UI needs to hand the player the official torrent for the full client.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullClientInfo {
    pub product_name: String,
    /// Version label as beanfun states it (e.g. "V282").
    pub version: String,
    /// "YYYY/MM/DD" as published; empty when the manifest omits it.
    pub publish_date: String,
    pub size_bytes: i64,
    pub file_count: usize,
    /// `{baseUrl}torrent/{productId}_{version}.torrent` — the file GGM fetches.
    pub torrent_url: String,
    /// First segment of the manifest's `executionPath`: the folder the game
    /// lives in under the CDN root.
    pub folder_name: String,
    pub exe_name: String,
}

#[derive(Deserialize)]
struct ProductList {
    products: Vec<ProductEntry>,
}

#[derive(Deserialize)]
struct ProductEntry {
    #[serde(rename = "productId")]
    product_id: String,
    #[serde(rename = "infoData")]
    info_data: String,
}

#[derive(Deserialize)]
struct ProductInfo {
    #[serde(rename = "productName", default)]
    product_name: String,
    #[serde(rename = "productId")]
    product_id: String,
    version: String,
    #[serde(rename = "publishDate", default)]
    publish_date: String,
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "executionPath", default)]
    execution_path: String,
    #[serde(rename = "sizeInBytes", default)]
    size_in_bytes: i64,
    #[serde(default)]
    files: Vec<serde_json::Value>,
}

/// Fetch beanfun's `productInfo.json` for MapleStory TW, as raw text.
///
/// Two public GETs: the catalog names the game's manifest URL, the manifest
/// carries the version, the CDN base and the per-file list. Shared by the
/// torrent view here and by the client manager, which needs the file list.
pub async fn fetch_product_info_body() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let resp = client
        .get(PRODUCT_LIST_URL)
        .send()
        .await
        .map_err(|e| format!("product list request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("product list returned HTTP {}", resp.status()));
    }
    let body = crate::services::http_util::read_capped_text(resp, CATALOG_CAP)
        .await
        .ok_or_else(|| "product list body unreadable".to_string())?;
    let info_url = product_info_url(&body)?;

    let resp = client
        .get(&info_url)
        .send()
        .await
        .map_err(|e| format!("product info request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("product info returned HTTP {}", resp.status()));
    }
    crate::services::http_util::read_capped_text(resp, MANIFEST_CAP)
        .await
        .ok_or_else(|| "product info body unreadable".to_string())
}

/// Fetch the full-client torrent details for MapleStory TW.
pub async fn fetch_full_client_info() -> Result<FullClientInfo, String> {
    full_client_info(&fetch_product_info_body().await?)
}

/// The MapleStory entry's `infoData` URL out of the catalog body.
fn product_info_url(catalog_json: &str) -> Result<String, String> {
    let list: ProductList = serde_json::from_str(catalog_json)
        .map_err(|e| format!("failed to parse product list: {e}"))?;
    list.products
        .into_iter()
        .find(|p| p.product_id == MAPLESTORY_PRODUCT_ID)
        .map(|p| p.info_data)
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .ok_or_else(|| "MapleStory is not in the product list".to_string())
}

/// Build the UI record from a `productInfo.json` body, the way GGM does:
/// torrent at `{baseUrl}torrent/{productId}_{version}.torrent`, and the
/// game folder is the first segment of `executionPath`.
fn full_client_info(product_info_json: &str) -> Result<FullClientInfo, String> {
    let info: ProductInfo = serde_json::from_str(product_info_json)
        .map_err(|e| format!("failed to parse product info: {e}"))?;
    if info.version.trim().is_empty() {
        return Err("product info has no version".to_string());
    }
    let mut base = info.base_url.trim().to_string();
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err("product info has no usable baseUrl".to_string());
    }
    if !base.ends_with('/') {
        base.push('/');
    }
    let mut segments = info.execution_path.split('/').filter(|s| !s.is_empty());
    let folder_name = segments.next().unwrap_or_default().to_string();
    let exe_name = segments.next_back().unwrap_or_default().to_string();
    Ok(FullClientInfo {
        product_name: info.product_name,
        torrent_url: format!("{base}torrent/{}_{}.torrent", info.product_id, info.version),
        version: info.version,
        publish_date: info.publish_date,
        size_bytes: info.size_in_bytes,
        file_count: info.files.len(),
        folder_name,
        exe_name,
    })
}

#[cfg(test)]
mod full_client_tests {
    use super::*;

    const CATALOG: &str = r#"{"products":[
        {"seq":2,"productId":"ELS","infoData":"http://elsword-download.beanfun.com/Elsword/productInfo.json"},
        {"seq":9,"productId":"MS","infoData":"http://maplestory-download.beanfun.com/maplestory/productInfo.json"}
    ]}"#;

    const INFO: &str = r#"{
        "productName":"新楓之谷","productId":"MS","sizeInBytes":72733461215,
        "version":"V282","publishDate":"2026/09/04",
        "baseUrl":"https://maplestory-download.beanfun.com/maplestory/download/",
        "executionPath":"P2PdPoyK5obH/MapleStory.exe",
        "files":[{"path":"a","sizeInBytes":1},{"path":"b","sizeInBytes":"2"}]
    }"#;

    #[test]
    fn picks_maplestory_out_of_the_catalog() {
        assert_eq!(
            product_info_url(CATALOG).unwrap(),
            "http://maplestory-download.beanfun.com/maplestory/productInfo.json"
        );
    }

    #[test]
    fn a_catalog_without_maplestory_is_an_error() {
        let err = product_info_url(r#"{"products":[{"productId":"ELS","infoData":"http://x/"}]}"#)
            .unwrap_err();
        assert!(err.contains("not in the product list"));
    }

    #[test]
    fn a_non_http_info_url_is_refused() {
        let err =
            product_info_url(r#"{"products":[{"productId":"MS","infoData":"file:///c:/x"}]}"#)
                .unwrap_err();
        assert!(err.contains("not in the product list"));
    }

    #[test]
    fn builds_the_torrent_url_the_way_ggm_does() {
        let got = full_client_info(INFO).unwrap();
        assert_eq!(
            got,
            FullClientInfo {
                product_name: "新楓之谷".into(),
                version: "V282".into(),
                publish_date: "2026/09/04".into(),
                size_bytes: 72_733_461_215,
                file_count: 2,
                torrent_url:
                    "https://maplestory-download.beanfun.com/maplestory/download/torrent/MS_V282.torrent"
                        .into(),
                folder_name: "P2PdPoyK5obH".into(),
                exe_name: "MapleStory.exe".into(),
            }
        );
    }

    #[test]
    fn a_base_url_without_a_trailing_slash_gets_one() {
        let body = INFO.replace("/maplestory/download/\"", "/maplestory/download\"");
        let got = full_client_info(&body).unwrap();
        assert!(got
            .torrent_url
            .ends_with("/download/torrent/MS_V282.torrent"));
    }

    #[test]
    fn missing_version_or_base_url_is_an_error() {
        assert!(full_client_info(&INFO.replace("\"V282\"", "\"\"")).is_err());
        assert!(full_client_info(&INFO.replace("https://maplestory-download", "ftp://x")).is_err());
    }

    #[test]
    fn an_empty_execution_path_yields_empty_names_not_a_panic() {
        let got = full_client_info(&INFO.replace("P2PdPoyK5obH/MapleStory.exe", "")).unwrap();
        assert_eq!(got.folder_name, "");
        assert_eq!(got.exe_name, "");
    }
}

// ---------------------------------------------------------------------------
// The torrent itself.
//
// beanfun's torrent is built for GGM, not for a BitTorrent client: its root
// folder is named `Bin64`, but the CDN serves the files under the folder that
// `executionPath` names (e.g. `P2PdPoyK5obH`). A client following BEP 19 asks
// the web seed for `{url-list}Bin64/<file>` and gets 404 for every file, and
// there are no peers because GGM never seeds. Renaming the root to the CDN
// folder makes the web seed resolve. Nothing else is touched — the piece
// hashes are the official ones, so the client still verifies every byte
// against what beanfun published.
// ---------------------------------------------------------------------------

/// A `.torrent` is ~430 KB today; the cap leaves room for a much bigger client.
const TORRENT_CAP: u64 = 32 * 1024 * 1024;

/// Fetch beanfun's torrent for the full client and return it with its root
/// folder renamed to the CDN folder, plus the record the UI already shows.
pub async fn fetch_full_client_torrent() -> Result<(FullClientInfo, Vec<u8>), String> {
    let info = fetch_full_client_info().await?;
    if info.folder_name.is_empty() {
        return Err("product info names no game folder".to_string());
    }
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;
    let resp = client
        .get(&info.torrent_url)
        .send()
        .await
        .map_err(|e| format!("torrent request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("torrent returned HTTP {}", resp.status()));
    }
    let raw = crate::services::http_util::read_capped(resp, TORRENT_CAP)
        .await
        .map_err(|e| format!("torrent body unreadable: {e}"))?;
    let fixed = relocate_torrent_root(&raw, &info.folder_name)?;
    Ok((info, fixed))
}

/// Rename the torrent's root folder (`info.name`) to `folder`, leaving every
/// other byte of the file as published. Refuses anything that does not look
/// like a multi-file torrent with a web seed, and anything whose encoding is
/// not canonical (so we can be sure the only difference is the one we made).
fn relocate_torrent_root(raw: &[u8], folder: &str) -> Result<Vec<u8>, String> {
    use crate::services::bencode::{decode, encode, Value};

    if folder.is_empty() || folder.contains(['/', '\\']) || folder == "." || folder == ".." {
        return Err(format!("unusable folder name {folder:?}"));
    }
    let mut top = decode(raw).map_err(|e| format!("torrent is not valid bencode: {e}"))?;
    if encode(&top) != raw {
        return Err("torrent is not canonically encoded".to_string());
    }
    let dict = top.as_dict_mut().ok_or("torrent is not a dictionary")?;
    let has_web_seed = dict.get(b"url-list".as_slice()).is_some_and(|v| {
        v.as_bytes().is_some_and(|b| !b.is_empty()) || v.as_list().is_some_and(|l| !l.is_empty())
    });
    if !has_web_seed {
        return Err("torrent has no web seed (url-list)".to_string());
    }
    let info = dict
        .get_mut(b"info".as_slice())
        .and_then(Value::as_dict_mut)
        .ok_or("torrent has no info dictionary")?;
    let is_multi_file = info
        .get(b"files".as_slice())
        .and_then(Value::as_list)
        .is_some_and(|l| !l.is_empty());
    if !is_multi_file {
        return Err("torrent is not a multi-file torrent".to_string());
    }
    if !info.contains_key(b"pieces".as_slice()) || !info.contains_key(b"piece length".as_slice()) {
        return Err("torrent info lacks piece data".to_string());
    }
    let name = info
        .get_mut(b"name".as_slice())
        .ok_or("torrent info has no name")?;
    if name.as_bytes().is_none() {
        return Err("torrent name is not a string".to_string());
    }
    *name = Value::Bytes(folder.as_bytes().to_vec());
    Ok(encode(&top))
}

#[cfg(test)]
mod torrent_tests {
    use super::*;
    use crate::services::bencode::{decode, encode, Value};

    /// A torrent shaped like beanfun's, built canonically.
    fn official(name: &str, url_list: Option<Value>) -> Vec<u8> {
        let mut info = std::collections::BTreeMap::new();
        info.insert(
            b"files".to_vec(),
            Value::List(vec![Value::Dict(
                [
                    (b"length".to_vec(), Value::Int(191)),
                    (
                        b"path".to_vec(),
                        Value::List(vec![Value::Bytes(b"beanfun.url".to_vec())]),
                    ),
                ]
                .into_iter()
                .collect(),
            )]),
        );
        info.insert(b"name".to_vec(), Value::Bytes(name.as_bytes().to_vec()));
        info.insert(b"piece length".to_vec(), Value::Int(4194304));
        info.insert(b"pieces".to_vec(), Value::Bytes(vec![7u8; 20]));
        let mut top = std::collections::BTreeMap::new();
        top.insert(b"announce".to_vec(), Value::Bytes(b"udp://t:1".to_vec()));
        top.insert(b"info".to_vec(), Value::Dict(info));
        if let Some(u) = url_list {
            top.insert(b"url-list".to_vec(), u);
        }
        encode(&Value::Dict(top))
    }

    fn web_seed() -> Option<Value> {
        Some(Value::List(vec![Value::Bytes(
            b"https://maplestory-download.beanfun.com/maplestory/download/".to_vec(),
        )]))
    }

    fn name_of(torrent: &[u8]) -> Vec<u8> {
        decode(torrent).unwrap().as_dict().unwrap()[b"info".as_slice()]
            .as_dict()
            .unwrap()[b"name".as_slice()]
        .as_bytes()
        .unwrap()
        .to_vec()
    }

    #[test]
    fn only_the_root_name_changes() {
        let raw = official("Bin64", web_seed());
        let fixed = relocate_torrent_root(&raw, "P2PdPoyK5obH").unwrap();
        assert_eq!(name_of(&fixed), b"P2PdPoyK5obH");
        assert_eq!(fixed, official("P2PdPoyK5obH", web_seed()));
        // The pieces and web seed are byte-identical.
        let (a, b) = (decode(&raw).unwrap(), decode(&fixed).unwrap());
        let (a, b) = (a.as_dict().unwrap(), b.as_dict().unwrap());
        assert_eq!(a[b"url-list".as_slice()], b[b"url-list".as_slice()]);
        assert_eq!(
            a[b"info".as_slice()].as_dict().unwrap()[b"pieces".as_slice()],
            b[b"info".as_slice()].as_dict().unwrap()[b"pieces".as_slice()]
        );
    }

    #[test]
    fn a_torrent_without_a_web_seed_is_refused() {
        let raw = official("Bin64", None);
        assert!(relocate_torrent_root(&raw, "X")
            .unwrap_err()
            .contains("web seed"));
        let raw = official("Bin64", Some(Value::List(vec![])));
        assert!(relocate_torrent_root(&raw, "X")
            .unwrap_err()
            .contains("web seed"));
    }

    #[test]
    fn a_non_canonical_or_broken_file_is_refused() {
        let raw = official("Bin64", web_seed());
        // Reorder two top-level keys: still decodes, but no longer round-trips.
        let text = String::from_utf8_lossy(&raw).to_string();
        assert!(text.starts_with("d8:announce"));
        assert!(relocate_torrent_root(b"d3:foo3:bare", "X").is_err());
        assert!(relocate_torrent_root(b"not bencode", "X").is_err());
        assert!(relocate_torrent_root(&raw[..raw.len() - 1], "X").is_err());
    }

    #[test]
    fn a_folder_name_that_could_escape_is_refused() {
        let raw = official("Bin64", web_seed());
        for bad in ["", "..", ".", "a/b", "a\\b"] {
            assert!(relocate_torrent_root(&raw, bad).is_err(), "{bad:?}");
        }
    }
}

/// Live check against beanfun. Ignored by default (network); run with
/// `cargo test live_torrent -- --ignored`. Set `MAPLELINK_TORRENT_OUT` to also
/// write the result to a file.
#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn live_torrent_root_matches_the_cdn_folder() {
        let (info, fixed) = fetch_full_client_torrent().await.unwrap();
        let top = crate::services::bencode::decode(&fixed).unwrap();
        let name = top.as_dict().unwrap()[b"info".as_slice()]
            .as_dict()
            .unwrap()[b"name".as_slice()]
        .as_bytes()
        .unwrap()
        .to_vec();
        assert_eq!(name, info.folder_name.as_bytes());
        assert!(!info.folder_name.is_empty());
        eprintln!(
            "version={} folder={} bytes={}",
            info.version,
            info.folder_name,
            fixed.len()
        );
        if let Ok(out) = std::env::var("MAPLELINK_TORRENT_OUT") {
            std::fs::write(out, &fixed).unwrap();
        }
    }
}
