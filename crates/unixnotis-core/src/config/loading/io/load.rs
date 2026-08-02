//! Bounded configuration loading and parsing
//!
//! Focuses on I/O and filesystem-related helpers for config management

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::config::runtime::{apply_brightness_backend, apply_volume_backend, sanitize_config};
use crate::config::schema::deserialize_config_with_migrations;
use crate::{log_config_diagnostics, Config, ConfigLoadReport};

use super::super::diagnostics::{
    adjustment_diagnostics, empty_exact_media_policy_diagnostic, migrated_field_diagnostic,
    migration_diagnostic, unknown_key_diagnostic,
};
use super::ConfigError;

/// Maximum accepted `config.toml` size before parsing
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

impl Config {
    /// Load configuration from a specific path
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or its TOML cannot be parsed
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let report = Self::load_from_path_with_report(path)?;
        log_config_diagnostics(&report.diagnostics);
        Ok(report.config)
    }

    /// Load configuration from a specific path with structured diagnostics
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or its TOML cannot be parsed
    pub fn load_from_path_with_report(path: &Path) -> Result<ConfigLoadReport, ConfigError> {
        let contents = read_config_bounded(path)?;
        Self::parse_with_report(&contents)
    }

    /// Parse and migrate configuration text without reading the filesystem
    ///
    /// # Errors
    ///
    /// Returns an error for invalid TOML or unsupported schema versions
    pub fn parse(contents: &str) -> Result<Self, ConfigError> {
        let report = Self::parse_with_report(contents)?;
        log_config_diagnostics(&report.diagnostics);
        Ok(report.config)
    }

    /// Parse and migrate configuration text with structured diagnostics
    ///
    /// # Errors
    ///
    /// Returns an error for invalid TOML or unsupported schema versions
    pub fn parse_with_report(contents: &str) -> Result<ConfigLoadReport, ConfigError> {
        let (mut config, ignored_keys, migrated_paths) =
            deserialize_config_with_migrations(contents).map_err(ConfigError::ParseFailed)?;
        let mut diagnostics = migration_diagnostic(contents)
            .into_iter()
            .collect::<Vec<_>>();
        diagnostics.extend(empty_exact_media_policy_diagnostic(contents));
        diagnostics.extend(migrated_paths.into_iter().map(migrated_field_diagnostic));
        diagnostics.extend(ignored_keys.into_iter().map(unknown_key_diagnostic));
        let before_runtime = config.clone();
        config.apply_runtime_defaults();
        diagnostics.extend(adjustment_diagnostics(&before_runtime, &config));
        Ok(ConfigLoadReport {
            config,
            diagnostics,
        })
    }

    /// Load configuration from the default XDG config location, if present
    ///
    /// # Errors
    ///
    /// Returns an error when the default location cannot be resolved or an existing config file
    /// cannot be read and parsed
    pub fn load_default() -> Result<Self, ConfigError> {
        let report = Self::load_default_with_report()?;
        log_config_diagnostics(&report.diagnostics);
        Ok(report.config)
    }

    /// Load default configuration with structured diagnostics
    ///
    /// # Errors
    ///
    /// Returns an error when the default location cannot be resolved or read
    pub fn load_default_with_report() -> Result<ConfigLoadReport, ConfigError> {
        let path = Self::default_config_path()?;
        if !path.exists() {
            let mut config = Self::default();
            let before_runtime = config.clone();
            config.apply_runtime_defaults();
            return Ok(ConfigLoadReport {
                diagnostics: adjustment_diagnostics(&before_runtime, &config),
                config,
            });
        }
        Self::load_from_path_with_report(&path)
    }

    fn apply_runtime_defaults(&mut self) {
        apply_volume_backend(&mut self.widgets.volume);
        apply_brightness_backend(&mut self.widgets.brightness);
        sanitize_config(self);
    }
}

fn read_config_bounded(path: &Path) -> Result<String, ConfigError> {
    // Opening first keeps metadata and reads tied to the same filesystem object
    let file = File::open(path).map_err(|err| ConfigError::ReadFailed(err.to_string()))?;
    let initial_size = file
        .metadata()
        .map_err(|err| ConfigError::ReadFailed(err.to_string()))?
        .len();
    read_config_contents(file, initial_size)
}

pub(super) fn read_config_contents<R: Read>(
    reader: R,
    initial_size: u64,
) -> Result<String, ConfigError> {
    if initial_size > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge {
            size: initial_size,
            max: MAX_CONFIG_BYTES,
        });
    }

    // The extra byte detects files that grow after metadata is checked
    let mut contents = String::with_capacity(initial_size as usize);
    reader
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_string(&mut contents)
        .map_err(|err| ConfigError::ReadFailed(err.to_string()))?;
    let observed_size = contents.len() as u64;
    if observed_size > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge {
            size: observed_size,
            max: MAX_CONFIG_BYTES,
        });
    }
    Ok(contents)
}
