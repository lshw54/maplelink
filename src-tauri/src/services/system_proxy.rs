//! Windows' own proxy setting, for the requests that ought to follow it.
//!
//! reqwest reads this itself only with its `system-proxy` feature, and this
//! build turns default features off — so no request made from Rust has ever
//! gone through the proxy that a browser, or a WebView2 window, uses. Proxy
//! tools and some accelerators work by setting exactly that. On such a machine
//! beanfun's CDN opens fine in a browser while the client manager cannot reach
//! it, which is the report that led here.
//!
//! Scope, deliberately: only the client manager's requests use this. Sending
//! the login through a proxy is a different decision with different stakes, and
//! it is not made here.

use std::sync::Arc;

const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

/// The current user's proxy setting, as Windows states it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemProxy {
    /// `host:port` for `http://` requests.
    pub http: Option<String>,
    /// `host:port` for `https://` requests.
    pub https: Option<String>,
    /// `ProxyOverride`: hosts that go direct, in Windows' own pattern syntax.
    pub bypass: Vec<String>,
    /// An auto-config (PAC) script is set. Nothing here runs it, so a machine
    /// that relies on one alone is routed direct — and the UI says so.
    pub pac: bool,
}

impl SystemProxy {
    /// What Internet Settings say right now.
    ///
    /// Read fresh every time: proxy tools switch this on and off while the app
    /// runs, and a remembered answer would route through a proxy that has
    /// already gone away.
    #[cfg(windows)]
    pub fn read() -> Self {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(INTERNET_SETTINGS) else {
            return Self::default();
        };
        let text = |name: &str| key.get_value::<String, _>(name).unwrap_or_default();
        Self::parse(
            key.get_value::<u32, _>("ProxyEnable").unwrap_or(0) != 0,
            &text("ProxyServer"),
            &text("ProxyOverride"),
            !text("AutoConfigURL").trim().is_empty(),
        )
    }

    #[cfg(not(windows))]
    pub fn read() -> Self {
        let _ = INTERNET_SETTINGS;
        Self::default()
    }

    fn parse(enabled: bool, server: &str, bypass: &str, pac: bool) -> Self {
        let mut out = Self {
            bypass: bypass
                .split(';')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect(),
            pac,
            ..Self::default()
        };
        let server = server.trim();
        if !enabled || server.is_empty() {
            return out;
        }
        // One address for everything, or a per-scheme list.
        if !server.contains('=') {
            out.http = address(server);
            out.https = out.http.clone();
            return out;
        }
        for part in server.split(';') {
            let Some((scheme, addr)) = part.split_once('=') else {
                continue;
            };
            match scheme.trim().to_ascii_lowercase().as_str() {
                "http" => out.http = address(addr),
                "https" => out.https = address(addr),
                // `socks=` and `ftp=`: this client speaks neither, and handing
                // a SOCKS port an HTTP request fails every download rather than
                // routing it.
                _ => {}
            }
        }
        out
    }

    /// The proxy in use, for saying which one.
    pub fn describe(&self) -> Option<&str> {
        self.https.as_deref().or(self.http.as_deref())
    }

    /// Where a request to `url` goes: through a proxy, or `None` for direct.
    pub fn route(&self, url: &reqwest::Url) -> Option<String> {
        let server = match url.scheme() {
            "https" => self.https.as_ref(),
            "http" => self.http.as_ref(),
            _ => None,
        }?;
        let host = url.host_str()?;
        if self.bypass.iter().any(|p| bypass_matches(p, host)) {
            return None;
        }
        Some(format!("http://{server}"))
    }

    /// `builder`, routed the way Windows would route each of its requests.
    pub fn apply(self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        if self.http.is_none() && self.https.is_none() {
            return builder;
        }
        let rules = Arc::new(self);
        builder.proxy(reqwest::Proxy::custom(move |url| rules.route(url)))
    }
}

/// `host:port`, without whatever scheme a tool left on the front.
fn address(raw: &str) -> Option<String> {
    let s = raw.trim();
    let s = s
        .strip_prefix("http://")
        .or_else(|| s.strip_prefix("https://"))
        .unwrap_or(s)
        .trim_end_matches('/');
    (!s.is_empty()).then(|| s.to_string())
}

/// One `ProxyOverride` entry against a host: `*` wildcards, case-insensitive,
/// and `<local>` for any name without a dot — the rules Windows applies.
fn bypass_matches(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    if pattern == "<local>" {
        return !host.contains('.');
    }
    wildcard(pattern.as_bytes(), host.as_bytes())
}

fn wildcard(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0, 0);
    // Where the last `*` was, and how much text it has swallowed so far.
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some((p, t));
            p += 1;
        } else if p < pattern.len() && pattern[p] == text[t] {
            p += 1;
            t += 1;
        } else if let Some((sp, st)) = star {
            p = sp + 1;
            t = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == b'*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> reqwest::Url {
        reqwest::Url::parse(s).unwrap()
    }

    #[test]
    fn one_address_serves_both_schemes() {
        let p = SystemProxy::parse(true, "127.0.0.1:7890", "", false);
        assert_eq!(p.http.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(p.https.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(
            p.route(&url("https://maplestory-download.beanfun.com/x")),
            Some("http://127.0.0.1:7890".to_string())
        );
    }

    #[test]
    fn a_switched_off_proxy_is_no_proxy() {
        let p = SystemProxy::parse(false, "127.0.0.1:7890", "", false);
        assert_eq!(p.describe(), None);
        assert_eq!(p.route(&url("https://example.com/")), None);
    }

    #[test]
    fn a_per_scheme_list_routes_each_scheme_its_own_way() {
        let p = SystemProxy::parse(
            true,
            "http=10.0.0.1:8080;https=http://10.0.0.2:8443/",
            "",
            false,
        );
        assert_eq!(
            p.route(&url("http://p2p-gamania.cdn.hinet.net/product_list.json")),
            Some("http://10.0.0.1:8080".to_string())
        );
        assert_eq!(
            p.route(&url("https://maplestory-download.beanfun.com/")),
            Some("http://10.0.0.2:8443".to_string())
        );
    }

    /// Only an HTTP proxy is something this client can talk to.
    #[test]
    fn a_socks_only_setting_goes_direct() {
        let p = SystemProxy::parse(true, "socks=127.0.0.1:1080", "", false);
        assert_eq!(p.describe(), None);
        assert_eq!(p.route(&url("https://example.com/")), None);
    }

    #[test]
    fn the_override_list_sends_matching_hosts_direct() {
        let p = SystemProxy::parse(
            true,
            "127.0.0.1:7890",
            "localhost;127.*;*.hinet.net;<local>",
            false,
        );
        assert_eq!(p.route(&url("http://p2p-gamania.cdn.hinet.net/a")), None);
        assert_eq!(p.route(&url("http://127.0.0.1:1420/")), None);
        assert_eq!(p.route(&url("http://intranet/")), None);
        assert!(p
            .route(&url("https://maplestory-download.beanfun.com/"))
            .is_some());
    }

    #[test]
    fn wildcards_match_the_way_windows_means_them() {
        assert!(bypass_matches(
            "*.Beanfun.com",
            "maplestory-download.beanfun.com"
        ));
        assert!(bypass_matches("192.168.*", "192.168.1.20"));
        assert!(bypass_matches("*", "anything.example"));
        assert!(!bypass_matches("*.beanfun.com", "beanfun.com.evil.example"));
        assert!(!bypass_matches("<local>", "example.com"));
    }

    /// The parsing is only worth anything if reqwest then really sends the
    /// request to the proxy. A listener standing in for one sees the
    /// absolute-form request line a proxy is sent, instead of the site.
    #[tokio::test]
    async fn an_applied_proxy_really_carries_the_request() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let n = sock.read(&mut buf).unwrap();
            let _ = sock.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
            String::from_utf8_lossy(&buf[..n]).to_string()
        });

        let client = SystemProxy::parse(true, &format!("127.0.0.1:{port}"), "", false)
            .apply(reqwest::Client::builder())
            .build()
            .unwrap();
        let status = client
            .get("http://maplestory-download.invalid/maplestory/productInfo.json")
            .send()
            .await
            .unwrap()
            .status();

        assert_eq!(status, 204);
        let request = seen.join().unwrap();
        assert!(
            request.starts_with(
                "GET http://maplestory-download.invalid/maplestory/productInfo.json HTTP/1.1"
            ),
            "{request}"
        );
    }

    #[test]
    fn a_pac_script_is_noticed_even_with_no_proxy_to_use() {
        let p = SystemProxy::parse(false, "", "", true);
        assert!(p.pac);
        assert_eq!(p.describe(), None);
    }
}
