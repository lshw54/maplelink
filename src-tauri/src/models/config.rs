//! Application configuration models.

use serde::{Deserialize, Serialize};

use super::session::Region;

/// Full application configuration persisted as INI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub game_path: String,
    pub locale: String,
    pub theme: Theme,
    pub language: Language,
    pub auto_update: bool,
    pub update_channel: UpdateChannel,
    pub skip_play_confirm: bool,
    pub auto_start: bool,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub region: Region,
    pub debug_logging: bool,
    #[serde(default = "default_true")]
    pub gamepass_incognito: bool,
    #[serde(default = "default_font_size")]
    pub font_size: FontSize,
    /// Traditional login mode (default: true).
    /// When true, only GUID + game path are passed to LRProc.
    /// When false, also passes server/port/account/otp args.
    #[serde(default = "default_true")]
    pub traditional_login: bool,
    /// Auto-kill Patcher.exe when launching the game (default: true).
    #[serde(default = "default_true")]
    pub auto_kill_patcher: bool,
}

fn default_true() -> bool {
    true
}

fn default_font_size() -> FontSize {
    FontSize::Medium
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            game_path: String::new(),
            locale: "zh-TW".into(),
            theme: Theme::System,
            language: Language::ZhTW,
            auto_update: true,
            update_channel: UpdateChannel::Release,
            skip_play_confirm: true,
            auto_start: false,
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
            region: Region::HK,
            debug_logging: false,
            gamepass_incognito: true,
            font_size: FontSize::Medium,
            traditional_login: true,
            auto_kill_patcher: true,
        }
    }
}

/// UI theme selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Dark,
    Light,
}

/// Supported UI languages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Language {
    #[serde(rename = "en-US")]
    EnUS,
    #[serde(rename = "zh-TW")]
    ZhTW,
    #[serde(rename = "zh-CN")]
    ZhCN,
}

/// Update channel preference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateChannel {
    Release,
    PreRelease,
}

/// UI font size preference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FontSize {
    Small,
    Medium,
    Large,
    ExtraLarge,
}
