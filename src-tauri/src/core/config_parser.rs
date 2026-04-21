//! Pure INI config parser and serializer — no I/O, no side effects.
//!
//! Handles four INI sections: `[general]`, `[game]`, `[appearance]`, `[window]`.
//! Malformed entries fall back to [`AppConfig::default()`] values with logged warnings.

use crate::core::error::ConfigError;
use crate::models::config::{AccountViewMode, AppConfig, FontSize, Language, Theme, UpdateChannel};
use crate::models::session::Region;
use std::collections::HashMap;

/// Parse an INI-style string into an [`AppConfig`].
///
/// Malformed or unrecognised entries are silently skipped (with a `tracing::warn`),
/// and the corresponding field keeps its [`Default`] value.
pub fn parse_ini(input: &str) -> Result<AppConfig, ConfigError> {
    let defaults = AppConfig::default();
    let mut config = defaults.clone();
    let mut current_section = String::new();

    // Collect key-value pairs grouped by section.
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();

    for (line_no, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();

        // Skip empty lines and comments.
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Section header.
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_lowercase();
            continue;
        }

        // Key = value pair.
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_lowercase();
            let value = value.trim().to_string();

            if current_section.is_empty() {
                tracing::warn!(
                    line = line_no + 1,
                    "key outside any section, skipping: {key}"
                );
                continue;
            }

            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key, value);
        } else {
            tracing::warn!(line = line_no + 1, "malformed line, skipping: {raw_line}");
        }
    }

    // --- [general] ---
    if let Some(general) = sections.get("general") {
        if let Some(v) = general.get("region") {
            config.region = parse_region(v);
        }
        if let Some(v) = general.get("language") {
            config.language = parse_language(v);
        }
        if let Some(v) = general.get("auto_update") {
            config.auto_update = parse_bool(v, "auto_update", defaults.auto_update);
        }
        if let Some(v) = general.get("auto_start") {
            config.auto_start = parse_bool(v, "auto_start", defaults.auto_start);
        }
        if let Some(v) = general.get("debug_logging") {
            config.debug_logging = parse_bool(v, "debug_logging", defaults.debug_logging);
        }
        if let Some(v) = general.get("update_channel") {
            config.update_channel = parse_update_channel(v);
        }
        if let Some(v) = general.get("gamepass_incognito") {
            config.gamepass_incognito =
                parse_bool(v, "gamepass_incognito", defaults.gamepass_incognito);
        }
        if let Some(v) = general.get("traditional_login") {
            config.traditional_login =
                parse_bool(v, "traditional_login", defaults.traditional_login);
        }
        if let Some(v) = general.get("auto_kill_patcher") {
            config.auto_kill_patcher =
                parse_bool(v, "auto_kill_patcher", defaults.auto_kill_patcher);
        }
    }

    // --- [game] ---
    if let Some(game) = sections.get("game") {
        if let Some(v) = game.get("path") {
            config.game_path = v.clone();
        }
        if let Some(v) = game.get("locale") {
            config.locale = v.clone();
        }
        if let Some(v) = game.get("skip_play_confirm") {
            config.skip_play_confirm =
                parse_bool(v, "skip_play_confirm", defaults.skip_play_confirm);
        }
    }

    // --- [appearance] ---
    if let Some(appearance) = sections.get("appearance") {
        if let Some(v) = appearance.get("theme") {
            config.theme = parse_theme(v);
        }
        if let Some(v) = appearance.get("font_size") {
            config.font_size = parse_font_size(v);
        }
        if let Some(v) = appearance.get("account_view_mode") {
            config.account_view_mode = parse_account_view_mode(v);
        }
    }

    // --- [window] ---
    if let Some(window) = sections.get("window") {
        if let Some(v) = window.get("x") {
            config.window_x = parse_optional_i32(v, "x");
        }
        if let Some(v) = window.get("y") {
            config.window_y = parse_optional_i32(v, "y");
        }
        if let Some(v) = window.get("width") {
            config.window_width = parse_optional_u32(v, "width");
        }
        if let Some(v) = window.get("height") {
            config.window_height = parse_optional_u32(v, "height");
        }
    }

    Ok(config)
}

/// Serialize an [`AppConfig`] into a pretty-printed INI string.
pub fn serialize_ini(config: &AppConfig) -> String {
    let mut out = String::new();

    // [general]
    out.push_str("[general]\n");
    out.push_str(&format!("region = {}\n", region_to_str(&config.region)));
    out.push_str(&format!(
        "language = {}\n",
        language_to_str(&config.language)
    ));
    out.push_str(&format!("auto_update = {}\n", config.auto_update));
    out.push_str(&format!("auto_start = {}\n", config.auto_start));
    out.push_str(&format!("debug_logging = {}\n", config.debug_logging));
    out.push_str(&format!(
        "update_channel = {}\n",
        update_channel_to_str(&config.update_channel)
    ));
    out.push_str(&format!(
        "gamepass_incognito = {}\n",
        config.gamepass_incognito
    ));
    out.push_str(&format!(
        "traditional_login = {}\n",
        config.traditional_login
    ));
    out.push_str(&format!(
        "auto_kill_patcher = {}\n",
        config.auto_kill_patcher
    ));
    out.push('\n');

    // [game]
    out.push_str("[game]\n");
    out.push_str(&format!("path = {}\n", config.game_path));
    out.push_str(&format!("locale = {}\n", config.locale));
    out.push_str(&format!(
        "skip_play_confirm = {}\n",
        config.skip_play_confirm
    ));
    out.push('\n');

    // [appearance]
    out.push_str("[appearance]\n");
    out.push_str(&format!("theme = {}\n", theme_to_str(&config.theme)));
    out.push_str(&format!(
        "font_size = {}\n",
        font_size_to_str(&config.font_size)
    ));
    out.push_str(&format!(
        "account_view_mode = {}\n",
        account_view_mode_to_str(&config.account_view_mode)
    ));
    out.push('\n');

    // [window]
    out.push_str("[window]\n");
    if let Some(x) = config.window_x {
        out.push_str(&format!("x = {x}\n"));
    }
    if let Some(y) = config.window_y {
        out.push_str(&format!("y = {y}\n"));
    }
    if let Some(w) = config.window_width {
        out.push_str(&format!("width = {w}\n"));
    }
    if let Some(h) = config.window_height {
        out.push_str(&format!("height = {h}\n"));
    }

    out
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_bool(value: &str, field: &str, default: bool) -> bool {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" => true,
        "false" | "0" | "no" => false,
        _ => {
            tracing::warn!("invalid bool for '{field}': '{value}', using default");
            default
        }
    }
}

fn parse_region(value: &str) -> Region {
    match value.to_uppercase().as_str() {
        "TW" => Region::TW,
        "HK" => Region::HK,
        _ => {
            tracing::warn!("unknown region '{value}', falling back to default");
            AppConfig::default().region
        }
    }
}

fn parse_language(value: &str) -> Language {
    match value {
        "en-US" => Language::EnUS,
        "zh-TW" => Language::ZhTW,
        "zh-CN" => Language::ZhCN,
        _ => {
            tracing::warn!("unknown language '{value}', falling back to default");
            AppConfig::default().language
        }
    }
}

fn parse_theme(value: &str) -> Theme {
    match value.to_lowercase().as_str() {
        "system" => Theme::System,
        "dark" => Theme::Dark,
        "light" => Theme::Light,
        _ => {
            tracing::warn!("unknown theme '{value}', falling back to default");
            AppConfig::default().theme
        }
    }
}

fn parse_update_channel(value: &str) -> UpdateChannel {
    match value.to_lowercase().replace('_', "-").as_str() {
        "release" => UpdateChannel::Release,
        "pre-release" | "prerelease" => UpdateChannel::PreRelease,
        _ => {
            tracing::warn!("unknown update_channel '{value}', falling back to default");
            AppConfig::default().update_channel
        }
    }
}

fn parse_font_size(value: &str) -> FontSize {
    match value.to_lowercase().as_str() {
        "small" => FontSize::Small,
        "medium" => FontSize::Medium,
        "large" => FontSize::Large,
        "extra-large" => FontSize::ExtraLarge,
        _ => {
            tracing::warn!("unknown font_size '{value}', falling back to default");
            FontSize::Medium
        }
    }
}

fn parse_optional_i32(value: &str, field: &str) -> Option<i32> {
    match value.parse::<i32>() {
        Ok(n) => Some(n),
        Err(_) => {
            tracing::warn!("invalid i32 for '{field}': '{value}', skipping");
            None
        }
    }
}

fn parse_optional_u32(value: &str, field: &str) -> Option<u32> {
    match value.parse::<u32>() {
        Ok(n) => Some(n),
        Err(_) => {
            tracing::warn!("invalid u32 for '{field}': '{value}', skipping");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn region_to_str(region: &Region) -> &'static str {
    match region {
        Region::TW => "TW",
        Region::HK => "HK",
    }
}

fn language_to_str(lang: &Language) -> &'static str {
    match lang {
        Language::EnUS => "en-US",
        Language::ZhTW => "zh-TW",
        Language::ZhCN => "zh-CN",
    }
}

fn theme_to_str(theme: &Theme) -> &'static str {
    match theme {
        Theme::System => "system",
        Theme::Dark => "dark",
        Theme::Light => "light",
    }
}

fn update_channel_to_str(channel: &UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Release => "release",
        UpdateChannel::PreRelease => "pre-release",
    }
}

fn font_size_to_str(size: &FontSize) -> &'static str {
    match size {
        FontSize::Small => "small",
        FontSize::Medium => "medium",
        FontSize::Large => "large",
        FontSize::ExtraLarge => "extra-large",
    }
}

fn parse_account_view_mode(value: &str) -> AccountViewMode {
    match value.to_lowercase().as_str() {
        "list" => AccountViewMode::List,
        "card" => AccountViewMode::Card,
        _ => AccountViewMode::Card,
    }
}

fn account_view_mode_to_str(mode: &AccountViewMode) -> &'static str {
    match mode {
        AccountViewMode::Card => "card",
        AccountViewMode::List => "list",
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_valid_ini() {
        let ini = "\
[general]
region = HK
language = zh-TW
auto_update = true
auto_start = false
debug_logging = false

[game]
path = C:\\Nexon\\MapleStory\\MapleStory.exe
locale = zh-TW
skip_play_confirm = false

[appearance]
theme = dark

[window]
x = 100
y = 200
width = 600
height = 480
";
        let config = parse_ini(ini).unwrap();
        assert_eq!(config.region, Region::HK);
        assert_eq!(config.language, Language::ZhTW);
        assert!(config.auto_update);
        assert!(!config.auto_start);
        assert!(!config.debug_logging);
        assert_eq!(config.game_path, "C:\\Nexon\\MapleStory\\MapleStory.exe");
        assert_eq!(config.locale, "zh-TW");
        assert!(!config.skip_play_confirm);
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.window_x, Some(100));
        assert_eq!(config.window_y, Some(200));
        assert_eq!(config.window_width, Some(600));
        assert_eq!(config.window_height, Some(480));
    }

    #[test]
    fn parse_empty_string_returns_defaults() {
        let config = parse_ini("").unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn malformed_entries_use_defaults() {
        let ini = "\
[general]
region = INVALID
language = klingon
auto_update = maybe

[window]
x = not_a_number
";
        let config = parse_ini(ini).unwrap();
        let defaults = AppConfig::default();
        assert_eq!(config.region, defaults.region);
        assert_eq!(config.language, defaults.language);
        // malformed bool falls back to the field's default (auto_update defaults to true)
        assert_eq!(config.auto_update, defaults.auto_update);
        assert_eq!(config.window_x, None);
    }

    #[test]
    fn serialize_then_parse_round_trip() {
        let original = AppConfig {
            game_path: "D:\\Games\\Maple.exe".into(),
            locale: "en-US".into(),
            theme: Theme::Light,
            language: Language::EnUS,
            auto_update: false,
            update_channel: UpdateChannel::PreRelease,
            skip_play_confirm: true,
            auto_start: true,
            window_x: Some(-50),
            window_y: Some(300),
            window_width: Some(1024),
            window_height: Some(768),
            region: Region::TW,
            debug_logging: true,
            gamepass_incognito: false,
            font_size: crate::models::config::FontSize::Large,
            traditional_login: false,
            auto_kill_patcher: false,
            account_view_mode: crate::models::config::AccountViewMode::List,
        };
        let ini = serialize_ini(&original);
        let parsed = parse_ini(&ini).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn serialize_omits_none_window_fields() {
        let config = AppConfig::default();
        let ini = serialize_ini(&config);
        assert!(!ini.contains("x ="));
        assert!(!ini.contains("y ="));
        assert!(!ini.contains("width ="));
        assert!(!ini.contains("height ="));
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let ini = "\
# This is a comment
; Another comment

[general]
region = TW
";
        let config = parse_ini(ini).unwrap();
        assert_eq!(config.region, Region::TW);
    }
}
