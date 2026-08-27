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
//! Three rules keep a third-party list from being a foothold:
//! - only GitHub's own domains are overridable, so a tampered list cannot point
//!   `beanfun.com` (or anything else the app talks to) at an attacker;
//! - addresses that arrive over DNS rather than from the curated list must sit
//!   inside GitHub's own address blocks, which is also what makes a poisoned
//!   answer detectable;
//! - certificates are validated, so an override says which address to dial and
//!   never who may answer: a wrong or hostile IP fails the handshake and we fall
//!   back to the mirrors rather than downloading an exe from it.
//!
//! Where the list comes from matters as much as what's in it: the users who
//! need it are exactly the ones who cannot fetch it from GitHub. So several
//! independent networks are asked at once — the ghproxy mirrors, CDN copies of
//! the same file, and DoH resolvers, which is the trick upstream uses to build
//! the list in the first place.
//!
//! Nothing is stored between runs. Upstream republishes daily, so a kept copy
//! would only ever be a stale answer standing in the way of a current one; if
//! no source answers, the mirrors take over exactly as they did before this
//! existed.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

/// The daily-updated hosts list.
const HOSTS_URL: &str = "https://raw.githubusercontent.com/maxiaof/github-hosts/master/hosts";

/// Places the same list is published that are not GitHub itself. CDN edges are
/// generally reachable from where GitHub isn't, and none of them is the ghproxy
/// mirrors this whole feature exists to stop depending on.
const LIST_MIRRORS: &[&str] = &[
    "https://cdn.jsdelivr.net/gh/maxiaof/github-hosts@master/hosts",
    "https://fastly.jsdelivr.net/gh/maxiaof/github-hosts@master/hosts",
    "https://gcore.jsdelivr.net/gh/maxiaof/github-hosts@master/hosts",
    "https://cdn.statically.io/gh/maxiaof/github-hosts/master/hosts",
];

/// DNS-over-HTTPS endpoints, JSON flavour, tried in order per host.
///
/// This is how the upstream list is built in the first place — it asks
/// Cloudflare over DoH. Doing it here as well means a working answer even when
/// every copy of the file is out of reach. Cloudflare goes first because its
/// answers are the trustworthy ones; the two Chinese resolvers follow because
/// Cloudflare's own endpoint is frequently unreachable from where this matters,
/// and a domestic resolver that returns a poisoned answer is caught by
/// [`is_github_ip`].
const DOH_ENDPOINTS: &[&str] = &[
    "https://cloudflare-dns.com/dns-query",
    "https://dns.alidns.com/resolve",
    "https://doh.pub/dns-query",
];

/// The handful of domains the update path actually travels through.
const DOH_HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "raw.githubusercontent.com",
    "codeload.github.com",
];

/// GitHub's own IPv4 blocks, as (network, prefix length).
///
/// Only applied to DNS answers, never to the curated list. A poisoned reply is
/// typically an unrelated or unroutable address, and dialling one costs a full
/// connect timeout — so an answer that isn't GitHub's is dropped rather than
/// queued up behind the good ones.
const GITHUB_NETS: &[(Ipv4Addr, u32)] = &[
    (Ipv4Addr::new(140, 82, 112, 0), 20),
    (Ipv4Addr::new(185, 199, 108, 0), 22),
    (Ipv4Addr::new(192, 30, 252, 0), 22),
    (Ipv4Addr::new(143, 55, 64, 0), 20),
];

/// Per-DoH-query timeout. Several endpoints are tried per host, so a blocked
/// one must give up quickly.
const DOH_TIMEOUT: Duration = Duration::from_secs(4);

/// Domains this override is allowed to touch. A suffix match, so
/// `api.github.com` and `objects.githubusercontent.com` are both covered.
const ALLOWED_SUFFIXES: &[&str] = &["github.com", "githubusercontent.com", "githubassets.com"];

/// Per-source fetch timeout. The file is a few KB, and every source is asked at
/// once, so this is the whole download step's ceiling rather than one step of
/// many.
const FETCH_TIMEOUT: Duration = Duration::from_secs(6);

/// How much of a hosts list is worth reading. The real one is a few hundred KB;
/// this leaves it room to grow several times over and still stops a mirror that
/// answers with something else.
const MAX_LIST_BYTES: u64 = 4 * 1024 * 1024;

/// How much of a DoH answer is worth reading. A handful of A records is a few
/// hundred bytes; two of the three resolvers are domestic ones this feature
/// exists precisely because it cannot take on trust.
const MAX_DOH_BYTES: u64 = 64 * 1024;

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

/// Whether `ip` falls inside one of GitHub's own address blocks.
fn is_github_ip(ip: IpAddr) -> bool {
    let IpAddr::V4(v4) = ip else {
        return false;
    };
    let bits = u32::from(v4);
    GITHUB_NETS.iter().any(|(net, len)| {
        let mask = u32::MAX << (32 - len);
        bits & mask == u32::from(*net) & mask
    })
}

/// Fold `from` into `into`, keeping the order addresses were discovered in.
/// Earlier sources stay in front, so the connector reaches for them first.
fn merge(into: &mut HostsMap, from: HostsMap) {
    for (host, ips) in from {
        let slot = into.entry(host).or_default();
        for ip in ips {
            if !slot.contains(&ip) {
                slot.push(ip);
            }
        }
    }
}

/// Ask one DoH endpoint for `host`'s A records, keeping only GitHub addresses.
async fn doh_lookup(client: &reqwest::Client, endpoint: &str, host: &str) -> Vec<IpAddr> {
    let url = format!("{endpoint}?name={host}&type=A");
    let Ok(response) = client
        .get(&url)
        .header("Accept", "application/dns-json")
        .timeout(DOH_TIMEOUT)
        .send()
        .await
    else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Some(body) = crate::services::http_util::read_capped_text(response, MAX_DOH_BYTES).await
    else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) else {
        return Vec::new();
    };

    json["Answer"]
        .as_array()
        .map(|answers| {
            answers
                .iter()
                // Type 1 is an A record; CNAME rows in the same answer set are
                // not addresses.
                .filter(|entry| entry["type"].as_u64() == Some(1))
                .filter_map(|entry| entry["data"].as_str()?.parse::<IpAddr>().ok())
                .filter(|ip| is_github_ip(*ip))
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve the update path's domains over DoH, all of them at once.
async fn resolve_over_doh(client: &reqwest::Client) -> HostsMap {
    let resolved = futures_util::future::join_all(DOH_HOSTS.iter().map(|host| async move {
        for endpoint in DOH_ENDPOINTS {
            let ips = doh_lookup(client, endpoint, host).await;
            if !ips.is_empty() {
                return ((*host).to_string(), ips);
            }
        }
        ((*host).to_string(), Vec::new())
    }))
    .await;

    let map: HostsMap = resolved
        .into_iter()
        .filter(|(_, ips)| !ips.is_empty())
        .collect();
    if !map.is_empty() {
        tracing::info!("github-hosts: DoH resolved {} domains", map.len());
    }
    map
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
    // Every source here is replaceable, so one answering with something
    // enormous is dropped rather than parsed.
    crate::services::http_util::read_capped_text(response, MAX_LIST_BYTES).await
}

/// Download the list.
///
/// Every source is asked at once and the first one to answer *in preference
/// order* wins: direct, then the CDN copies, then the ghproxy mirrors — direct
/// because it is nobody's proxy, the mirrors last because needing them is the
/// situation this feature exists to leave behind. Asking in sequence would
/// stack one timeout on top of another; the file is a couple of KB, so asking
/// everyone costs nothing but a few requests.
async fn fetch_list(client: &reqwest::Client, mirrors: &[&str]) -> HostsMap {
    let mut candidates = vec![HOSTS_URL.to_string()];
    candidates.extend(LIST_MIRRORS.iter().map(|m| (*m).to_string()));
    candidates.extend(mirrors.iter().map(|m| format!("{m}{HOSTS_URL}")));

    let answers = futures_util::future::join_all(
        candidates
            .iter()
            .map(|url| async move { (url, fetch(client, url).await) }),
    )
    .await;

    for (url, body) in answers {
        let Some(body) = body else {
            tracing::debug!("github-hosts: no response from {url}");
            continue;
        };
        let map = parse(&body);
        if map.is_empty() {
            tracing::debug!("github-hosts: {url} returned nothing usable");
            continue;
        }
        tracing::info!("github-hosts: fetched {} domains from {url}", map.len());
        return map;
    }

    tracing::info!("github-hosts: no source served the list");
    HostsMap::new()
}

/// Obtain GitHub's addresses, fresh, from every source worth asking.
///
/// Nothing is kept between runs. Upstream republishes daily and the addresses
/// are the whole point, so a stored copy would only ever be a stale answer
/// standing in the way of a current one — and the download is a couple of KB
/// on a path that already waited out a failed connection.
///
/// The list download and the DoH queries run together, so the wait is the
/// slower of the two rather than the sum, and both results are merged: a source
/// that knows only some of the domains still contributes what it has.
pub async fn load(client: &reqwest::Client, mirrors: &[&str]) -> HostsMap {
    let (fetched, resolved) =
        futures_util::future::join(fetch_list(client, mirrors), resolve_over_doh(client)).await;

    let mut map = fetched;
    merge(&mut map, resolved);
    map
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

    #[test]
    fn dns_answers_must_be_github_addresses() {
        // Real GitHub blocks.
        assert!(is_github_ip("140.82.116.5".parse().unwrap()));
        assert!(is_github_ip("185.199.111.133".parse().unwrap()));
        assert!(is_github_ip("192.30.255.1".parse().unwrap()));
        // The shapes a poisoned answer actually takes.
        assert!(!is_github_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_github_ip("31.13.64.35".parse().unwrap()));
        // Just outside 140.82.112.0/20 and 185.199.108.0/22.
        assert!(!is_github_ip("140.82.128.1".parse().unwrap()));
        assert!(!is_github_ip("185.199.112.1".parse().unwrap()));
    }

    #[test]
    fn merge_keeps_the_first_source_in_front() {
        let mut map = parse("185.199.109.133 raw.githubusercontent.com");
        merge(&mut map, parse("1.2.3.4 evil.example.com"));
        merge(
            &mut map,
            parse("185.199.108.133 raw.githubusercontent.com\n140.82.116.5 api.github.com"),
        );
        // The fresher address stays first; the older one is kept as a fallback
        // the connector only reaches if the first doesn't answer.
        assert_eq!(
            map["raw.githubusercontent.com"],
            vec![
                "185.199.109.133".parse::<IpAddr>().unwrap(),
                "185.199.108.133".parse::<IpAddr>().unwrap()
            ]
        );
        // A host only the later source knew about is added.
        assert!(map.contains_key("api.github.com"));
        // Merging cannot smuggle in what parsing would have rejected.
        assert!(!map.contains_key("evil.example.com"));
    }

    #[test]
    fn merge_does_not_duplicate_a_repeated_address() {
        let mut map = parse("140.82.116.5 api.github.com");
        merge(&mut map, parse("140.82.116.5 api.github.com"));
        assert_eq!(map["api.github.com"].len(), 1);
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
