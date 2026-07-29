//! Atomic persistence for the explicit theme source

use std::path::Path;

use thiserror::Error;
use toml_edit::{value, DocumentMut, Item, Table};

use crate::filesystem::{read_regular_file_bounded, write_file_atomic_preserving_mode};
use crate::{Config, ThemeMode};

use super::MAX_CONFIG_BYTES;

#[derive(Debug, Error)]
pub enum ThemeModeWriteError {
    #[error("read config: {0}")]
    Read(std::io::Error),
    #[error("config is not valid UTF-8")]
    Encoding,
    #[error("config is invalid: {0}")]
    InvalidConfig(String),
    #[error("config document is invalid: {0}")]
    InvalidDocument(toml_edit::TomlError),
    #[error("theme section is not a table")]
    InvalidThemeSection,
    #[error("write config: {0}")]
    Write(std::io::Error),
}

impl ThemeModeWriteError {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Read(_) => "read",
            Self::Encoding => "encoding",
            Self::InvalidConfig(_) => "invalid-config",
            Self::InvalidDocument(_) => "invalid-document",
            Self::InvalidThemeSection => "invalid-theme-section",
            Self::Write(_) => "write",
        }
    }
}

/// Persist the theme source while retaining unrelated TOML formatting
///
/// # Errors
///
/// Returns an error when the existing config is unsafe, invalid, or cannot be replaced atomically
pub fn persist_theme_mode(path: &Path, mode: ThemeMode) -> Result<(), ThemeModeWriteError> {
    let bytes =
        read_regular_file_bounded(path, MAX_CONFIG_BYTES).map_err(ThemeModeWriteError::Read)?;
    let contents = std::str::from_utf8(&bytes).map_err(|_error| ThemeModeWriteError::Encoding)?;
    Config::parse_with_report(contents)
        .map_err(|error| ThemeModeWriteError::InvalidConfig(error.to_string()))?;
    let mut document = contents
        .parse::<DocumentMut>()
        .map_err(ThemeModeWriteError::InvalidDocument)?;
    if !document.as_table().contains_key("theme") {
        document["theme"] = Item::Table(Table::new());
    }
    let Some(theme) = document["theme"].as_table_mut() else {
        return Err(ThemeModeWriteError::InvalidThemeSection);
    };
    theme["mode"] = value(mode.as_str());
    write_file_atomic_preserving_mode(path, document.to_string().as_bytes(), 0o600)
        .map_err(ThemeModeWriteError::Write)
}
