//! Configuration loading, path resolution, and on-disk defaults.
//!
//! Focuses on I/O and filesystem-related helpers for config management.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{DEFAULT_BASE_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS};

use super::config_runtime::{apply_brightness_backend, apply_volume_backend};
use super::Config;

#[derive(Debug, Clone)]
pub struct ThemePaths {
    pub base_css: PathBuf,
    pub popup_css: PathBuf,
    pub panel_css: PathBuf,
    pub widgets_css: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFailed(String),
    #[error("failed to parse config: {0}")]
    ParseFailed(String),
    #[error("missing $HOME, unable to resolve config directory")]
    MissingHome,
}

impl Config {
    /// Load configuration from a specific path.
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let contents =
            fs::read_to_string(path).map_err(|err| ConfigError::ReadFailed(err.to_string()))?;
        let mut config: Config =
            toml::from_str(&contents).map_err(|err| ConfigError::ParseFailed(err.to_string()))?;
        config.apply_runtime_defaults();
        Ok(config)
    }

    /// Load configuration from the default XDG config location, if present.
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = Self::default_config_path()?;
        if !path.exists() {
            let mut config = Self::default();
            config.apply_runtime_defaults();
            return Ok(config);
        }
        Self::load_from_path(&path)
    }

    /// Resolve configured CSS paths relative to the config directory.
    pub fn resolve_theme_paths(&self) -> Result<ThemePaths, ConfigError> {
        let base = Self::default_config_dir()?;
        self.resolve_theme_paths_from(&base)
    }

    /// Resolve configured CSS paths relative to an explicit config directory.
    pub fn resolve_theme_paths_from(&self, base: &Path) -> Result<ThemePaths, ConfigError> {
        // Resolve relative paths against the supplied config directory.
        Ok(ThemePaths {
            base_css: Self::resolve_path(base, &self.theme.base_css),
            popup_css: Self::resolve_path(base, &self.theme.popup_css),
            panel_css: Self::resolve_path(base, &self.theme.panel_css),
            widgets_css: Self::resolve_path(base, &self.theme.widgets_css),
        })
    }

    /// Ensure all theme files exist in the config directory.
    pub fn ensure_theme_files(&self, theme_paths: &ThemePaths) -> Result<(), ConfigError> {
        let config_dir = Self::default_config_dir()?;
        fs::create_dir_all(&config_dir)
            .map_err(|err| ConfigError::ReadFailed(err.to_string()))?;

        let legacy = config_dir.join("style.css");
        let legacy_contents = fs::read_to_string(&legacy)
            .ok()
            .filter(|contents| !contents.trim().is_empty());

        write_if_missing(
            &theme_paths.base_css,
            legacy_contents
                .as_deref()
                .unwrap_or(DEFAULT_BASE_CSS),
        )?;
        write_if_missing(&theme_paths.panel_css, DEFAULT_PANEL_CSS)?;
        write_if_missing(&theme_paths.popup_css, DEFAULT_POPUP_CSS)?;
        write_if_missing(&theme_paths.widgets_css, DEFAULT_WIDGETS_CSS)?;

        if legacy_contents.is_some() && legacy.exists() {
            let backup = legacy.with_extension("css.bak");
            if !backup.exists() {
                let _ = fs::rename(&legacy, &backup);
            }
        }

        Ok(())
    }

    fn apply_runtime_defaults(&mut self) {
        apply_volume_backend(&mut self.widgets.volume);
        apply_brightness_backend(&mut self.widgets.brightness);
    }

    /// Return the default config directory based on XDG or $HOME.
    pub fn default_config_dir() -> Result<PathBuf, ConfigError> {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            // Prefer the XDG base directory when it is explicitly configured.
            return Ok(PathBuf::from(xdg).join("unixnotis"));
        }
        let home = env::var("HOME").map_err(|_| ConfigError::MissingHome)?;
        // Fall back to the standard $HOME/.config path for predictable location.
        Ok(PathBuf::from(home).join(".config").join("unixnotis"))
    }

    /// Return the default config file path.
    pub fn default_config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::default_config_dir()?.join("config.toml"))
    }

    fn resolve_path(base: &Path, value: &str) -> PathBuf {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            base.join(path)
        }
    }
}

fn write_if_missing(path: &Path, contents: &str) -> Result<(), ConfigError> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, contents).map_err(|err| ConfigError::ReadFailed(err.to_string()))
}
