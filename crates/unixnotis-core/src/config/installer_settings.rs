//! Shared installer settings used by local reset frontends

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::filesystem::{create_directory_all, write_file_if_missing};

pub const INSTALLER_CONFIG_FILE: &str = "installer.toml";
pub const DEFAULT_BACKUP_RETENTION: usize = 3;

const INSTALLER_CONFIG_TEMPLATE: &str = r"# UnixNotis installer settings
# Backup retention for config/theme resets
[backups]
keep = 3
";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InstallerConfig {
    pub backups: BackupConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BackupConfig {
    pub keep: usize,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            keep: DEFAULT_BACKUP_RETENTION,
        }
    }
}

#[must_use]
pub fn installer_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(INSTALLER_CONFIG_FILE)
}

/// Ensure that the shared retention settings file exists
///
/// # Errors
///
/// Returns an error when the configuration directory or settings file cannot
/// be created
pub fn ensure_installer_config(config_dir: &Path) -> Result<(PathBuf, bool)> {
    create_directory_all(config_dir, 0o700).context("create UnixNotis configuration directory")?;
    let config_path = installer_config_path(config_dir);
    let created = write_file_if_missing(&config_path, INSTALLER_CONFIG_TEMPLATE.as_bytes(), 0o644)
        .context("write installer settings")?;
    Ok((config_path, created))
}

/// Read retention settings while distinguishing absence from I/O failure
///
/// # Errors
///
/// Returns an error when an existing settings file cannot be read or parsed
pub fn load_installer_config(config_dir: &Path) -> Result<InstallerConfig> {
    let config_path = installer_config_path(config_dir);
    let contents = match fs::read_to_string(&config_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            // A first-run install has no settings yet, so use the shared default
            return Ok(InstallerConfig::default());
        }
        Err(error) => {
            // Permission, encoding, directory, and other failures must reach both callers
            return Err(error).with_context(|| format!("read {}", config_path.display()));
        }
    };
    toml::from_str(&contents).with_context(|| format!("parse {}", config_path.display()))
}

#[cfg(test)]
#[path = "installer_settings/tests/mod.rs"]
mod tests;
