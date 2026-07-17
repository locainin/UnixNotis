//! Shared configuration path origin resolution for diagnostic commands

use std::path::PathBuf;

use unixnotis_core::util::CONFIG_PATH_ENV;
use unixnotis_core::Config;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigPathSource {
    Cli,
    Environment,
    Default,
    Builtin,
}

impl ConfigPathSource {
    pub const fn is_explicit(self) -> bool {
        matches!(self, Self::Cli | Self::Environment)
    }
}

pub fn resolve_config_path(
    requested_path: Option<PathBuf>,
) -> Result<(PathBuf, ConfigPathSource), unixnotis_core::ConfigError> {
    // CLI input outranks environment and default path discovery
    if let Some(path) = requested_path {
        return Ok((path, ConfigPathSource::Cli));
    }

    // Empty environment overrides retain the normal config location
    let source = if std::env::var_os(CONFIG_PATH_ENV).is_some_and(|value| !value.is_empty()) {
        ConfigPathSource::Environment
    } else {
        ConfigPathSource::Default
    };
    Config::active_config_path().map(|path| (path, source))
}

#[cfg(test)]
#[path = "tests/config_path.rs"]
mod tests;
