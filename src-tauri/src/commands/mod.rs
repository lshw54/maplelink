//! Tauri command handlers — thin wrappers that deserialize args,
//! invoke core/service logic, and map errors to serializable DTOs.

pub mod account;
pub mod auth;
pub mod config;
pub mod launcher;
pub mod system;
pub mod update;
