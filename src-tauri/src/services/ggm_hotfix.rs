//! Where the client-integrity values come from, newest source first.
//!
//! beanfun's TW credential endpoint asks the caller to state which build of the
//! Gamania Games Manager is asking: a version, and the SHA-256 of one of its
//! files. Both are constants for a given release of the manager, so MapleLink
//! ships a known-good pair — but the day beanfun requires a newer one, that
//! pair stops working for everyone at once.
//!
//! Rather than answer that with an emergency release, the values are looked up
//! in order:
//!
//! 1. a `GGMWebStart.dll`, or a `ggm-client.json` marked `"override": true`,
//!    placed in MapleLink's data folder — an explicit choice, so nothing
//!    overrides it;
//! 2. the game manager installed here, or a small file published alongside
//!    MapleLink and cached for six hours — whichever names the newer build;
//! 3. the pair compiled in, so a machine with none of the above still works.
//!
//! The second layer is a comparison rather than an order because both ways
//! round have a failure the other doesn't. The manager updates itself, but only
//! when it runs, and the people this app exists for never run it — so an
//! install can sit at whatever version it was last opened at, which is exactly
//! what the endpoint stops accepting, and preferring it would put the stalest
//! machines beyond the reach of a published fix. Preferring the published pair
//! instead would let one bad commit take down everyone whose own install was
//! fine. Comparing costs nothing and avoids both; a tie goes to the installed
//! file.

use std::time::Duration;

use crate::services::process_service::{self, ClientIntegrity};

/// Where the published values live, and the mirrors that serve the same file
/// where GitHub itself is unreachable.
const HOTFIX_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/lshw54/maplelink/main/ggm-client.json",
    "https://cdn.jsdelivr.net/gh/lshw54/maplelink@main/ggm-client.json",
    "https://fastly.jsdelivr.net/gh/lshw54/maplelink@main/ggm-client.json",
    "https://ghproxy.net/https://raw.githubusercontent.com/lshw54/maplelink/main/ggm-client.json",
];

/// File name of both the published copy and the local override — the same
/// name on purpose: whatever is fetched can be edited in place, and an edited
/// file is simply one the fetch won't overwrite while it stays fresh.
const CACHE_FILE: &str = "ggm-client.json";

/// A local file the user wrote themselves, rather than one we fetched.
///
/// Told apart by an `override` flag rather than by location, so editing the
/// fetched file in place is all it takes to pin values — no second path to
/// explain, and no way to "fix" it and have the next fetch quietly undo you.
fn local_override() -> Option<(String, String)> {
    let body = std::fs::read_to_string(cache_path()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    if value["override"].as_bool() != Some(true) {
        return None;
    }
    let pair = parse(&body)?;
    tracing::info!(cv = %pair.0, "ggm-hotfix: using the pinned local values");
    Some(pair)
}

/// How long a fetched copy is used before asking again. Long enough that a
/// launch never waits on the network twice in a session, short enough that a
/// fix published today reaches people today.
const CACHE_MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);

/// Per-source timeout. The file is a few dozen bytes; a source slower than this
/// is one the built-in values can outrun.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

fn cache_path() -> Option<std::path::PathBuf> {
    let dir = std::env::var("APPDATA").ok()?;
    Some(
        std::path::Path::new(&dir)
            .join("com.maplelink.app")
            .join(CACHE_FILE),
    )
}

/// Parse a published pair, rejecting anything that isn't shaped like one.
///
/// A malformed file must not be able to replace working values with rubbish:
/// the endpoint would reject it and the failure would look like beanfun's, not
/// ours.
fn parse(body: &str) -> Option<(String, String)> {
    // Editors add a byte-order mark unasked, and it makes the file unparseable
    // as JSON. Someone hand-editing this to fix their own launch shouldn't be
    // defeated by their text editor.
    let value: serde_json::Value =
        serde_json::from_str(body.trim_start_matches('\u{feff}')).ok()?;
    let cv = value["cv"].as_str()?.trim().to_string();
    let hash = value["hash"].as_str()?.trim().to_ascii_lowercase();

    let version_shaped =
        !cv.is_empty() && cv.len() <= 32 && cv.chars().all(|c| c.is_ascii_digit() || c == '.');
    let hash_shaped = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
    (version_shaped && hash_shaped).then_some((cv, hash))
}

/// Read the cached pair. `require_fresh` rejects one older than the max age.
fn read_cache(require_fresh: bool) -> Option<(String, String)> {
    let path = cache_path()?;
    if require_fresh {
        let age = std::fs::metadata(&path)
            .ok()?
            .modified()
            .ok()?
            .elapsed()
            .ok()?;
        if age > CACHE_MAX_AGE {
            return None;
        }
    }
    parse(&std::fs::read_to_string(&path).ok()?)
}

/// Fetch the published pair, trying each source in turn.
async fn fetch(client: &reqwest::Client) -> Option<(String, String)> {
    for url in HOTFIX_URLS {
        let Ok(response) = client.get(*url).timeout(FETCH_TIMEOUT).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        // The real file is under 200 bytes, and every source is a mirror of
        // one repository — anything substantial is answering something else.
        let Some(body) = crate::services::http_util::read_capped_text(response, 64 * 1024).await
        else {
            continue;
        };
        let Some(pair) = parse(&body) else {
            tracing::debug!("ggm-hotfix: {url} served something unusable");
            continue;
        };
        tracing::info!(cv = %pair.0, "ggm-hotfix: published values fetched");
        if let Some(path) = cache_path() {
            let _ = std::fs::write(path, &body);
        }
        return Some(pair);
    }
    None
}

/// The values to send, from the best source available.
pub async fn client_integrity(client: &reqwest::Client) -> ClientIntegrity {
    // Placed by hand, so nothing else may override it.
    if let Some(dropped) = process_service::dropped_client_integrity() {
        return dropped;
    }
    if let Some((cv, hash)) = local_override() {
        return ClientIntegrity {
            cv,
            hash,
            arch: process_service::arch(),
        };
    }

    let published = match read_cache(true) {
        Some(pair) => Some(pair),
        None => match fetch(client).await {
            Some(pair) => Some(pair),
            // Offline, or nothing published: a stale copy still beats values
            // frozen at build time.
            None => read_cache(false),
        },
    };

    // The installed manager and the published pair, whichever names the newer
    // build.
    //
    // The manager updates itself, but only when it runs, and the people this
    // app exists for are the ones who never run it: they launch from here, not
    // from the official site, and this path no longer starts it at all. An
    // install left untouched since before beanfun's last release reports what
    // it was then — exactly the values the endpoint stops accepting. Preferring
    // it outright would make the stalest machines the only ones a published fix
    // could never reach.
    //
    // Preferring the published pair outright trades that for the opposite
    // hazard: a bad publish taking down users whose own install was fine.
    // Comparing avoids both. A tie goes to the installed file, which is this
    // machine's own truth rather than a claim about it.
    let installed = process_service::installed_client_integrity();
    match (installed, published) {
        (Some(installed), Some((cv, hash))) => {
            if names_a_newer_build(&cv, &installed.cv) {
                tracing::info!(
                    installed = %installed.cv,
                    published = %cv,
                    "ggm: the published pair is newer than the installed manager"
                );
                ClientIntegrity {
                    cv,
                    hash,
                    arch: process_service::arch(),
                }
            } else {
                installed
            }
        }
        (Some(installed), None) => installed,
        (None, Some((cv, hash))) => ClientIntegrity {
            cv,
            hash,
            arch: process_service::arch(),
        },
        (None, None) => process_service::builtin_client_integrity(),
    }
}

/// Whether `candidate` names a strictly newer build than `current`.
///
/// Dotted numbers compared segment by segment, missing segments read as zero so
/// `1.5.1` beats `1.5`. Anything unparseable compares as zero, which makes a
/// malformed version lose rather than win — the safe direction, since the loser
/// is simply not used.
fn names_a_newer_build(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse().unwrap_or(0))
            .collect()
    }
    let (a, b) = (parts(candidate), parts(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_published_version_wins() {
        // The case that motivated comparing at all: an install left alone since
        // before beanfun's last release.
        assert!(names_a_newer_build("1.5.0.2", "1.4.9.9"));
        assert!(names_a_newer_build("1.5.1", "1.5.0.2"));
        assert!(names_a_newer_build("2.0", "1.9.9.9"));
    }

    #[test]
    fn an_equal_or_older_published_version_does_not() {
        // A tie goes to the installed file, and a publish naming an older build
        // is a mistake that must not take working machines down.
        assert!(!names_a_newer_build("1.5.0.2", "1.5.0.2"));
        assert!(
            !names_a_newer_build("1.5.0", "1.5.0.0"),
            "missing segments read as zero"
        );
        assert!(!names_a_newer_build("1.4.0", "1.5.0.2"));
    }

    #[test]
    fn an_unreadable_version_loses() {
        // Whatever cannot be parsed compares as zero. Losing is the safe
        // direction: the loser is simply not used.
        assert!(!names_a_newer_build("", "1.0"));
        assert!(!names_a_newer_build("not.a.version", "0.0.1"));
        assert!(names_a_newer_build("0.0.1", "not.a.version"));
    }

    #[test]
    fn accepts_a_well_formed_pair() {
        let body = r#"{"cv":"1.5.0.2","hash":"DFD568A69D87ABCD8F4A93D1A4481EBB57712D1D28AB0B6FC018FCF140101E06"}"#;
        let (cv, hash) = parse(body).expect("parses");
        assert_eq!(cv, "1.5.0.2");
        // Stored lowercase, which is the form the endpoint expects.
        assert!(hash.starts_with("dfd568a6") && hash.len() == 64);
    }

    #[test]
    fn a_byte_order_mark_does_not_defeat_it() {
        let body = concat!(
            "\u{feff}",
            r#"{"cv":"1.5.0.2","hash":"dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06"}"#
        );
        assert!(parse(body).is_some(), "a BOM must not hide the values");
    }

    #[test]
    fn a_pinned_file_is_told_apart_by_its_flag() {
        let pinned = r#"{"override":true,"cv":"9.9.9.9","hash":"dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06"}"#;
        let plain = r#"{"cv":"9.9.9.9","hash":"dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06"}"#;
        // Both parse as a pair; only the flag says "leave this alone".
        assert!(parse(pinned).is_some());
        assert!(parse(plain).is_some());
        let flagged: serde_json::Value = serde_json::from_str(pinned).unwrap();
        let unflagged: serde_json::Value = serde_json::from_str(plain).unwrap();
        assert_eq!(flagged["override"].as_bool(), Some(true));
        assert_ne!(unflagged["override"].as_bool(), Some(true));
    }

    #[test]
    fn rejects_anything_not_shaped_like_a_pair() {
        // A published file that is wrong would replace working values with
        // ones the endpoint rejects, and the failure would look like beanfun's.
        assert!(parse("not json").is_none());
        assert!(parse(r#"{"cv":"1.5.0.2"}"#).is_none(), "hash missing");
        assert!(
            parse(r#"{"cv":"","hash":"aa"}"#).is_none(),
            "hash too short"
        );
        assert!(
            parse(r#"{"cv":"1.5.0.2","hash":"zz68a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06gg"}"#).is_none(),
            "hash not hex"
        );
        assert!(
            parse(r#"{"cv":"1.5.0.2; rm -rf","hash":"dfd568a69d87abcd8f4a93d1a4481ebb57712d1d28ab0b6fc018fcf140101e06"}"#)
                .is_none(),
            "version must be digits and dots"
        );
    }
}
