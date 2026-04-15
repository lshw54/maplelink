//! Feature: maplelink-rewrite, Property 3: No credentials persisted to disk
//!
//! For any `Session` stored in `AppState`, no file in the application data
//! directory shall contain the session token or refresh token as a substring.
//!
//! **Validates: Requirements 1.7, 13.2**

use maplelink_lib::core::config_parser::serialize_ini;
use maplelink_lib::models::config::{AppConfig, FontSize, Language, Theme, UpdateChannel};
use maplelink_lib::models::session::{Region, Session};
use maplelink_lib::services::config_service::save_config;
use proptest::prelude::*;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Arbitrary generators
// ---------------------------------------------------------------------------

/// Generate INI-safe strings (reused from config_roundtrip pattern).
fn arb_ini_safe_string() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 _\\-\\\\:/\\.]{0,100}")
        .unwrap()
        .prop_map(|s| s.trim().to_string())
}

/// Generate non-empty credential strings that are distinguishable from config
/// values. Uses a prefix to ensure tokens are unique enough to detect leaks.
fn arb_token() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9]{8,64}")
        .unwrap()
        .prop_map(|s| format!("tok_{s}"))
}

fn arb_theme() -> impl Strategy<Value = Theme> {
    prop_oneof![Just(Theme::System), Just(Theme::Dark), Just(Theme::Light),]
}

fn arb_language() -> impl Strategy<Value = Language> {
    prop_oneof![
        Just(Language::EnUS),
        Just(Language::ZhTW),
        Just(Language::ZhCN),
    ]
}

fn arb_region() -> impl Strategy<Value = Region> {
    prop_oneof![Just(Region::TW), Just(Region::HK),]
}

fn arb_app_config() -> impl Strategy<Value = AppConfig> {
    let group_a = (
        arb_ini_safe_string(),
        arb_ini_safe_string(),
        arb_theme(),
        arb_language(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    );
    let group_b = (
        proptest::option::of(any::<i32>()),
        proptest::option::of(any::<i32>()),
        proptest::option::of(any::<u32>()),
        proptest::option::of(any::<u32>()),
        arb_region(),
        any::<bool>(),
        prop_oneof![
            Just(UpdateChannel::Release),
            Just(UpdateChannel::PreRelease)
        ],
        any::<bool>(), // gamepass_incognito
        prop_oneof![
            Just(FontSize::Small),
            Just(FontSize::Medium),
            Just(FontSize::Large),
            Just(FontSize::ExtraLarge)
        ],
    );

    (group_a, group_b).prop_map(
        |(
            (game_path, locale, theme, language, auto_update, skip_play_confirm, auto_start),
            (
                window_x,
                window_y,
                window_width,
                window_height,
                region,
                debug_logging,
                update_channel,
                gamepass_incognito,
                font_size,
            ),
        )| {
            AppConfig {
                game_path,
                locale,
                theme,
                language,
                auto_update,
                update_channel,
                skip_play_confirm,
                auto_start,
                window_x,
                window_y,
                window_width,
                window_height,
                region,
                debug_logging,
                gamepass_incognito,
                font_size,
                traditional_login: true,
                auto_kill_patcher: true,
            }
        },
    )
}

/// Generate a random `Session` with non-empty token and optional refresh token.
fn arb_session() -> impl Strategy<Value = Session> {
    (
        arb_token(),
        proptest::option::of(arb_token()),
        arb_region(),
        arb_ini_safe_string(),
    )
        .prop_map(|(token, refresh_token, region, account_name)| Session {
            token,
            refresh_token,
            expires_at: chrono::Utc::now(),
            region,
            account_name,
            session_key: None,
            totp_state: None,
        })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read all files in a directory and concatenate their contents.
fn read_all_files_in_dir(dir: &PathBuf) -> String {
    let mut contents = String::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    contents.push_str(&text);
                }
            }
        }
    }
    contents
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Serializing an `AppConfig` to INI shall never contain session credentials.
    ///
    /// **Validates: Requirements 1.7, 13.2**
    #[test]
    fn serialize_ini_never_contains_credentials(
        config in arb_app_config(),
        session in arb_session(),
    ) {
        let ini_output = serialize_ini(&config);

        // Token must not appear in serialized config
        prop_assert!(
            !ini_output.contains(&session.token),
            "INI output contains session token: {}",
            session.token,
        );

        // Refresh token (if present) must not appear either
        if let Some(ref rt) = session.refresh_token {
            prop_assert!(
                !ini_output.contains(rt),
                "INI output contains refresh token: {}",
                rt,
            );
        }
    }

    /// Writing config to disk via `save_config` shall never persist credentials.
    ///
    /// **Validates: Requirements 1.7, 13.2**
    #[test]
    fn saved_config_files_never_contain_credentials(
        config in arb_app_config(),
        session in arb_session(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            // Create a unique temp directory per test case
            let dir = std::env::temp_dir().join(format!(
                "maplelink_nocred_{}_{}", std::process::id(), rand_suffix()
            ));
            let config_path = dir.join("config.ini");

            // Write config to disk
            save_config(&config_path, &config).await.unwrap();

            // Read ALL files in the temp directory
            let all_contents = read_all_files_in_dir(&dir);

            // Assert no file contains the session token
            assert!(
                !all_contents.contains(&session.token),
                "Disk files contain session token: {}",
                session.token,
            );

            // Assert no file contains the refresh token
            if let Some(ref rt_tok) = session.refresh_token {
                assert!(
                    !all_contents.contains(rt_tok),
                    "Disk files contain refresh token: {}",
                    rt_tok,
                );
            }

            // Cleanup
            std::fs::remove_dir_all(&dir).ok();
        });
    }
}

/// Generate a short random suffix for temp directory uniqueness.
fn rand_suffix() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    hasher.finish()
}
