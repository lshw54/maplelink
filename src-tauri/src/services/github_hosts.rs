//! GitHub DNS overrides, so a direct connection works where DNS is poisoned.
//!
//! Update traffic in mainland China currently leans on the ghproxy mirrors,
//! which are a lottery — fast one hour, dead the next. What actually breaks
//! direct GitHub access there is DNS, not routing, so feeding the app the right
//! IPs is usually enough to skip the mirrors entirely.
//!
//! The daily-updated list from `maxiaof/github-hosts` is fetched (through a
//! mirror, since GitHub itself is the thing that's unreachable) and handed to a
//! dedicated reqwest client as DNS overrides. Nothing on the machine is
//! touched: the system hosts file is left alone and only this process sees the
//! mapping.
//!
//! Two rules keep a third-party list from being a foothold:
//! - only GitHub's own domains are overridable, so a tampered list cannot point
//!   `beanfun.com` (or anything else the app talks to) at an attacker;
//! - the client built here validates certificates, unlike the app-wide one, so
//!   a wrong or hostile IP fails the handshake and we fall back to the mirrors
//!   rather than downloading an exe from it.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::time::Duration;

/// The daily-updated hosts list.
const HOSTS_URL: &str = "https://raw.githubusercontent.com/maxiaof/github-hosts/master/hosts";

/// Domains this override is allowed to touch. A suffix match, so
/// `api.github.com` and `objects.githubusercontent.com` are both covered.
const ALLOWED_SUFFIXES: &[&str] = &["github.com", "githubusercontent.com", "githubassets.com"];

/// How long a cached list is used before refetching. The upstream repo
/// publishes daily.
const CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Per-candidate fetch timeout. The file is a few KB; anything slower than this
/// is a mirror not worth waiting on when there are others to try.
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// Cache file name, kept beside `config.ini`.
const CACHE_FILE: &str = "github-hosts.txt";

/// A domain → IPs mapping parsed out of a hosts file.
pub type HostsMap = HashMap<String, Vec<IpAddr>>;

/// Whether `host` is one of GitHub's own domains.
fn is_github_domain(host: &str) -> bool {
    ALLOWED_SUFFIXES.iter().any(|suffix| {
        host == *suffix
            || (host.len() > suffix.len()
                && host.ends_with(suffix)
                && host.as_bytes()[host.len() - suffix.len() - 1] == b'.')
    })
}

/// Parse hosts-file syntax into a domain → IPs map, keeping only GitHub
/// domains.
///
/// Blank lines, `#` comments, and blackhole entries (`0.0.0.0`, loopback) are
/// dropped — the last of those would otherwise cut off the very domain we are
/// trying to reach.
pub fn parse(body: &str) -> HostsMap {
    let mut map: HostsMap = HashMap::new();

    for line in body.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(ip) = parts.next().and_then(|p| p.parse::<IpAddr>().ok()) else {
            continue;
        };
        if ip.is_unspecified() || ip.is_loopback() {
            continue;
        }

        for host in parts {
            let host = host.to_ascii_lowercase();
            if !is_github_domain(&host) {
                continue;
            }
            let ips = map.entry(host).or_default();
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }

    map
}

/// Build an HTTP client that resolves GitHub's domains through `map`.
///
/// Returns `None` for an empty map — there would be nothing to override, and a
/// second client is only worth having when it changes something.
pub fn build_client(map: &HostsMap) -> Option<reqwest::Client> {
    if map.is_empty() {
        return None;
    }

    let mut builder = reqwest::Client::builder();
    for (host, ips) in map {
        // Port 0 means "the conventional port for the scheme", so one override
        // covers both https and the plain-http redirects GitHub occasionally
        // hands out.
        let addrs: Vec<SocketAddr> = ips.iter().map(|ip| SocketAddr::new(*ip, 0)).collect();
        builder = builder.resolve_to_addrs(host, &addrs);
    }

    match builder.build() {
        Ok(client) => {
            tracing::info!("github-hosts: {} domains overridden", map.len());
            Some(client)
        }
        Err(e) => {
            tracing::warn!("github-hosts: failed to build client: {e}");
            None
        }
    }
}

/// Path of the cached list for a given config directory.
fn cache_path(config_dir: &Path) -> std::path::PathBuf {
    config_dir.join(CACHE_FILE)
}

/// Read the cached list, if it exists. `require_fresh` rejects a copy older
/// than [`CACHE_MAX_AGE`].
async fn read_cache(config_dir: &Path, require_fresh: bool) -> Option<String> {
    let path = cache_path(config_dir);
    if require_fresh {
        let age = tokio::fs::metadata(&path)
            .await
            .ok()?
            .modified()
            .ok()?
            .elapsed()
            .ok()?;
        if age > CACHE_MAX_AGE {
            return None;
        }
    }
    tokio::fs::read_to_string(&path).await.ok()
}

/// Fetch `url`, returning the body only if it looks like a hosts file.
async fn fetch(client: &reqwest::Client, url: &str) -> Option<String> {
    let response = client
        .get(url)
        .header("User-Agent", "MapleLink-Updater")
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}

/// Obtain the GitHub hosts list: a fresh cached copy if there is one, otherwise
/// the network (direct first, then each mirror), otherwise whatever stale copy
/// is on disk.
///
/// Always best-effort — an empty map just means the caller carries on with the
/// mirrors, exactly as before this existed.
pub async fn load(client: &reqwest::Client, config_dir: &Path, mirrors: &[&str]) -> HostsMap {
    if let Some(body) = read_cache(config_dir, true).await {
        let map = parse(&body);
        if !map.is_empty() {
            tracing::info!("github-hosts: using cached list ({} domains)", map.len());
            return map;
        }
    }

    // Direct first: it costs one quick timeout and is the only candidate that
    // isn't someone else's proxy. The mirrors are the fallback the list exists
    // to make unnecessary.
    let mut candidates = vec![HOSTS_URL.to_string()];
    candidates.extend(mirrors.iter().map(|m| format!("{m}{HOSTS_URL}")));

    for url in candidates {
        let Some(body) = fetch(client, &url).await else {
            tracing::debug!("github-hosts: no response from {url}");
            continue;
        };
        let map = parse(&body);
        if map.is_empty() {
            tracing::debug!("github-hosts: {url} returned nothing usable");
            continue;
        }
        tracing::info!("github-hosts: fetched {} domains from {url}", map.len());
        if let Err(e) = tokio::fs::write(cache_path(config_dir), &body).await {
            tracing::debug!("github-hosts: could not cache the list: {e}");
        }
        return map;
    }

    // Every route failed. A stale list is still a far better guess than the
    // poisoned DNS we are working around.
    if let Some(body) = read_cache(config_dir, false).await {
        let map = parse(&body);
        if !map.is_empty() {
            tracing::info!("github-hosts: falling back to a stale cached list");
            return map;
        }
    }

    tracing::info!("github-hosts: no list available");
    HostsMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Shaped like the real file: a header comment, tabs, trailing comments.
    const SAMPLE: &str = "\
# GitHub Hosts Start
140.82.112.3\tgithub.com
140.82.113.5    api.github.com
185.199.108.133 raw.githubusercontent.com objects.githubusercontent.com
185.199.109.133 raw.githubusercontent.com
# 140.82.112.4 commented.github.com
0.0.0.0 blocked.github.com
127.0.0.1 localhost
1.2.3.4 evil.example.com
2606:50c0:8000::154 github.io
# GitHub Hosts End
";

    #[test]
    fn parses_a_real_shaped_hosts_file() {
        let map = parse(SAMPLE);
        assert_eq!(
            map["github.com"],
            vec!["140.82.112.3".parse::<IpAddr>().unwrap()]
        );
        assert_eq!(
            map["api.github.com"],
            vec!["140.82.113.5".parse::<IpAddr>().unwrap()]
        );
        // Two lines, one host — both IPs kept, in file order.
        assert_eq!(
            map["raw.githubusercontent.com"],
            vec![
                "185.199.108.133".parse::<IpAddr>().unwrap(),
                "185.199.109.133".parse::<IpAddr>().unwrap()
            ]
        );
        // A second host on the same line is picked up too.
        assert!(map.contains_key("objects.githubusercontent.com"));
    }

    #[test]
    fn drops_comments_blackholes_and_foreign_domains() {
        let map = parse(SAMPLE);
        assert!(!map.contains_key("commented.github.com"));
        // 0.0.0.0 / loopback would blackhole the domain we're trying to reach.
        assert!(!map.contains_key("blocked.github.com"));
        assert!(!map.contains_key("localhost"));
        // The whole point of the allowlist: a tampered list cannot redirect
        // anything that isn't GitHub.
        assert!(!map.contains_key("evil.example.com"));
        // github.io is not one of the suffixes we serve traffic to.
        assert!(!map.contains_key("github.io"));
    }

    #[test]
    fn suffix_match_is_on_label_boundaries() {
        assert!(is_github_domain("github.com"));
        assert!(is_github_domain("api.github.com"));
        assert!(is_github_domain("objects.githubusercontent.com"));
        // `notgithub.com` ends with the suffix as a substring but is a
        // different domain — the boundary check is what rejects it.
        assert!(!is_github_domain("notgithub.com"));
        assert!(!is_github_domain("github.com.evil.net"));
    }

    #[test]
    fn empty_map_builds_no_client() {
        assert!(build_client(&HostsMap::new()).is_none());
    }

    proptest! {
        /// Whatever a mirror hands back, parsing must not panic and must never
        /// yield an entry outside GitHub's domains.
        #[test]
        fn prop_parse_never_escapes_the_allowlist(body in ".{0,400}") {
            let map = parse(&body);
            for host in map.keys() {
                prop_assert!(is_github_domain(host));
            }
        }
    }
}
