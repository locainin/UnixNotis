//! Process startup helpers for config, logging, and session checks

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;
use unixnotis_core::{util::CONFIG_PATH_ENV, Config};

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Args {
    /// Path to config.toml
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigSource {
    // Explicit CLI or daemon-provided path
    Custom,
    // Config loaded from the default on-disk location
    Default,
    // Builtin fallback used because no config file exists yet
    Builtin,
}

pub(crate) fn load_config(args: &Args) -> Result<(Config, PathBuf, ConfigSource)> {
    if let Some(path) = config_override_path(args, env::var_os(CONFIG_PATH_ENV)) {
        return Ok((
            Config::load_from_path(&path).context("read config from path")?,
            path,
            ConfigSource::Custom,
        ));
    }

    let path = Config::default_config_path().context("resolve default config path")?;
    let config = Config::load_default().context("read default config")?;
    let source = if path.exists() {
        ConfigSource::Default
    } else {
        ConfigSource::Builtin
    };
    Ok((config, path, source))
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

fn config_override_path(args: &Args, env_path: Option<OsString>) -> Option<PathBuf> {
    if let Some(path) = args.config.as_ref() {
        // CLI flags still win so direct launches stay easy to reason about
        return Some(path.clone());
    }

    let path = env_path?;
    if path.is_empty() {
        return None;
    }

    // Daemon-spawned child apps read the exact config file path from this env value
    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{config_override_path, Args};

    #[test]
    fn config_override_prefers_cli_arg() {
        let args = Args {
            config: Some(PathBuf::from("/tmp/cli.toml")),
        };
        assert_eq!(
            config_override_path(&args, Some(OsString::from("/tmp/env.toml"))),
            Some(PathBuf::from("/tmp/cli.toml"))
        );
    }

    #[test]
    fn config_override_accepts_env_path() {
        let args = Args { config: None };
        assert_eq!(
            config_override_path(&args, Some(OsString::from("/tmp/env.toml"))),
            Some(PathBuf::from("/tmp/env.toml"))
        );
    }

    #[test]
    fn config_override_ignores_empty_env_path() {
        let args = Args { config: None };
        assert_eq!(config_override_path(&args, Some(OsString::new())), None);
    }
}
