//! Game launch argument construction and path validation (pure core logic).
//!
//! This module builds the command-line invocation for launching the game
//! executable. It performs no file system access or process spawning — only
//! argument construction and syntactic validation.

use crate::core::error::FsError;
use crate::models::config::AppConfig;
use crate::models::game_account::GameCredentials;

/// The fully-resolved launch command ready for process spawning.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchCommand {
    /// Absolute path to the game executable.
    pub executable: String,
    /// Working directory (parent folder of the executable).
    pub working_dir: String,
    /// Command-line arguments passed to the executable.
    pub args: Vec<String>,
}

/// Validate that `path` is a syntactically valid Windows `.exe` path.
///
/// Checks:
/// - Non-empty
/// - Ends with `.exe` (case-insensitive)
/// - Contains no invalid Windows filename characters (`< > " | ? *` and
///   control characters U+0000–U+001F). Note that `:` is allowed for drive
///   letters and `\` / `/` are valid path separators.
pub fn validate_game_path(path: &str) -> Result<(), FsError> {
    if path.is_empty() {
        return Err(FsError::NotFound {
            path: String::new(),
        });
    }

    if !path.to_ascii_lowercase().ends_with(".exe") {
        return Err(FsError::NotFound {
            path: path.to_string(),
        });
    }

    // Invalid characters for Windows file paths (excluding : \ / which are
    // valid as drive-letter separator and path separators respectively).
    let invalid_chars = ['<', '>', '"', '|', '?', '*'];
    for ch in path.chars() {
        if ch.is_control() || invalid_chars.contains(&ch) {
            return Err(FsError::NotFound {
                path: path.to_string(),
            });
        }
    }

    Ok(())
}

/// Build the full launch command from application config and game credentials.
///
/// The returned [`LaunchCommand`] contains:
/// - `executable` — the game path from config
/// - `working_dir` — the parent directory of the executable
/// - `args` — command-line arguments including the OTP
pub fn build_launch_command(
    config: &AppConfig,
    credentials: &GameCredentials,
) -> Result<LaunchCommand, FsError> {
    validate_game_path(&config.game_path)?;

    let working_dir = extract_parent_dir(&config.game_path);

    Ok(LaunchCommand {
        executable: config.game_path.clone(),
        working_dir,
        args: vec![credentials.otp.clone()],
    })
}

/// Extract the parent directory from a Windows-style path.
///
/// Handles both `\` and `/` separators. Falls back to `.` if no separator is
/// found.
fn extract_parent_dir(path: &str) -> String {
    // Find the last path separator (either \ or /)
    let last_sep = path.rfind(['\\', '/']);
    match last_sep {
        Some(idx) => path[..idx].to_string(),
        None => ".".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::{Language, Theme};
    use crate::models::session::Region;
    use chrono::Utc;
    use proptest::prelude::*;

    // --- validate_game_path ---

    #[test]
    fn valid_exe_path_accepted() {
        assert!(validate_game_path(r"C:\Games\MapleStory\MapleStory.exe").is_ok());
    }

    #[test]
    fn valid_exe_path_case_insensitive() {
        assert!(validate_game_path(r"D:\Game\test.EXE").is_ok());
        assert!(validate_game_path(r"D:\Game\test.Exe").is_ok());
    }

    #[test]
    fn empty_path_rejected() {
        assert!(validate_game_path("").is_err());
    }

    #[test]
    fn non_exe_extension_rejected() {
        assert!(validate_game_path(r"C:\Games\game.txt").is_err());
        assert!(validate_game_path(r"C:\Games\game").is_err());
    }

    #[test]
    fn invalid_chars_rejected() {
        assert!(validate_game_path(r"C:\Games\game<1>.exe").is_err());
        assert!(validate_game_path("C:\\Games\\game\".exe").is_err());
        assert!(validate_game_path("C:\\Games\\game\x01.exe").is_err());
    }

    #[test]
    fn forward_slash_path_accepted() {
        assert!(validate_game_path("C:/Games/MapleStory.exe").is_ok());
    }

    // --- build_launch_command ---

    fn make_config(game_path: &str) -> AppConfig {
        AppConfig {
            game_path: game_path.to_string(),
            ..AppConfig::default()
        }
    }

    fn make_credentials(otp: &str) -> GameCredentials {
        GameCredentials {
            account_id: "acc123".to_string(),
            otp: otp.to_string(),
            retrieved_at: Utc::now(),
            command_line_template: None,
        }
    }

    #[test]
    fn build_launch_command_basic() {
        let config = make_config(r"C:\Nexon\MapleStory\MapleStory.exe");
        let creds = make_credentials("1234567890");

        let cmd = build_launch_command(&config, &creds).unwrap();

        assert_eq!(cmd.executable, r"C:\Nexon\MapleStory\MapleStory.exe");
        assert_eq!(cmd.working_dir, r"C:\Nexon\MapleStory");
        assert_eq!(cmd.args, vec!["1234567890"]);
    }

    #[test]
    fn build_launch_command_forward_slashes() {
        let config = make_config("C:/Games/MapleStory.exe");
        let creds = make_credentials("0000000000");

        let cmd = build_launch_command(&config, &creds).unwrap();

        assert_eq!(cmd.working_dir, "C:/Games");
    }

    #[test]
    fn build_launch_command_invalid_path_errors() {
        let config = make_config("not_an_exe");
        let creds = make_credentials("1234567890");

        assert!(build_launch_command(&config, &creds).is_err());
    }

    #[test]
    fn extract_parent_dir_no_separator() {
        assert_eq!(extract_parent_dir("game.exe"), ".");
    }

    // --- Property-based tests ---

    /// Generate a valid Windows `.exe` game path like `C:\<dir>\<name>.exe`.
    fn arb_game_path() -> impl Strategy<Value = String> {
        (
            "[A-Z]",                                                // drive letter
            proptest::collection::vec("[A-Za-z0-9_]{1,20}", 1..=3), // path segments
            "[A-Za-z0-9_]{1,15}", // executable name (no extension)
        )
            .prop_map(|(drive, segments, name)| {
                format!("{}:\\{}\\{}.exe", drive, segments.join("\\"), name)
            })
    }

    fn arb_theme() -> impl Strategy<Value = Theme> {
        prop_oneof![Just(Theme::System), Just(Theme::Dark), Just(Theme::Light)]
    }

    fn arb_language() -> impl Strategy<Value = Language> {
        prop_oneof![
            Just(Language::EnUS),
            Just(Language::ZhTW),
            Just(Language::ZhCN)
        ]
    }

    fn arb_region() -> impl Strategy<Value = Region> {
        prop_oneof![Just(Region::TW), Just(Region::HK)]
    }

    /// Generate a valid `AppConfig` with a syntactically valid `.exe` game path.
    fn arb_valid_app_config() -> impl Strategy<Value = AppConfig> {
        (
            arb_game_path(),
            arb_theme(),
            arb_language(),
            any::<bool>(),
            any::<bool>(),
            arb_region(),
            any::<bool>(),
        )
            .prop_map(
                |(
                    game_path,
                    theme,
                    language,
                    auto_update,
                    skip_play_confirm,
                    region,
                    debug_logging,
                )| {
                    AppConfig {
                        game_path,
                        locale: "zh-TW".into(),
                        theme,
                        language,
                        auto_update,
                        update_channel: crate::models::config::UpdateChannel::Release,
                        skip_play_confirm,
                        auto_start: false,
                        window_x: None,
                        window_y: None,
                        window_width: None,
                        window_height: None,
                        region,
                        debug_logging,
                        gamepass_incognito: true,
                        font_size: crate::models::config::FontSize::Medium,
                        traditional_login: true,
                        auto_kill_patcher: true,
                        account_view_mode: crate::models::config::AccountViewMode::Card,
                    }
                },
            )
    }

    /// Generate valid `GameCredentials` with a random OTP string.
    fn arb_game_credentials() -> impl Strategy<Value = GameCredentials> {
        (
            "[a-zA-Z0-9]{4,16}", // account_id
            "[0-9]{6,12}",       // otp
        )
            .prop_map(|(account_id, otp)| GameCredentials {
                account_id,
                otp,
                retrieved_at: Utc::now(),
                command_line_template: None,
            })
    }

    // Feature: maplelink-rewrite, Property 8: Game launch argument construction
    //
    // For any valid AppConfig (with a non-empty game_path) and any valid
    // GameCredentials, the constructed launch command shall include the game
    // executable path as the target, the config's game directory as the working
    // directory, and the credential OTP as a command-line argument.
    //
    // **Validates: Requirements 4.1, 4.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_game_launch_argument_construction(
            config in arb_valid_app_config(),
            creds in arb_game_credentials(),
        ) {
            let cmd = build_launch_command(&config, &creds)
                .expect("build_launch_command should succeed for valid inputs");

            // Executable path equals the config game_path
            prop_assert_eq!(
                &cmd.executable, &config.game_path,
                "executable must equal config.game_path"
            );

            // Working directory is the parent directory of the game path
            let expected_working_dir = extract_parent_dir(&config.game_path);
            prop_assert_eq!(
                &cmd.working_dir, &expected_working_dir,
                "working_dir must be the parent directory of game_path"
            );

            // Args contain the OTP from credentials
            prop_assert!(
                cmd.args.contains(&creds.otp),
                "args must contain the OTP, got args={:?}, expected otp={}",
                cmd.args,
                creds.otp
            );
        }
    }

    // Feature: maplelink-rewrite, Property 9: Game path validation
    //
    // For any string input as a game executable path, the path validator shall
    // accept only strings that end with `.exe` and represent a syntactically
    // valid Windows file path, rejecting empty strings, paths with invalid
    // characters, and non-`.exe` extensions.
    //
    // **Validates: Requirements 4.6**

    /// Generate a random string that does NOT end with `.exe` (case-insensitive).
    fn arb_non_exe_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9_./\\\\: ]{1,50}".prop_filter("must not end with .exe", |s| {
            !s.to_ascii_lowercase().ends_with(".exe")
        })
    }

    /// Generate a path that contains at least one invalid Windows char but ends
    /// with `.exe`.
    fn arb_path_with_invalid_char() -> impl Strategy<Value = String> {
        let invalid_chars = prop_oneof![
            Just('<'),
            Just('>'),
            Just('"'),
            Just('|'),
            Just('?'),
            Just('*'),
            (0u8..=31u8).prop_map(|b| b as char),
        ];

        ("[A-Za-z0-9_]{0,10}", invalid_chars, "[A-Za-z0-9_]{0,10}")
            .prop_map(|(prefix, bad_char, suffix)| format!("{}{}{}.exe", prefix, bad_char, suffix))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// Valid `.exe` paths generated by `arb_game_path()` are always accepted.
        #[test]
        fn prop_valid_exe_paths_accepted(path in arb_game_path()) {
            prop_assert!(
                validate_game_path(&path).is_ok(),
                "valid .exe path should be accepted: {:?}",
                path
            );
        }

        /// Non-`.exe` strings are always rejected.
        #[test]
        fn prop_non_exe_extensions_rejected(path in arb_non_exe_string()) {
            prop_assert!(
                validate_game_path(&path).is_err(),
                "non-.exe path should be rejected: {:?}",
                path
            );
        }

        /// Paths containing invalid Windows characters are always rejected,
        /// even if they end with `.exe`.
        #[test]
        fn prop_invalid_chars_rejected(path in arb_path_with_invalid_char()) {
            prop_assert!(
                validate_game_path(&path).is_err(),
                "path with invalid chars should be rejected: {:?}",
                path
            );
        }
    }

    /// Empty strings are always rejected by `validate_game_path`.
    #[test]
    fn prop_empty_string_rejected() {
        assert!(
            validate_game_path("").is_err(),
            "empty string should always be rejected"
        );
    }
}
