//! Configuration loading and tracing setup.
//!
//! Keeps environment handling and logging setup out of the main control flow.

use std::env;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;

use super::Args;

pub(super) fn load_config(args: &Args) -> Result<Config> {
    match args.config.as_ref() {
        Some(path) => Config::load_from_path(path).context("read config from path"),
        None => Config::load_default().context("read default config"),
    }
}

pub(super) fn init_tracing(config: &Config) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            config
                .general
                .log_level
                .clone()
                .unwrap_or_else(|| "info".to_string()),
        )
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

pub(super) fn is_wayland_session() -> bool {
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        if session_type.eq_ignore_ascii_case("wayland") {
            return true;
        }
    }
    env::var("WAYLAND_DISPLAY").is_ok()
}
