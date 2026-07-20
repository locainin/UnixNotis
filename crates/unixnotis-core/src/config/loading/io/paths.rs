//! Configuration and theme path discovery

use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use tracing::warn;

use crate::util::expand_tilde;
use crate::Config;

use super::ConfigError;

static INVALID_XDG_WARNED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct ThemePaths {
    // Base directory used to resolve relative theme paths
    pub base_dir: PathBuf,
    pub base_css: PathBuf,
    pub popup_css: PathBuf,
    pub panel_css: PathBuf,
    pub widgets_css: PathBuf,
    pub media_css: PathBuf,
}

impl Config {
    /// Resolve configured CSS paths relative to the config directory
    ///
    /// # Errors
    ///
    /// Returns an error when the default config directory cannot be resolved
    pub fn resolve_theme_paths(&self) -> Result<ThemePaths, ConfigError> {
        let base = Self::default_config_dir()?;
        self.resolve_theme_paths_from(&base)
    }

    /// Resolve the config directory that should anchor relative theme paths
    ///
    /// # Errors
    ///
    /// Returns an error when a parentless relative path requires the current directory and that
    /// directory cannot be read
    pub fn config_dir_for_path(path: &Path) -> Result<PathBuf, ConfigError> {
        if let Some(parent) = path.parent() {
            // Plain file names report an empty parent, so skip that case
            if !parent.as_os_str().is_empty() {
                return Ok(parent.to_path_buf());
            }
        }
        env::current_dir().map_err(|err| ConfigError::ReadFailed(err.to_string()))
    }

    /// Resolve configured CSS paths relative to an explicit config directory
    ///
    /// # Errors
    ///
    /// This operation currently has no failure path; the result type is retained for API
    /// compatibility with other theme-resolution helpers
    pub fn resolve_theme_paths_from(&self, base: &Path) -> Result<ThemePaths, ConfigError> {
        // Resolve relative paths against the supplied config directory
        Ok(ThemePaths {
            base_dir: base.to_path_buf(),
            base_css: Self::resolve_path(base, &self.theme.base_css),
            popup_css: Self::resolve_path(base, &self.theme.popup_css),
            panel_css: Self::resolve_path(base, &self.theme.panel_css),
            widgets_css: Self::resolve_path(base, &self.theme.widgets_css),
            media_css: Self::resolve_path(base, &self.theme.media_css),
        })
    }

    /// Return the default config directory based on XDG or $HOME
    ///
    /// # Errors
    ///
    /// Returns an error when neither a valid absolute `XDG_CONFIG_HOME` nor `HOME` is available
    pub fn default_config_dir() -> Result<PathBuf, ConfigError> {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            let trimmed = xdg.trim();
            if !trimmed.is_empty() {
                let path = PathBuf::from(trimmed);
                if path.is_absolute() {
                    // Prefer the XDG base directory when it is explicitly configured
                    return Ok(path.join("unixnotis"));
                }
            }
            if INVALID_XDG_WARNED
                .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                warn!("invalid XDG_CONFIG_HOME; falling back to $HOME/.config");
            }
        }
        let home = env::var("HOME").map_err(|_error| ConfigError::MissingHome)?;
        // Fall back to the standard $HOME/.config path for predictable location
        Ok(PathBuf::from(home).join(".config").join("unixnotis"))
    }

    /// Return the default config file path
    ///
    /// # Errors
    ///
    /// Returns an error when the default config directory cannot be resolved
    pub fn default_config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::default_config_dir()?.join("config.toml"))
    }

    /// Resolve the environment-selected config file or the normal default file
    ///
    /// # Errors
    ///
    /// Returns an error when no explicit path is set and the default directory cannot resolve
    pub fn active_config_path() -> Result<PathBuf, ConfigError> {
        let configured =
            env::var_os(crate::util::CONFIG_PATH_ENV).filter(|value| !value.is_empty());
        configured.map_or_else(Self::default_config_path, |path| Ok(PathBuf::from(path)))
    }

    fn resolve_path(base: &Path, value: &str) -> PathBuf {
        let path = expand_tilde(value);
        let path = PathBuf::from(path.as_ref());
        if path.is_absolute() {
            path
        } else {
            base.join(path)
        }
    }
}
