#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

pub mod config;
pub mod db_path;
pub mod engine;
pub mod logging;
pub mod path_utils;
pub mod search;
pub mod text_utils;
pub mod traits;
pub mod types;

// Re-export parser for backward compatibility
pub use kajet_parser as parser;
