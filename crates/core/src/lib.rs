#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

pub mod config;
pub mod engine;
pub mod parser;
pub mod traits;
pub mod types;
