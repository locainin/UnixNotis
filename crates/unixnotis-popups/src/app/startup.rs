//! Startup helpers for config loading and process-wide setup

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Context;
use tracing_subscriber::EnvFilter;
use unixnotis_core::{util::CONFIG_PATH_ENV, Config};

use super::Args;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfigSource {
    // Explicit CLI or daemon-provided path
    Custom,
    // Config loaded from the default on-disk location
    Default,
    // Builtin fallback used because no config file exists yet
    Builtin,
}

pub(super) fn load_config(args: &Args) -> anyhow::Result<(Config, PathBuf, ConfigSource)> {
    if let Some(path) = config_override_path(args, env::var_os(CONFIG_PATH_ENV)) {
        // Explicit config paths should bypass the default lookup completely
        return Ok((
            Config::load_from_path(&path).context("read config from path")?,
            path,
            ConfigSource::Custom,
        ));
    }
    // Default-path startup keeps existing popup behavior unchanged for normal runs
    let path = Config::default_config_path().context("resolve default config path")?;
    let config = Config::load_default().context("read default config")?;
    let source = if path.exists() {
        ConfigSource::Default
    } else {
        ConfigSource::Builtin
    };
    Ok((config, path, source))
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

fn config_override_path(args: &Args, env_path: Option<OsString>) -> Option<PathBuf> {
    if let Some(path) = args.config.as_ref() {
        // CLI flags still win so direct popup launches stay consistent
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
mod startup_tests {
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
