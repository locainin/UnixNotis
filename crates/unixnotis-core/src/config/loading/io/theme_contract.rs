//! Read-only custom theme compatibility contract

use std::io::ErrorKind;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::filesystem::read_regular_file_bounded;

use super::ThemePaths;

/// Theme contract understood by this release
pub const THEME_API_VERSION: u32 = 2;

const THEME_MANIFEST_FILE: &str = "theme.toml";
const MAX_THEME_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_THEME_NAME_CHARS: usize = 128;

/// Manifest required before user-controlled CSS can be loaded
#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeManifest {
    pub api_version: u32,
    pub name: String,
}

/// Reason an existing custom theme could not be enabled safely
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ThemeIncompatibility {
    MissingManifest,
    UnreadableManifest,
    InvalidManifest,
    UnsupportedVersion { found: u32 },
    InvalidName,
}

/// Active source selected without changing user files
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ThemeContractState {
    EmbeddedStock,
    Compatible(ThemeManifest),
    Incompatible(ThemeIncompatibility),
}

impl ThemeContractState {
    /// Return whether configured CSS may be loaded
    #[must_use]
    pub const fn custom_theme_allowed(&self) -> bool {
        matches!(self, Self::Compatible(_))
    }

    /// Return whether the panel should explain the stock fallback
    #[must_use]
    pub const fn is_incompatible(&self) -> bool {
        matches!(self, Self::Incompatible(_))
    }
}

impl ThemePaths {
    /// Return the manifest anchored beside the active configuration
    #[must_use]
    pub fn manifest_path(&self) -> PathBuf {
        self.base_dir.join(THEME_MANIFEST_FILE)
    }

    /// Select custom or embedded CSS without creating or changing files
    #[must_use]
    pub fn inspect_theme_contract(&self) -> ThemeContractState {
        let manifest_path = self.manifest_path();
        let contents = match read_regular_file_bounded(&manifest_path, MAX_THEME_MANIFEST_BYTES) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return ThemeContractState::Incompatible(ThemeIncompatibility::MissingManifest);
            }
            Err(_error) => {
                return ThemeContractState::Incompatible(ThemeIncompatibility::UnreadableManifest);
            }
        };
        let Ok(contents) = std::str::from_utf8(&contents) else {
            return ThemeContractState::Incompatible(ThemeIncompatibility::InvalidManifest);
        };
        let Ok(mut manifest) = toml::from_str::<ThemeManifest>(contents) else {
            return ThemeContractState::Incompatible(ThemeIncompatibility::InvalidManifest);
        };
        if manifest.api_version != THEME_API_VERSION {
            return ThemeContractState::Incompatible(ThemeIncompatibility::UnsupportedVersion {
                found: manifest.api_version,
            });
        }

        // A bounded printable name keeps diagnostics useful without becoming another payload
        manifest.name = manifest.name.trim().to_string();
        if manifest.name.is_empty()
            || manifest.name.chars().count() > MAX_THEME_NAME_CHARS
            || manifest.name.chars().any(char::is_control)
        {
            return ThemeContractState::Incompatible(ThemeIncompatibility::InvalidName);
        }

        ThemeContractState::Compatible(manifest)
    }
}
