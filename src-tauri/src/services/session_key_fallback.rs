//! Native fallbacks for bootstrapping a beanfun session key when the primary
//! reqwest path can't connect (e.g. TLS fingerprinting): first a
//! PowerShell/.NET fetch, then a hidden WebView2 page load.

use base64::Engine;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tauri::Manager;

use crate::core::error::NetworkError;
use crate::models::session::{Region, Session};
use crate::services::beanfun_service::{self, LoginError, QrCodeData};
use crate::services::webview_util::WEBVIEW_USER_AGENT;

type SessionKeyFallbackSender = tokio::sync::oneshot::Sender<WebviewFetchResult>;

static SESSION_KEY_WEBVIEW_RESULTS: OnceLock<Mutex<HashMap<String, SessionKeyFallbackSender>>> =
    OnceLock::new();

#[derive(Debug, Clone)]
struct WebviewFetchResult {
    url: String,
    html: String,
}

/// Login, retrying over a natively-fetched session key when the primary path
/// fails before the session key bootstrap.
pub async fn login_with_native_fallback(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    account: &str,
    password: &str,
    region: &Region,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
    tokens: &beanfun_service::RecaptchaTokens,
) -> Result<Session, LoginError> {
    match beanfun_service::login(client, account, password, region, cookie_jar, tokens).await {
        Ok(session) => Ok(session),
        Err(err) if should_try_session_key_fallback(&err) => {
            tracing::warn!(
                region = ?region,
                account = %account,
                "Primary login path failed before session key bootstrap; trying native fallbacks"
            );
            let session_key = fetch_session_key_with_fallback(app, region).await?;
            beanfun_service::login_with_session_key(
                client,
                account,
                password,
                region,
                cookie_jar,
                &session_key,
                tokens,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

/// Start a QR login, retrying over a natively-fetched session key when the
/// primary path fails before the session key bootstrap.
pub async fn qr_login_start_with_native_fallback(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    region: &Region,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<QrCodeData, LoginError> {
    match beanfun_service::qr_login_start(client, region, cookie_jar).await {
        Ok(data) => Ok(data),
        Err(err) if should_try_session_key_fallback(&err) => {
            tracing::warn!(region = ?region, "QR start failed before session key bootstrap; trying native fallbacks");
            let session_key = fetch_session_key_with_fallback(app, region).await?;
            beanfun_service::qr_login_start_with_session_key(client, region, &session_key).await
        }
        Err(err) => Err(err),
    }
}

/// Deliver a captured page back to the fallback waiting on `request_id`.
/// Called by the `session_key_webview_done` Tauri command.
pub fn deliver_webview_result(request_id: &str, url: String, html: String) {
    tracing::info!(
        request_id = %request_id,
        final_url = %url,
        html_len = html.len(),
        "Session-key WebView2 fallback captured page"
    );

    if let Some(sender) = SESSION_KEY_WEBVIEW_RESULTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .remove(request_id)
    {
        let _ = sender.send(WebviewFetchResult { url, html });
    }
}

fn should_try_session_key_fallback(err: &LoginError) -> bool {
    matches!(
        err,
        LoginError::Network(NetworkError::ConnectionFailed { .. })
            | LoginError::Network(NetworkError::Timeout { .. })
    )
}

async fn fetch_session_key_with_fallback(
    app: &tauri::AppHandle,
    region: &Region,
) -> Result<String, LoginError> {
    if let Some(session_key) = fetch_session_key_via_powershell(region).await? {
        tracing::info!(region = ?region, session_key_len = session_key.len(), "Session key obtained via PowerShell/.NET fallback");
        return Ok(session_key);
    }

    let webview = fetch_session_key_via_webview2(app, region).await?;
    let session_key = match region {
        Region::HK => beanfun_service::parse_hk_session_key_html(&webview.html)?,
        Region::TW => beanfun_service::parse_tw_session_key_url(&webview.url)?,
    };
    tracing::info!(region = ?region, session_key_len = session_key.len(), "Session key obtained via WebView2 fallback");
    Ok(session_key)
}

async fn fetch_session_key_via_powershell(region: &Region) -> Result<Option<String>, LoginError> {
    #[cfg(target_os = "windows")]
    {
        let script = match region {
            Region::HK => format!(
                r#"$ProgressPreference='SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
$req = [System.Net.HttpWebRequest]::Create('{0}')
$req.Method = 'GET'
$req.UserAgent = '{1}'
$req.AutomaticDecompression = [System.Net.DecompressionMethods]::GZip -bor [System.Net.DecompressionMethods]::Deflate
$req.Headers['Accept-Encoding'] = 'identity'
$resp = $req.GetResponse()
$reader = New-Object System.IO.StreamReader($resp.GetResponseStream(), [System.Text.Encoding]::UTF8)
$body = $reader.ReadToEnd()
$reader.Close()
$resp.Close()
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Write-Output $body"#,
                "https://bfweb.hk.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0",
                WEBVIEW_USER_AGENT
            ),
            Region::TW => format!(
                r#"$ProgressPreference='SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
$req = [System.Net.HttpWebRequest]::Create('{0}')
$req.Method = 'GET'
$req.UserAgent = '{1}'
$req.AllowAutoRedirect = $true
$req.AutomaticDecompression = [System.Net.DecompressionMethods]::GZip -bor [System.Net.DecompressionMethods]::Deflate
$req.Headers['Accept-Encoding'] = 'identity'
$resp = $req.GetResponse()
$finalUrl = $resp.ResponseUri.AbsoluteUri
$resp.Close()
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Write-Output $finalUrl"#,
                "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0",
                WEBVIEW_USER_AGENT
            ),
        };

        let encoded =
            base64::engine::general_purpose::STANDARD.encode(encode_powershell_script(&script));
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
                .output()
        })
        .await
        .map_err(|e| {
            LoginError::Network(NetworkError::ConnectionFailed {
                url: format!("powershell session-key fallback task failed ({e})"),
            })
        })?
        .map_err(|e| {
            LoginError::Network(NetworkError::ConnectionFailed {
                url: format!("powershell session-key fallback failed to launch ({e})"),
            })
        })?;

        if !output.status.success() {
            tracing::warn!(
                region = ?region,
                status = ?output.status.code(),
                stderr = %String::from_utf8_lossy(&output.stderr),
                "PowerShell/.NET session-key fallback failed"
            );
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            tracing::warn!(region = ?region, "PowerShell/.NET session-key fallback returned empty output");
            return Ok(None);
        }

        let session_key = match region {
            Region::HK => beanfun_service::parse_hk_session_key_html(&stdout)?,
            Region::TW => beanfun_service::parse_tw_session_key_url(&stdout)?,
        };

        Ok(Some(session_key))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = region;
        Ok(None)
    }
}

#[cfg(target_os = "windows")]
fn encode_powershell_script(script: &str) -> Vec<u8> {
    script
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect()
}

async fn fetch_session_key_via_webview2(
    app: &tauri::AppHandle,
    region: &Region,
) -> Result<WebviewFetchResult, LoginError> {
    let request_id = format!("session-key-{}", uuid::Uuid::new_v4());
    let label = format!("session-key-webview-{}", uuid::Uuid::new_v4());
    let results = SESSION_KEY_WEBVIEW_RESULTS.get_or_init(|| Mutex::new(HashMap::new()));
    let (tx, rx) = tokio::sync::oneshot::channel();
    results.lock().unwrap().insert(request_id.clone(), tx);

    let start_url = match region {
        Region::HK => {
            "https://bfweb.hk.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0"
        }
        Region::TW => "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0",
    };

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| {
            LoginError::Network(NetworkError::ConnectionFailed {
                url: format!("failed to get app data dir for session-key webview ({e})"),
            })
        })?
        .join("session-key-fallback");
    let _ = std::fs::create_dir_all(&data_dir);

    let init_script = format!(
        r#"
(() => {{
  const REQUEST_ID = "{}";
  const START_URL = "{}";
  if (window.location.href === 'about:blank') {{
    setTimeout(() => {{ window.location.href = START_URL; }}, 50);
    return;
  }}
  const send = () => {{
    const html = document.documentElement ? document.documentElement.outerHTML : document.body?.outerHTML || '';
    if (window.__TAURI_INTERNALS__) {{
      window.__TAURI_INTERNALS__.invoke('session_key_webview_done', {{
        requestId: REQUEST_ID,
        url: window.location.href,
        html
      }}).catch(err => console.error('[SessionKeyFallback] invoke failed', err));
    }}
  }};
  if (document.readyState === 'complete' || document.readyState === 'interactive') {{
    setTimeout(send, 50);
  }} else {{
    window.addEventListener('DOMContentLoaded', () => setTimeout(send, 50), {{ once: true }});
  }}
}})();
"#,
        request_id, start_url
    );

    let window = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("Session Key Fallback")
    .visible(false)
    .focused(false)
    .resizable(false)
    .inner_size(320.0, 240.0)
    .data_directory(data_dir)
    .user_agent(WEBVIEW_USER_AGENT)
    .additional_browser_args("--disable-blink-features=AutomationControlled --no-sandbox")
    .initialization_script(&init_script)
    .build()
    .map_err(|e| {
        LoginError::Network(NetworkError::ConnectionFailed {
            url: format!("failed to create session-key fallback webview ({e})"),
        })
    })?;

    let result = tokio::time::timeout(std::time::Duration::from_secs(20), rx)
        .await
        .map_err(|_| {
            LoginError::Network(NetworkError::Timeout {
                url: "session-key webview fallback timed out".to_string(),
            })
        })?
        .map_err(|_| {
            LoginError::Network(NetworkError::ConnectionFailed {
                url: "session-key webview fallback channel closed".to_string(),
            })
        })?;

    let _ = window.destroy();
    SESSION_KEY_WEBVIEW_RESULTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .remove(&request_id);

    Ok(result)
}
