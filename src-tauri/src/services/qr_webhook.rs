//! Send the QR-login deeplink to the player's phone through a webhook.
//!
//! TW players who sign in with a GamaPass passkey have to pass the passkey
//! prompt on every login, because beanfun's OAuth request carries
//! `prompt=login`. The beanfun app's QR login sidesteps that whole flow: the
//! phone confirms the login where the passkey is native. Scanning the code
//! from the launcher window is the awkward part, so this posts the same
//! deeplink to a Discord webhook, a Telegram bot, or any endpoint that takes a
//! JSON `text`, and the player taps it on the phone instead.
//!
//! The deeplink is a `gameplapp://` URL, which chat apps do not turn into a
//! link. The message therefore carries an `https://` bounce page on the
//! official site that redirects to it; the deeplink rides in the URL fragment,
//! which the browser never sends to the server hosting the page.
//!
//! The webhook URL (a bearer secret for Discord; the bot token for Telegram)
//! lives in the prefs store, not `AppConfig`, so it stays out of exported
//! backups.

use std::time::Duration;

use crate::models::config::Language;

/// Prefs keys the settings page and [`send`] share.
pub const PREF_URL: &str = "qr_webhook_url";
pub const PREF_CHAT_ID: &str = "qr_webhook_chat_id";

/// Static page on the official site that redirects to the deeplink held in
/// its fragment (`site/static/open/index.html`).
const BOUNCE_PAGE: &str = "https://lshw54.github.io/maplelink/open/";

/// Upper bound on one webhook call; the login does not wait on it.
const TIMEOUT: Duration = Duration::from_secs(10);
/// How much of an error body is kept for the log / the toast.
const ERROR_BODY_CAP: usize = 300;

/// Which shape of request the endpoint expects, decided from its URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookKind {
    /// `https://discord.com/api/webhooks/…` — `{"content": …}`.
    Discord,
    /// `https://api.telegram.org/bot<token>/sendMessage` — needs a chat id.
    Telegram,
    /// Anything else: `{"text": …}` (Slack-compatible incoming webhooks and
    /// most self-hosted relays).
    Generic,
}

impl WebhookKind {
    pub fn detect(url: &str) -> Self {
        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_lowercase))
            .unwrap_or_default();
        if host == "discord.com" || host == "discordapp.com" || host.ends_with(".discord.com") {
            WebhookKind::Discord
        } else if host == "api.telegram.org" {
            WebhookKind::Telegram
        } else {
            WebhookKind::Generic
        }
    }
}

/// The `https://` link the phone can tap: the bounce page with the deeplink
/// in its fragment.
pub fn bounce_url(deeplink: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(deeplink.as_bytes()).collect();
    format!("{BOUNCE_PAGE}#{encoded}")
}

/// What gets posted. `None` from [`build_payload`] means the endpoint kind
/// needs something the settings do not provide (a Telegram chat id).
pub fn build_payload(kind: WebhookKind, chat_id: &str, text: &str) -> Option<serde_json::Value> {
    Some(match kind {
        WebhookKind::Discord => serde_json::json!({ "content": text }),
        WebhookKind::Telegram => {
            let chat_id = chat_id.trim();
            if chat_id.is_empty() {
                return None;
            }
            serde_json::json!({
                "chat_id": chat_id,
                "text": text,
                "disable_web_page_preview": true,
            })
        }
        WebhookKind::Generic => serde_json::json!({ "text": text }),
    })
}

/// The login message in the app's language.
pub fn login_message(language: Language, link: &str) -> String {
    match language {
        Language::ZhTW => format!(
            "MapleLink 登入請求\n在手機點開下面的連結，用 beanfun App 確認登入（3 分鐘內有效）：\n{link}"
        ),
        Language::ZhCN => format!(
            "MapleLink 登录请求\n在手机点开下面的链接，用 beanfun App 确认登录（3 分钟内有效）：\n{link}"
        ),
        Language::EnUS => format!(
            "MapleLink login request\nOpen the link below on your phone and confirm in the beanfun app (valid for 3 minutes):\n{link}"
        ),
    }
}

/// The message the settings page's test button sends.
pub fn test_message(language: Language) -> String {
    match language {
        Language::ZhTW => {
            "MapleLink 測試訊息：Webhook 已設定好，登入時會把連結送到這裡。".to_string()
        }
        Language::ZhCN => {
            "MapleLink 测试消息：Webhook 已设置好，登录时会把链接发到这里。".to_string()
        }
        Language::EnUS => {
            "MapleLink test message: the webhook is set up; login links will arrive here."
                .to_string()
        }
    }
}

/// Reasons a send cannot even start, or did not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// No webhook URL configured.
    NotConfigured,
    /// The URL is not an `http(s)` URL.
    InvalidUrl,
    /// A Telegram endpoint without a chat id.
    MissingChatId,
    /// The request failed or the endpoint answered outside 2xx.
    Failed(String),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::NotConfigured => write!(f, "no webhook URL configured"),
            SendError::InvalidUrl => write!(f, "webhook URL must start with http:// or https://"),
            SendError::MissingChatId => write!(f, "Telegram webhook needs a chat id"),
            SendError::Failed(why) => write!(f, "{why}"),
        }
    }
}

/// Post `text` to `webhook_url`.
pub async fn send(
    client: &reqwest::Client,
    webhook_url: &str,
    chat_id: &str,
    text: &str,
) -> Result<(), SendError> {
    let webhook_url = webhook_url.trim();
    if webhook_url.is_empty() {
        return Err(SendError::NotConfigured);
    }
    let parsed = url::Url::parse(webhook_url).map_err(|_| SendError::InvalidUrl)?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(SendError::InvalidUrl);
    }
    let kind = WebhookKind::detect(webhook_url);
    let payload = build_payload(kind, chat_id, text).ok_or(SendError::MissingChatId)?;

    let response = client
        .post(webhook_url)
        .timeout(TIMEOUT)
        .json(&payload)
        .send()
        .await
        .map_err(|e| SendError::Failed(crate::services::http_util::with_causes(&e)))?;
    let status = response.status();
    if status.is_success() {
        tracing::info!(kind = ?kind, %status, "qr webhook: message sent");
        return Ok(());
    }
    let body = crate::services::http_util::read_capped_text(response, ERROR_BODY_CAP as u64)
        .await
        .unwrap_or_default();
    Err(SendError::Failed(format!("{status}: {}", body.trim())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_endpoint_kind_from_the_host() {
        assert_eq!(
            WebhookKind::detect("https://discord.com/api/webhooks/1/abc"),
            WebhookKind::Discord
        );
        assert_eq!(
            WebhookKind::detect("https://DISCORDAPP.com/api/webhooks/1/abc"),
            WebhookKind::Discord
        );
        assert_eq!(
            WebhookKind::detect("https://api.telegram.org/bot123:abc/sendMessage"),
            WebhookKind::Telegram
        );
        assert_eq!(
            WebhookKind::detect("https://hooks.slack.com/services/x"),
            WebhookKind::Generic
        );
        assert_eq!(WebhookKind::detect("not a url"), WebhookKind::Generic);
    }

    #[test]
    fn bounce_url_keeps_the_deeplink_in_the_fragment() {
        let link = bounce_url("gameplapp://gameplhost/deeplink?type=1&action=web&code=a%2Fb+c");
        assert!(link.starts_with("https://lshw54.github.io/maplelink/open/#"));
        // `&` and `%` are escaped so the fragment survives the chat app and
        // decodes back to the exact deeplink.
        let fragment = link.split('#').nth(1).unwrap();
        assert!(!fragment.contains('&'));
        let decoded: String = url::form_urlencoded::parse(fragment.as_bytes())
            .map(|(k, v)| {
                if v.is_empty() {
                    k.into_owned()
                } else {
                    format!("{k}={v}")
                }
            })
            .collect();
        assert_eq!(
            decoded,
            "gameplapp://gameplhost/deeplink?type=1&action=web&code=a%2Fb+c"
        );
    }

    #[test]
    fn payload_shapes_follow_the_kind() {
        assert_eq!(
            build_payload(WebhookKind::Discord, "", "hi"),
            Some(serde_json::json!({ "content": "hi" }))
        );
        assert_eq!(
            build_payload(WebhookKind::Generic, "", "hi"),
            Some(serde_json::json!({ "text": "hi" }))
        );
        let tg = build_payload(WebhookKind::Telegram, " 42 ", "hi").unwrap();
        assert_eq!(tg["chat_id"], "42");
        assert_eq!(tg["text"], "hi");
        assert_eq!(build_payload(WebhookKind::Telegram, "", "hi"), None);
    }

    #[test]
    fn messages_carry_the_link_in_every_language() {
        for lang in [Language::ZhTW, Language::ZhCN, Language::EnUS] {
            let msg = login_message(lang.clone(), "https://example.test/#x");
            assert!(msg.ends_with("https://example.test/#x"), "{msg}");
            assert!(!test_message(lang).is_empty());
        }
    }

    /// Needs the network; run with `--ignored`. httpbin echoes the JSON it
    /// was sent, so this proves the generic shape goes out as intended.
    #[tokio::test]
    #[ignore]
    async fn send_posts_the_generic_shape_to_a_live_endpoint() {
        let client = reqwest::Client::new();
        send(
            &client,
            "https://httpbin.org/post",
            "",
            "hello from maplelink",
        )
        .await
        .expect("httpbin answers 200");
    }

    #[tokio::test]
    async fn send_refuses_bad_settings_before_any_request() {
        let client = reqwest::Client::new();
        assert_eq!(
            send(&client, "  ", "", "x").await,
            Err(SendError::NotConfigured)
        );
        assert_eq!(
            send(&client, "ftp://example.test/hook", "", "x").await,
            Err(SendError::InvalidUrl)
        );
        assert_eq!(
            send(
                &client,
                "https://api.telegram.org/bot1:a/sendMessage",
                "",
                "x"
            )
            .await,
            Err(SendError::MissingChatId)
        );
    }
}
