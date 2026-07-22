//! External interaction layer — file I/O, HTTP, process management.

pub mod account_storage;
pub mod announcement_service;
pub mod autopaste_service;
pub mod beanfun_service;
pub mod cafe_service;
pub mod config_service;
pub mod cookie_native;
pub mod data_transfer;
pub mod exe_rename_service;
pub mod game_download;
pub mod game_env_service;
pub mod game_launch_service;
pub mod log_service;
pub mod lr_service;
pub mod network_service;
pub mod process_service;
pub mod recaptcha_window;
pub mod session_key_fallback;
pub mod update_service;
pub mod web_launch;
pub mod web_popup_service;
pub mod webview_login;
pub mod webview_util;
