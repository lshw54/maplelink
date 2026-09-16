//! Official MapleStory TW client download list.
//!
//! Fetches the *official* download list from beanfun's public download page and
//! hands it to the UI as plain links. We deliberately do NOT download, patch, or
//! replace any game file ourselves — the launcher never touches client binaries,
//! so there's no path for us to ship tampered files (see issue #21). The user
//! copies the link or opens it in their browser and downloads from the official
//! CDN directly.

use serde::{Deserialize, Serialize};
use std::path::Path;

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
    /// The Gamania Games Manager installer. beanfun labels it "(推薦)", but it
    /// takes back the `gamaniagames://` registry key, which turns off
    /// MapleLink's web-launch interception — so the UI says otherwise.
    pub manager: bool,
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
    let client = crate::services::system_proxy::SystemProxy::read()
        .apply(reqwest::Client::builder())
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
            manager: is_manager(&it.name, &it.point),
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

/// Whether an entry is the Gamania Games Manager installer rather than the
/// game itself. Its download lives under `/ggm/`; the name is the fallback for
/// the day that path changes.
fn is_manager(name: &str, url: &str) -> bool {
    url.to_ascii_lowercase().contains("/ggm/")
        || name.contains("遊戲管理器")
        || name.contains("遊戲管理員")
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
/// The only host game files may come from.
///
/// The manifest decides two things at once: where each file is fetched from
/// (`baseUrl`) and which SHA-256 counts as correct. Whoever writes a manifest
/// can therefore make a repair write any bytes they like into the game folder
/// and have them verify. Over HTTPS from beanfun that is beanfun; but a
/// manifest also arrives from the cache in the app data folder, which players
/// have already started handing each other when their own fetch failed. So the
/// address it names is held to beanfun's CDN over HTTPS, whatever it came from.
///
/// If beanfun ever moves the CDN, repairs stop with a clear error until this
/// is updated — the price of a file from a stranger not being able to redirect
/// them.
pub const CDN_HOST: &str = "maplestory-download.beanfun.com";

/// Refuse a manifest `baseUrl` that is not beanfun's CDN over HTTPS.
///
/// Parsed rather than prefix-matched: `https://maplestory-download.beanfun.com.evil.example/`
/// and `https://maplestory-download.beanfun.com@evil.example/` both start with
/// the right text and go somewhere else entirely.
pub fn check_cdn_base(base: &str) -> Result<(), String> {
    let refuse = || format!("product info names a download address outside {CDN_HOST}: {base}");
    let url = reqwest::Url::parse(base).map_err(|_| refuse())?;
    let sound = url.scheme() == "https"
        && url.host_str() == Some(CDN_HOST)
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none();
    if sound {
        Ok(())
    } else {
        Err(refuse())
    }
}

/// MapleStory's manifest, at the address the catalog names for it — over HTTPS.
///
/// Asked first, with the catalog only as the fallback. The catalog used to come
/// first on every load, and it is the weak link: a plain-HTTP file on HiNet's
/// P2P CDN that accelerators do not route. Players whose accelerator opened
/// this very address still timed out on the catalog and got no manifest at all.
///
/// It is also the file every SHA-256 the repair trusts comes from. Fetched over
/// plain HTTP, anyone on the path could swap a file and its hash together and
/// the check would pass; over TLS they cannot. Same bytes either way — checked
/// against the HTTP copy on 2026-09-16.
const PRODUCT_INFO_URL: &str =
    "https://maplestory-download.beanfun.com/maplestory/productInfo.json";
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
    /// Where the manifest itself lives, so a player can open it and check the
    /// same list the scan compares against.
    pub manifest_url: String,
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

/// Fetch beanfun's `productInfo.json` for MapleStory TW, as raw text, with the
/// address it came from.
///
/// The manifest carries the version, the CDN base and the per-file list. It is
/// fetched from its known HTTPS address; only if that fails is the catalog
/// asked where it lives now, in case beanfun has moved it. Shared by the
/// torrent view here and by the client manager, which needs the file list.
pub async fn fetch_product_info_body(data_dir: &Path) -> Result<(String, String), String> {
    fetch_product_info_learning(data_dir, |_| {}).await
}

/// Where the manifest can be asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ManifestSource {
    /// beanfun's own HTTPS address.
    Beanfun,
    /// The HiNet catalog, the way the Gamania game manager asks.
    Catalog,
}

impl ManifestSource {
    fn other(self) -> Self {
        match self {
            Self::Beanfun => Self::Catalog,
            Self::Catalog => Self::Beanfun,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Beanfun => "beanfun",
            Self::Catalog => "catalog",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        [Self::Beanfun, Self::Catalog]
            .into_iter()
            .find(|s| s.key() == key)
    }
}

/// Which source to ask first: the one that last worked on this machine.
///
/// Reachability belongs to the player's route rather than to either source — an
/// accelerator that carries beanfun's CDN may not carry HiNet's, and some
/// networks are the other way round — so nobody is asked to choose. Whatever
/// answered last time is asked first next time, and the other only when it
/// fails. A machine that has never loaded the list starts with beanfun.
const FIRST_CHOICE_KEY: &str = "client.manifest_source";

fn first_choice(data_dir: &Path) -> ManifestSource {
    crate::services::prefs::get(data_dir, FIRST_CHOICE_KEY)
        .and_then(|k| ManifestSource::from_key(&k))
        .unwrap_or(ManifestSource::Beanfun)
}

/// Make `used` the first choice, when it is not already.
fn remember(data_dir: &Path, first: ManifestSource, used: ManifestSource) {
    if used == first {
        return;
    }
    tracing::info!(
        "product info: {first:?} did not answer and {used:?} did; asking {used:?} first from now on"
    );
    if let Err(e) = crate::services::prefs::set(data_dir, FIRST_CHOICE_KEY, used.key()) {
        tracing::warn!("product info: could not remember the source that worked: {e}");
    }
}

/// One try at one source, sent as it starts, so a slow load can say what it is
/// waiting on instead of a spinner that might as well have hung.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestAttempt {
    pub source: ManifestSource,
    pub attempt: usize,
    pub attempts: usize,
}

/// Tries per source. A dropped connection or a 503 is often gone a second
/// later; a host the route cannot reach at all is not, so more than one retry
/// only makes the player wait longer to be told.
const MANIFEST_ATTEMPTS: usize = 2;
const MANIFEST_RETRY_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

/// A manifest, where it came from, and which source that was.
#[derive(Debug)]
struct Fetched {
    url: String,
    body: String,
    source: ManifestSource,
}

/// Fetch the manifest from whichever source worked last, falling back to the
/// other, and remember which one answered. `data_dir` is where that is kept.
pub async fn fetch_product_info_learning(
    data_dir: &Path,
    on_attempt: impl Fn(ManifestAttempt) + Send + Sync,
) -> Result<(String, String), String> {
    let client = crate::services::system_proxy::SystemProxy::read()
        .apply(reqwest::Client::builder())
        .user_agent(UA)
        // A host the route cannot reach usually never answers the connect at
        // all. Giving up on that quickly is what keeps two sources, each tried
        // twice, from adding up to a minute of nothing.
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let first = first_choice(data_dir);
    let fetched = fetch_product_info_from(
        &client,
        PRODUCT_INFO_URL,
        PRODUCT_LIST_URL,
        first,
        &on_attempt,
    )
    .await?;
    remember(data_dir, first, fetched.source);
    Ok((fetched.url, fetched.body))
}

/// Ask `first`, then the other source if it fails. Against given addresses, so
/// the order and the retries can be exercised without beanfun.
async fn fetch_product_info_from(
    client: &reqwest::Client,
    manifest_url: &str,
    catalog_url: &str,
    first: ManifestSource,
    on_attempt: &(dyn Fn(ManifestAttempt) + Send + Sync),
) -> Result<Fetched, String> {
    let ask = |source: ManifestSource| async move {
        match source {
            ManifestSource::Beanfun => retrying(source, on_attempt, || {
                fetch_manifest_at(client, manifest_url)
            })
            .await
            .map(|body| (manifest_url.to_string(), body)),
            ManifestSource::Catalog => {
                retrying(source, on_attempt, || {
                    fetch_via_catalog(client, catalog_url)
                })
                .await
            }
        }
    };

    let first_error = match ask(first).await {
        Ok((url, body)) => {
            return Ok(Fetched {
                url,
                body,
                source: first,
            })
        }
        Err(e) => e,
    };
    let second = first.other();
    tracing::info!("product info: {first:?} failed ({first_error}); asking {second:?}");
    match ask(second).await {
        Ok((url, body)) => Ok(Fetched {
            url,
            body,
            source: second,
        }),
        Err(second_error) => Err(format!(
            "{}: {first_error}; {}: {second_error}",
            first.key(),
            second.key()
        )),
    }
}

/// Why one attempt did not produce a manifest, and whether another might.
struct Failure {
    message: String,
    retryable: bool,
}

impl Failure {
    fn transient(message: String) -> Self {
        Self {
            message,
            retryable: true,
        }
    }

    fn permanent(message: String) -> Self {
        Self {
            message,
            retryable: false,
        }
    }

    /// A refusal from the server: worth another go only when it says "not now".
    fn status(what: &str, status: reqwest::StatusCode) -> Self {
        let message = format!("{what} returned HTTP {status}");
        match status.is_server_error()
            || status == reqwest::StatusCode::REQUEST_TIMEOUT
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            true => Self::transient(message),
            false => Self::permanent(message),
        }
    }
}

/// Run one source up to [`MANIFEST_ATTEMPTS`] times, telling `on_attempt` as
/// each try starts. A failure that cannot improve — a 404, a page that is not
/// a manifest — is returned at once rather than asked for again.
async fn retrying<T, F, Fut>(
    source: ManifestSource,
    on_attempt: &(dyn Fn(ManifestAttempt) + Send + Sync),
    mut run: F,
) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, Failure>>,
{
    let mut attempt = 1;
    loop {
        on_attempt(ManifestAttempt {
            source,
            attempt,
            attempts: MANIFEST_ATTEMPTS,
        });
        match run().await {
            Ok(found) => return Ok(found),
            Err(failure) if failure.retryable && attempt < MANIFEST_ATTEMPTS => {
                tracing::info!(
                    "product info: {source:?} attempt {attempt} failed ({}), trying again",
                    failure.message
                );
                attempt += 1;
                tokio::time::sleep(MANIFEST_RETRY_WAIT).await;
            }
            Err(failure) => {
                return Err(match attempt > 1 {
                    true => format!("{} (after {attempt} attempts)", failure.message),
                    false => failure.message,
                })
            }
        }
    }
}

/// One manifest address. Only a body that reads as a manifest counts: a captive
/// portal or a CDN error page answers 200 as readily as the real file does.
async fn fetch_manifest_at(client: &reqwest::Client, url: &str) -> Result<String, Failure> {
    let resp = client.get(url).send().await.map_err(|e| {
        Failure::transient(format!(
            "product info request failed: {}",
            crate::services::http_util::with_causes(&e)
        ))
    })?;
    if !resp.status().is_success() {
        return Err(Failure::status("product info", resp.status()));
    }
    // A body cut off part-way reads as unreadable; that is the connection, not
    // the file, so it is worth another go.
    let body = crate::services::http_util::read_capped_text(resp, MANIFEST_CAP)
        .await
        .ok_or_else(|| Failure::transient("product info body unreadable".to_string()))?;
    full_client_info(&body, url).map_err(Failure::permanent)?;
    Ok(body)
}

/// The long way round: ask the catalog where the manifest is, then fetch it.
async fn fetch_via_catalog(
    client: &reqwest::Client,
    catalog_url: &str,
) -> Result<(String, String), Failure> {
    let resp = client.get(catalog_url).send().await.map_err(|e| {
        Failure::transient(format!(
            "product list request failed: {}",
            crate::services::http_util::with_causes(&e)
        ))
    })?;
    if !resp.status().is_success() {
        return Err(Failure::status("product list", resp.status()));
    }
    let body = crate::services::http_util::read_capped_text(resp, CATALOG_CAP)
        .await
        .ok_or_else(|| Failure::transient("product list body unreadable".to_string()))?;
    let named = product_info_url(&body).map_err(Failure::permanent)?;

    // The catalog names a plain-HTTP address. The host serves HTTPS as well, so
    // that is asked first and the address as given only if it fails.
    let secure = named
        .strip_prefix("http://")
        .map(|rest| format!("https://{rest}"));
    let mut last = Failure::permanent(String::new());
    for url in secure.iter().chain(std::iter::once(&named)) {
        match fetch_manifest_at(client, url).await {
            Ok(body) => return Ok((url.clone(), body)),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Fetch the full-client torrent details for MapleStory TW.
pub async fn fetch_full_client_info(data_dir: &Path) -> Result<FullClientInfo, String> {
    let (url, body) = fetch_product_info_body(data_dir).await?;
    full_client_info(&body, &url)
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
fn full_client_info(product_info_json: &str, manifest_url: &str) -> Result<FullClientInfo, String> {
    let info: ProductInfo = serde_json::from_str(product_info_json)
        .map_err(|e| format!("failed to parse product info: {e}"))?;
    if info.version.trim().is_empty() {
        return Err("product info has no version".to_string());
    }
    let mut base = info.base_url.trim().to_string();
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err("product info has no usable baseUrl".to_string());
    }
    // The torrent is fetched from here too.
    check_cdn_base(&base)?;
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
        manifest_url: manifest_url.to_string(),
    })
}

#[cfg(test)]
mod full_client_tests {
    use super::*;

    /// A tiny server that answers by path, and records the paths it was asked.
    /// `None` for a path hangs up without answering.
    fn serve_paths(
        routes: Vec<(&'static str, Option<String>)>,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = asked.clone();
        std::thread::spawn(move || {
            for sock in listener.incoming() {
                let Ok(mut sock) = sock else { return };
                let mut buf = [0u8; 4096];
                let n = sock.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                log.lock().unwrap().push(path.clone());
                let reply = routes
                    .iter()
                    .find(|(p, _)| *p == path)
                    .map(|(_, r)| r.clone())
                    .unwrap_or_else(|| Some(response("404 Not Found", "")));
                if let Some(reply) = reply {
                    let _ = sock.write_all(reply.as_bytes());
                }
            }
        });
        (base, asked)
    }

    fn response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    const MANIFEST: &str = r#"{"productName":"新楓之谷","productId":"MS","version":"V282",
        "baseUrl":"https://maplestory-download.beanfun.com/maplestory/download/",
        "executionPath":"P2PdPoyK5obH/MapleStory.exe","files":[]}"#;

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
    }

    /// The report that led here: the manifest's own address works, the catalog
    /// does not answer. The catalog must not even be asked.
    #[tokio::test]
    async fn the_manifest_is_fetched_directly_without_the_catalog() {
        let (base, asked) = serve_paths(vec![
            (
                "/maplestory/productInfo.json",
                Some(response("200 OK", MANIFEST)),
            ),
            ("/product_list.json", None),
        ]);
        let got = fetch_product_info_from(
            &test_client(),
            &format!("{base}/maplestory/productInfo.json"),
            &format!("{base}/product_list.json"),
            ManifestSource::Beanfun,
            &|_| {},
        )
        .await
        .unwrap();

        assert!(got.url.ends_with("/maplestory/productInfo.json"));
        assert!(got.body.contains("V282"));
        assert_eq!(got.source, ManifestSource::Beanfun);
        assert_eq!(*asked.lock().unwrap(), vec!["/maplestory/productInfo.json"]);
    }

    /// If beanfun moves the manifest, the catalog still finds it.
    #[tokio::test]
    async fn a_moved_manifest_is_found_through_the_catalog() {
        // The old address is gone (404) ...
        let (old, _) = serve_paths(vec![]);
        // ... the manifest now lives somewhere else ...
        let (moved, _) = serve_paths(vec![(
            "/moved/productInfo.json",
            Some(response("200 OK", MANIFEST)),
        )]);
        // ... and the catalog knows where. Its server is started last because
        // the catalog has to name the new address, port and all.
        let catalog = format!(
            r#"{{"products":[{{"productId":"MS","infoData":"{moved}/moved/productInfo.json"}}]}}"#
        );
        let (catalog_base, _) = serve_paths(vec![(
            "/product_list.json",
            Some(response("200 OK", &catalog)),
        )]);

        let got = fetch_product_info_from(
            &test_client(),
            &format!("{old}/maplestory/productInfo.json"),
            &format!("{catalog_base}/product_list.json"),
            ManifestSource::Beanfun,
            &|_| {},
        )
        .await
        .unwrap();

        // An https upgrade of a plain-HTTP test server cannot work, so this also
        // shows the address as named being used once the upgrade fails.
        assert_eq!(got.url, format!("{moved}/moved/productInfo.json"));
        assert!(got.body.contains("V282"));
        // ...and says which source it was, which is what gets remembered.
        assert_eq!(got.source, ManifestSource::Catalog);
    }

    /// Answer each connection in turn from `replies`, whatever it asks for.
    /// `None` hangs up without replying.
    fn serve_in_turn(
        replies: Vec<Option<String>>,
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = hits.clone();
        std::thread::spawn(move || {
            for reply in replies {
                let Ok((mut sock, _)) = listener.accept() else {
                    return;
                };
                seen.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let _ = sock.read(&mut [0u8; 4096]);
                if let Some(reply) = reply {
                    let _ = sock.write_all(reply.as_bytes());
                }
            }
        });
        (base, hits)
    }

    /// Every attempt reported, in order.
    type AttemptLog = std::sync::Arc<std::sync::Mutex<Vec<(ManifestSource, usize)>>>;

    fn attempts_seen() -> (AttemptLog, impl Fn(ManifestAttempt) + Send + Sync) {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = log.clone();
        (log, move |a: ManifestAttempt| {
            sink.lock().unwrap().push((a.source, a.attempt))
        })
    }

    /// One dropped connection is not the end of the load.
    #[tokio::test]
    async fn a_dropped_connection_is_retried_and_reported() {
        let (base, hits) = serve_in_turn(vec![None, Some(response("200 OK", MANIFEST))]);
        let (log, on_attempt) = attempts_seen();
        let got = fetch_product_info_from(
            &test_client(),
            &format!("{base}/maplestory/productInfo.json"),
            &format!("{base}/product_list.json"),
            ManifestSource::Beanfun,
            &on_attempt,
        )
        .await
        .unwrap();

        assert!(got.body.contains("V282"));
        assert_eq!(hits.load(std::sync::atomic::Ordering::Relaxed), 2);
        assert_eq!(
            *log.lock().unwrap(),
            vec![(ManifestSource::Beanfun, 1), (ManifestSource::Beanfun, 2)]
        );
    }

    /// A file that is not there is not asked for twice — the other source is
    /// tried instead.
    #[tokio::test]
    async fn a_missing_manifest_is_not_retried_but_the_other_source_is_tried() {
        let (base, _) = serve_in_turn(vec![Some(response("404 Not Found", "")); 2]);
        let (log, on_attempt) = attempts_seen();
        let err = fetch_product_info_from(
            &test_client(),
            &format!("{base}/maplestory/productInfo.json"),
            &format!("{base}/product_list.json"),
            ManifestSource::Beanfun,
            &on_attempt,
        )
        .await
        .err()
        .unwrap();

        assert!(err.contains("404"), "{err}");
        assert_eq!(
            *log.lock().unwrap(),
            vec![(ManifestSource::Beanfun, 1), (ManifestSource::Catalog, 1)]
        );
    }

    /// Once the catalog is the one that works, it is asked first and beanfun's
    /// address is not tried at all while it keeps working.
    #[tokio::test]
    async fn a_remembered_catalog_is_asked_first() {
        let (moved, _) = serve_paths(vec![(
            "/moved/productInfo.json",
            Some(response("200 OK", MANIFEST)),
        )]);
        let catalog = format!(
            r#"{{"products":[{{"productId":"MS","infoData":"{moved}/moved/productInfo.json"}}]}}"#
        );
        let (base, asked) = serve_paths(vec![(
            "/product_list.json",
            Some(response("200 OK", &catalog)),
        )]);
        let (log, on_attempt) = attempts_seen();
        let got = fetch_product_info_from(
            &test_client(),
            &format!("{base}/maplestory/productInfo.json"),
            &format!("{base}/product_list.json"),
            ManifestSource::Catalog,
            &on_attempt,
        )
        .await
        .unwrap();

        assert_eq!(got.source, ManifestSource::Catalog);
        assert_eq!(*asked.lock().unwrap(), vec!["/product_list.json"]);
        assert_eq!(*log.lock().unwrap(), vec![(ManifestSource::Catalog, 1)]);
    }

    /// The source that answered becomes the first choice; one that was already
    /// first leaves nothing to write.
    #[test]
    fn the_source_that_answered_is_asked_first_next_time() {
        let dir =
            std::env::temp_dir().join(format!("maplelink_first_choice_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(first_choice(&dir), ManifestSource::Beanfun);
        remember(&dir, ManifestSource::Beanfun, ManifestSource::Catalog);
        assert_eq!(first_choice(&dir), ManifestSource::Catalog);
        remember(&dir, ManifestSource::Catalog, ManifestSource::Beanfun);
        assert_eq!(first_choice(&dir), ManifestSource::Beanfun);

        // An old value this build does not know is ignored, not trusted.
        crate::services::prefs::set(&dir, FIRST_CHOICE_KEY, "auto").unwrap();
        assert_eq!(first_choice(&dir), ManifestSource::Beanfun);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A 200 that is not a manifest — a captive portal, an error page — must
    /// not be taken as one.
    #[tokio::test]
    async fn a_page_that_is_not_a_manifest_is_not_accepted() {
        let (base, _) = serve_paths(vec![(
            "/maplestory/productInfo.json",
            Some(response(
                "200 OK",
                "<html>please log in to the hotel wifi</html>",
            )),
        )]);
        let err = fetch_product_info_from(
            &test_client(),
            &format!("{base}/maplestory/productInfo.json"),
            &format!("{base}/product_list.json"),
            ManifestSource::Beanfun,
            &|_| {},
        )
        .await
        .unwrap_err();

        assert!(err.contains("catalog:"), "{err}");
    }

    const MANIFEST_URL: &str = "http://maplestory-download.beanfun.com/maplestory/productInfo.json";

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
        let got = full_client_info(INFO, MANIFEST_URL).unwrap();
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
                manifest_url: MANIFEST_URL.into(),
            }
        );
    }

    #[test]
    fn a_base_url_without_a_trailing_slash_gets_one() {
        let body = INFO.replace("/maplestory/download/\"", "/maplestory/download\"");
        let got = full_client_info(&body, MANIFEST_URL).unwrap();
        assert!(got
            .torrent_url
            .ends_with("/download/torrent/MS_V282.torrent"));
    }

    #[test]
    fn only_beanfuns_cdn_over_https_is_accepted_as_the_download_address() {
        assert!(
            check_cdn_base("https://maplestory-download.beanfun.com/maplestory/download/").is_ok()
        );
        for bad in [
            // somewhere else
            "https://evil.example/maplestory/download/",
            // the right host, but readable and changeable on the way
            "http://maplestory-download.beanfun.com/maplestory/download/",
            // text that starts right and goes elsewhere
            "https://maplestory-download.beanfun.com.evil.example/",
            "https://maplestory-download.beanfun.com@evil.example/",
            // the right host on a port it does not serve the CDN from
            "https://maplestory-download.beanfun.com:8443/",
            "not a url",
        ] {
            assert!(check_cdn_base(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn a_manifest_pointing_downloads_elsewhere_is_refused() {
        let body = INFO.replace(
            "https://maplestory-download.beanfun.com",
            "https://evil.example",
        );
        let err = full_client_info(&body, MANIFEST_URL).unwrap_err();
        assert!(err.contains("outside"), "{err}");
    }

    #[test]
    fn missing_version_or_base_url_is_an_error() {
        assert!(full_client_info(&INFO.replace("\"V282\"", "\"\""), MANIFEST_URL).is_err());
        assert!(full_client_info(
            &INFO.replace("https://maplestory-download", "ftp://x"),
            MANIFEST_URL
        )
        .is_err());
    }

    #[test]
    fn an_empty_execution_path_yields_empty_names_not_a_panic() {
        let got = full_client_info(
            &INFO.replace("P2PdPoyK5obH/MapleStory.exe", ""),
            MANIFEST_URL,
        )
        .unwrap();
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
pub async fn fetch_full_client_torrent(
    data_dir: &Path,
) -> Result<(FullClientInfo, Vec<u8>), String> {
    let info = fetch_full_client_info(data_dir).await?;
    if info.folder_name.is_empty() {
        return Err("product info names no game folder".to_string());
    }
    let client = crate::services::system_proxy::SystemProxy::read()
        .apply(reqwest::Client::builder())
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
        // A throwaway folder, so this run's source is not remembered for the app.
        let data =
            std::env::temp_dir().join(format!("maplelink_live_torrent_{}", std::process::id()));
        let (info, fixed) = fetch_full_client_torrent(&data).await.unwrap();
        let _ = std::fs::remove_dir_all(&data);
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

#[cfg(test)]
mod manager_tests {
    use super::*;

    #[test]
    fn the_game_manager_entry_is_recognised() {
        assert!(is_manager(
            "遊戲橘子遊戲管理器(推薦)",
            "https://tw.beanfun.com/ggm/GGMSetup_1.5.0.2.exe"
        ));
        // Either signal alone is enough.
        assert!(is_manager("something else", "https://x/GGM/setup.exe"));
        assert!(is_manager("遊戲橘子遊戲管理員", "https://x/other.exe"));
        // The game's own downloads are not the manager.
        assert!(!is_manager(
            "【官方載點】V282.2 手動更新",
            "https://download.beanfun.com/maplestory/patch.exe"
        ));
        assert!(!is_manager(
            "【官方載點】V281~V282",
            "https://x/v281to282.exe"
        ));
    }
}
