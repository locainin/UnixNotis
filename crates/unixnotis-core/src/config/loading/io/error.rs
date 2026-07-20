//! Errors returned while loading and preparing configuration files

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFailed(String),
    #[error("failed to parse config: {0}")]
    ParseFailed(String),
    #[error("configuration file is too large ({size} bytes; maximum {max} bytes)")]
    TooLarge { size: u64, max: u64 },
    #[error("missing $HOME, unable to resolve config directory")]
    MissingHome,
}

impl ConfigError {
    /// Return a stable summary that never includes configuration contents
    #[must_use]
    pub const fn shareable_summary(&self) -> &'static str {
        match self {
            Self::ReadFailed(_) => "Configuration file could not be read",
            Self::ParseFailed(_) => "Configuration TOML or schema is invalid",
            Self::TooLarge { .. } => "Configuration file exceeds the maximum supported size",
            Self::MissingHome => "HOME is missing, so the configuration path cannot resolve",
        }
    }
}
