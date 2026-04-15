//! Feature: maplelink-rewrite, Property 1: Configuration round-trip
//!
//! For any valid `AppConfig` instance, serializing it to INI format via
//! `serialize_ini` and then parsing it back via `parse_ini` shall produce
//! an `AppConfig` that is equal to the original.
//!
//! **Validates: Requirements 5.1, 5.6, 5.7**

use maplelink_lib::core::config_parser::{parse_ini, serialize_ini};
use maplelink_lib::models::config::{AppConfig, FontSize, Language, Theme, UpdateChannel};
use maplelink_lib::models::session::Region;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Arbitrary generators
// ---------------------------------------------------------------------------

/// Generate INI-safe strings: no newlines, no `=`, no `[`, `]`, `#`, `;`.
/// Also avoid leading/trailing whitespace that would be trimmed by the parser.
fn arb_ini_safe_string() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Za-z0-9 _\\-\\\\:/\\.]{0,100}")
        .unwrap()
        .prop_map(|s| s.trim().to_string())
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
    // Split into two groups to stay within proptest's 12-element tuple limit.
    let group_a = (
        arb_ini_safe_string(), // game_path
        arb_ini_safe_string(), // locale
        arb_theme(),
        arb_language(),
        any::<bool>(), // auto_update
        any::<bool>(), // skip_play_confirm
        any::<bool>(), // auto_start
    );
    let group_b = (
        proptest::option::of(any::<i32>()), // window_x
        proptest::option::of(any::<i32>()), // window_y
        proptest::option::of(any::<u32>()), // window_width
        proptest::option::of(any::<u32>()), // window_height
        arb_region(),
        any::<bool>(), // debug_logging
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

// ---------------------------------------------------------------------------
// Property test
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// **Validates: Requirements 5.1, 5.6, 5.7**
    #[test]
    fn config_round_trip(config in arb_app_config()) {
        let serialized = serialize_ini(&config);
        let parsed = parse_ini(&serialized).unwrap();
        prop_assert_eq!(parsed, config);
    }
}
