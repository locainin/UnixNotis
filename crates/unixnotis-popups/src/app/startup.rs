//! Startup helpers for config loading and process-wide setup

use std::env;
use std::path::PathBuf;

use anyhow::Context;
use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;

use super::Args;

pub(super) fn load_config(args: &Args) -> anyhow::Result<(Config, PathBuf)> {
    if let Some(path) = args.config.as_ref() {
        // Explicit config paths should bypass the default lookup completely
        return Ok((
            Config::load_from_path(path).context("read config from path")?,
            path.clone(),
        ));
    }
    // Default-path startup keeps existing popup behavior unchanged for normal runs
    let path = Config::default_config_path().context("resolve default config path")?;
    let config = Config::load_default().context("read default config")?;
    Ok((config, path))
}

pub(super) fn init_tracing(config: &Config) {
    let (filter, env_warning) = match EnvFilter::try_from_default_env() {
        Ok(filter) => (filter, None),
        Err(err) => {
            let env_warning = if env::var("RUST_LOG").is_ok() {
                Some(format!(
                    "invalid RUST_LOG value: {err}; falling back to config log_level"
                ))
            } else {
                None
            };
            let configured = config
                .general
                .log_level
                .clone()
                .unwrap_or_else(|| "info".to_string());
            let filter = EnvFilter::try_new(configured.as_str()).unwrap_or_else(|err| {
                // Bad config should fall back to a safe default instead of crashing
                eprintln!(
                    "unixnotis-popups: invalid log level '{}': {err}; falling back to info",
                    configured
                );
                EnvFilter::new("info")
            });
            (filter, env_warning)
        }
    };
    if let Err(err) = tracing_subscriber::fmt().with_env_filter(filter).try_init() {
        eprintln!("unixnotis-popups: tracing initialization skipped: {err}");
    }
    if let Some(message) = env_warning {
        tracing::warn!("{message}");
    }
}

pub(super) fn is_wayland_session() -> bool {
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        if session_type.eq_ignore_ascii_case("wayland") {
            return true;
        }
    }
    env::var("WAYLAND_DISPLAY").is_ok()
}
