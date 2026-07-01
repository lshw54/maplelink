//! Parsing of the beanfun *web* game-launch command line.
//!
//! Some users can only log in through beanfun's website (e.g. the launcher's
//! reCAPTCHA won't load behind their accelerator). For them we register
//! `HKCU\SOFTWARE\Gamania\MapleStory\PATH` to point at MapleLink; beanfun then
//! launches MapleLink (instead of the game) with the SAME command line it
//! would have passed MapleStory:
//!
//! ```text
//!   <server> <port> BeanFun <account> <otp>
//! ```
//!
//! i.e. account = param 4, otp = param 5 — identical to MapleLink's own launch
//! template (`"... BeanFun %s %s"`, see `commands::launcher`) and to the
//! community `.bat` helper that reads `%4`/`%5`. This module extracts them.

/// The account + OTP beanfun handed us for a web-initiated game launch, plus
/// the raw params to forward verbatim to the real game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterceptCreds {
    pub account: String,
    /// One-time game-launch password (single-use, short-lived) — NOT the
    /// account's real password.
    pub otp: String,
    /// The params (excluding our own exe path) to pass straight to the game.
    pub raw_args: Vec<String>,
}

/// The marker beanfun places immediately before the `<account> <otp>` pair.
const BEANFUN_MARKER: &str = "BeanFun";

/// Detect and parse a beanfun web game-launch invocation.
///
/// `params` are the process arguments WITHOUT the executable path
/// (`std::env::args().skip(1)`). Returns `Some` only when the `BeanFun` marker
/// is present followed by a non-empty account + otp pair — so a normal
/// MapleLink launch (no such args) is never mistaken for an interception.
pub fn parse_intercept_args(params: &[String]) -> Option<InterceptCreds> {
    let marker = params
        .iter()
        .position(|a| a.eq_ignore_ascii_case(BEANFUN_MARKER))?;
    let account = params.get(marker + 1)?.trim().to_string();
    let otp = params.get(marker + 2)?.trim().to_string();
    if account.is_empty() || otp.is_empty() {
        return None;
    }
    Some(InterceptCreds {
        account,
        otp,
        raw_args: params.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_standard_beanfun_launch() {
        let params = v(&[
            "tw.login.maplestory.beanfun.com",
            "8484",
            "BeanFun",
            "myaccount",
            "otp123456",
        ]);
        let creds = parse_intercept_args(&params).expect("should parse");
        assert_eq!(creds.account, "myaccount");
        assert_eq!(creds.otp, "otp123456");
        assert_eq!(creds.raw_args, params);
    }

    #[test]
    fn marker_is_case_insensitive() {
        let params = v(&["s", "1", "beanfun", "acc", "otp"]);
        let creds = parse_intercept_args(&params).expect("should parse");
        assert_eq!(creds.account, "acc");
        assert_eq!(creds.otp, "otp");
    }

    #[test]
    fn normal_launch_without_marker_is_ignored() {
        assert!(parse_intercept_args(&v(&[])).is_none());
        assert!(parse_intercept_args(&v(&["--some-flag"])).is_none());
        assert!(parse_intercept_args(&v(&["a", "b", "c"])).is_none());
    }

    #[test]
    fn marker_without_full_pair_is_ignored() {
        assert!(parse_intercept_args(&v(&["s", "1", "BeanFun"])).is_none());
        assert!(parse_intercept_args(&v(&["s", "1", "BeanFun", "acc"])).is_none());
        assert!(parse_intercept_args(&v(&["s", "1", "BeanFun", "", "otp"])).is_none());
        assert!(parse_intercept_args(&v(&["s", "1", "BeanFun", "acc", "  "])).is_none());
    }
}
