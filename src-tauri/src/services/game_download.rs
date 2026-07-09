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
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

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
