//! Feature: maplelink-rewrite, Property 2: Malformed config entries fall back to defaults
//!
//! For any INI string that contains a mix of valid and malformed entries,
//! parsing it via `parse_ini` shall produce a valid `AppConfig` where
//! correctly-formed fields retain their parsed values and malformed fields
//! fall back to their `Default` values.
//!
//! **Validates: Requirements 5.4**

use maplelink_lib::core::config_parser::parse_ini;
use maplelink_lib::models::config::{AppConfig, Language, Theme};
use maplelink_lib::models::session::Region;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers — valid value generators
// ---------------------------------------------------------------------------

fn arb_valid_region() -> impl Strategy<Value = String> {
    prop_oneof![Just("TW".to_string()), Just("HK".to_string()),]
}

fn arb_valid_language() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("en-US".to_string()),
        Just("zh-TW".to_string()),
        Just("zh-CN".to_string()),
    ]
}

fn arb_valid_theme() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("system".to_string()),
        Just("dark".to_string()),
        Just("light".to_string()),
    ]
}

fn arb_valid_bool() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("true".to_string()),
        Just("false".to_string()),
        Just("1".to_string()),
        Just("0".to_string()),
        Just("yes".to_string()),
        Just("no".to_string()),
    ]
}

fn arb_valid_i32() -> impl Strategy<Value = String> {
    any::<i32>().prop_map(|n| n.to_string())
}

/// Garbage string that will NOT match any valid enum/bool/number value.
/// Avoids accidentally generating "true", "false", "1", "0", "yes", "no",
/// "TW", "HK", "dark", "light", "system", "en-US", "zh-TW", "zh-CN",
/// or valid integers.
fn arb_garbage() -> impl Strategy<Value = String> {
    // Use a prefix that guarantees the string is never a valid value.
    "[A-Z]{3,8}".prop_map(|s| format!("GARBAGE_{s}"))
}

// ---------------------------------------------------------------------------
// Strategy: for each field, either produce a valid value or garbage.
// We return (value_string, is_corrupted) pairs.
// ---------------------------------------------------------------------------

fn arb_maybe_corrupt_region() -> impl Strategy<Value = (String, bool)> {
    prop_oneof![
        arb_valid_region().prop_map(|v| (v, false)),
        arb_garbage().prop_map(|g| (g, true)),
    ]
}

fn arb_maybe_corrupt_language() -> impl Strategy<Value = (String, bool)> {
    prop_oneof![
        arb_valid_language().prop_map(|v| (v, false)),
        arb_valid_language().prop_map(|v| (v, false)),
        arb_garbage().prop_map(|g| (g, true)),
    ]
}

fn arb_maybe_corrupt_theme() -> impl Strategy<Value = (String, bool)> {
    prop_oneof![
        arb_valid_theme().prop_map(|v| (v, false)),
        arb_garbage().prop_map(|g| (g, true)),
    ]
}

fn arb_maybe_corrupt_bool() -> impl Strategy<Value = (String, bool)> {
    prop_oneof![
        arb_valid_bool().prop_map(|v| (v, false)),
        arb_garbage().prop_map(|g| (g, true)),
    ]
}

fn arb_maybe_corrupt_i32() -> impl Strategy<Value = (String, bool)> {
    prop_oneof![
        arb_valid_i32().prop_map(|v| (v, false)),
        arb_garbage().prop_map(|g| (g, true)),
    ]
}

// ---------------------------------------------------------------------------
// Property test
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// **Validates: Requirements 5.4**
    #[test]
    fn malformed_config_falls_back_to_defaults(
        (region_val, region_corrupt) in arb_maybe_corrupt_region(),
        (language_val, language_corrupt) in arb_maybe_corrupt_language(),
        (auto_update_val, auto_update_corrupt) in arb_maybe_corrupt_bool(),
        (auto_start_val, auto_start_corrupt) in arb_maybe_corrupt_bool(),
        (debug_logging_val, debug_logging_corrupt) in arb_maybe_corrupt_bool(),
        (skip_play_confirm_val, skip_play_confirm_corrupt) in arb_maybe_corrupt_bool(),
        (theme_val, theme_corrupt) in arb_maybe_corrupt_theme(),
        (window_x_val, window_x_corrupt) in arb_maybe_corrupt_i32(),
        (window_y_val, window_y_corrupt) in arb_maybe_corrupt_i32(),
    ) {
        let defaults = AppConfig::default();

        // Build an INI string with all sections and fields present.
        let ini = format!(
            "\
[general]
region = {region_val}
language = {language_val}
auto_update = {auto_update_val}
auto_start = {auto_start_val}
debug_logging = {debug_logging_val}

[game]
path = C:\\Games\\Test.exe
locale = zh-TW
skip_play_confirm = {skip_play_confirm_val}

[appearance]
theme = {theme_val}

[window]
x = {window_x_val}
y = {window_y_val}
"
        );

        // parse_ini must always succeed — it never returns Err.
        let config = parse_ini(&ini).expect("parse_ini should always return Ok");

        // --- Assert corrupted fields fall back to defaults ---

        if region_corrupt {
            prop_assert_eq!(config.region.clone(), defaults.region,
                "corrupted region should fall back to default");
        }

        if language_corrupt {
            prop_assert_eq!(config.language.clone(), defaults.language,
                "corrupted language should fall back to default");
        }

        if auto_update_corrupt {
            prop_assert_eq!(config.auto_update, defaults.auto_update,
                "corrupted auto_update should fall back to default");
        }

        if auto_start_corrupt {
            prop_assert_eq!(config.auto_start, defaults.auto_start,
                "corrupted auto_start should fall back to default");
        }

        if debug_logging_corrupt {
            prop_assert_eq!(config.debug_logging, defaults.debug_logging,
                "corrupted debug_logging should fall back to default");
        }

        if skip_play_confirm_corrupt {
            prop_assert_eq!(config.skip_play_confirm, defaults.skip_play_confirm,
                "corrupted skip_play_confirm should fall back to default");
        }

        if theme_corrupt {
            prop_assert_eq!(config.theme.clone(), defaults.theme,
                "corrupted theme should fall back to default");
        }

        // Window fields: corrupted → None (the default for optional fields).
        if window_x_corrupt {
            prop_assert_eq!(config.window_x, None,
                "corrupted window_x should fall back to None");
        }

        if window_y_corrupt {
            prop_assert_eq!(config.window_y, None,
                "corrupted window_y should fall back to None");
        }

        // --- Assert valid fields retain their parsed values ---

        if !region_corrupt {
            let expected = match region_val.as_str() {
                "TW" => Region::TW,
                "HK" => Region::HK,
                _ => unreachable!(),
            };
            prop_assert_eq!(config.region.clone(), expected,
                "valid region should be parsed correctly");
        }

        if !language_corrupt {
            let expected = match language_val.as_str() {
                "en-US" => Language::EnUS,
                "zh-TW" => Language::ZhTW,
                "zh-CN" => Language::ZhCN,
                _ => unreachable!(),
            };
            prop_assert_eq!(config.language.clone(), expected,
                "valid language should be parsed correctly");
        }

        if !theme_corrupt {
            let expected = match theme_val.as_str() {
                "system" => Theme::System,
                "dark" => Theme::Dark,
                "light" => Theme::Light,
                _ => unreachable!(),
            };
            prop_assert_eq!(config.theme.clone(), expected,
                "valid theme should be parsed correctly");
        }

        // Non-corrupt bools: parse the expected value.
        fn expected_bool(val: &str) -> bool {
            matches!(val.to_lowercase().as_str(), "true" | "1" | "yes")
        }

        if !auto_update_corrupt {
            prop_assert_eq!(config.auto_update, expected_bool(&auto_update_val));
        }
        if !auto_start_corrupt {
            prop_assert_eq!(config.auto_start, expected_bool(&auto_start_val));
        }
        if !debug_logging_corrupt {
            prop_assert_eq!(config.debug_logging, expected_bool(&debug_logging_val));
        }
        if !skip_play_confirm_corrupt {
            prop_assert_eq!(config.skip_play_confirm, expected_bool(&skip_play_confirm_val));
        }

        if !window_x_corrupt {
            prop_assert_eq!(config.window_x, Some(window_x_val.parse::<i32>().unwrap()));
        }
        if !window_y_corrupt {
            prop_assert_eq!(config.window_y, Some(window_y_val.parse::<i32>().unwrap()));
        }

        // game_path and locale are always set to valid values in this test,
        // so they should always parse correctly.
        prop_assert_eq!(config.game_path, "C:\\Games\\Test.exe");
        prop_assert_eq!(config.locale, "zh-TW");
    }
}
