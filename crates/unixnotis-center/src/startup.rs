//! Process startup helpers for config, logging, and session checks

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;
use unixnotis_core::Config;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Args {
    /// Path to config.toml
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
}

pub(crate) fn load_config(args: &Args) -> Result<(Config, PathBuf)> {
    if let Some(path) = args.config.as_ref() {
        return Ok((
            Config::load_from_path(path).context("read config from path")?,
            path.clone(),
        ));
    }

    let path = Config::default_config_path().context("resolve default config path")?;
    let config = Config::load_default().context("read default config")?;
    Ok((config, path))
}

pub(crate) fn init_tracing(config: &Config) {
    // Prefer RUST_LOG when it is valid
    // Fall back to the config file when the env value is missing or broken
    let (filter, warning, env_warning) = match EnvFilter::try_from_default_env() {
        Ok(filter) => (filter, None, None),
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

            match EnvFilter::try_new(configured.clone()) {
                Ok(filter) => (filter, None, env_warning),
                Err(err) => {
                    // Bad directives should not stop startup
                    // Fall back to info so logging still works
                    let fallback =
                        EnvFilter::try_new("info").unwrap_or_else(|_| EnvFilter::new("info"));
                    let warning = format!(
                        "invalid log_level '{}'; defaulting to 'info' ({err})",
                        configured
                    );
                    (fallback, Some(warning), env_warning)
                }
            }
        }
    };

    // Install the subscriber once
    // Warnings logged after init go through the configured sink
    tracing_subscriber::fmt().with_env_filter(filter).init();

    if let Some(message) = env_warning {
        tracing::warn!("{message}");
    }
    if let Some(message) = warning {
        tracing::warn!("{message}");
    }
}

pub(crate) fn is_wayland_session() -> bool {
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        if session_type.eq_ignore_ascii_case("wayland") {
            return true;
        }
    }

    env::var("WAYLAND_DISPLAY").is_ok()
}
