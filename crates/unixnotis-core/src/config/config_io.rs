//! Configuration loading, path resolution, and on-disk defaults.
//!
//! Focuses on I/O and filesystem-related helpers for config management.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use thiserror::Error;
use tracing::warn;

use crate::util::expand_tilde;
use crate::{DEFAULT_BASE_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS};

use super::config_runtime::{
    apply_brightness_backend, apply_toggle_backends, apply_volume_backend, sanitize_config,
};
use super::Config;

static LEGACY_RENAME_WARNED: AtomicBool = AtomicBool::new(false);
static INVALID_XDG_WARNED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct ThemePaths {
    // Base directory used to resolve relative theme paths.
    pub base_dir: PathBuf,
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
        let mut ignored_keys = Vec::new();
        let deserializer = toml::de::Deserializer::new(&contents);
        let mut config: Config = serde_ignored::deserialize(deserializer, |path| {
            ignored_keys.push(path.to_string());
        })
        .map_err(|err| ConfigError::ParseFailed(err.to_string()))?;
        if !ignored_keys.is_empty() {
            for key in ignored_keys {
                warn!(key = %key, "unknown config key ignored");
            }
        }
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
            base_dir: base.to_path_buf(),
            base_css: Self::resolve_path(base, &self.theme.base_css),
            popup_css: Self::resolve_path(base, &self.theme.popup_css),
            panel_css: Self::resolve_path(base, &self.theme.panel_css),
            widgets_css: Self::resolve_path(base, &self.theme.widgets_css),
        })
    }

    /// Ensure all theme files exist in the config directory.
    pub fn ensure_theme_files(&self, theme_paths: &ThemePaths) -> Result<(), ConfigError> {
        // Use the same base directory used for resolving theme paths.
        let config_dir = &theme_paths.base_dir;
        fs::create_dir_all(config_dir).map_err(|err| ConfigError::ReadFailed(err.to_string()))?;

        ensure_parent_dir(&theme_paths.base_css)?;
        ensure_parent_dir(&theme_paths.panel_css)?;
        ensure_parent_dir(&theme_paths.popup_css)?;
        ensure_parent_dir(&theme_paths.widgets_css)?;

        let legacy = config_dir.join("style.css");
        let base_exists = theme_paths.base_css.exists();
        let legacy_contents = if base_exists {
            None
        } else {
            fs::read_to_string(&legacy)
                .ok()
                .filter(|contents| !contents.trim().is_empty())
        };

        write_if_missing(
            &theme_paths.base_css,
            legacy_contents.as_deref().unwrap_or(DEFAULT_BASE_CSS),
        )?;
        write_if_missing(&theme_paths.panel_css, DEFAULT_PANEL_CSS)?;
        write_if_missing(&theme_paths.popup_css, DEFAULT_POPUP_CSS)?;
        write_if_missing(&theme_paths.widgets_css, DEFAULT_WIDGETS_CSS)?;

        if legacy_contents.is_some() && legacy.exists() {
            let backup = legacy.with_extension("css.bak");
            if !backup.exists() {
                if let Err(err) = fs::rename(&legacy, &backup) {
                    // Non-fatal: leave legacy style.css in place if backup fails (permissions,
                    // existing paths, or filesystem limitations).
                    if !LEGACY_RENAME_WARNED.swap(true, Ordering::Relaxed) {
                        warn!(
                            ?err,
                            legacy = %legacy.display(),
                            backup = %backup.display(),
                            "failed to rename legacy style.css"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn apply_runtime_defaults(&mut self) {
        apply_volume_backend(&mut self.widgets.volume);
        apply_brightness_backend(&mut self.widgets.brightness);
        apply_toggle_backends(&mut self.widgets.toggles);
        sanitize_config(self);
    }

    /// Return the default config directory based on XDG or $HOME.
    pub fn default_config_dir() -> Result<PathBuf, ConfigError> {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            let trimmed = xdg.trim();
            if !trimmed.is_empty() {
                let path = PathBuf::from(trimmed);
                if path.is_absolute() {
                    // Prefer the XDG base directory when it is explicitly configured.
                    return Ok(path.join("unixnotis"));
                }
            }
            if !INVALID_XDG_WARNED.swap(true, Ordering::Relaxed) {
                warn!("invalid XDG_CONFIG_HOME; falling back to $HOME/.config");
            }
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
        let path = expand_tilde(value);
        let path = PathBuf::from(path.as_ref());
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

fn ensure_parent_dir(path: &Path) -> Result<(), ConfigError> {
    let parent = path.parent().ok_or_else(|| {
        ConfigError::ReadFailed("missing theme file parent directory".to_string())
    })?;
    fs::create_dir_all(parent).map_err(|err| ConfigError::ReadFailed(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::env;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().expect("env lock")
    }

    fn set_env(key: &str, value: Option<&str>) -> Option<String> {
        let previous = env::var(key).ok();
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
        previous
    }

    fn restore_env(key: &str, previous: Option<String>) {
        match previous {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn default_config_dir_ignores_empty_xdg() {
        let _guard = env_lock();
        let home = env::var("HOME").unwrap_or_default();
        if home.trim().is_empty() {
            return;
        }
        let prev_xdg = set_env("XDG_CONFIG_HOME", Some(""));
        let prev_home = set_env("HOME", Some(&home));
        let dir = Config::default_config_dir().expect("config dir");
        assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
        restore_env("XDG_CONFIG_HOME", prev_xdg);
        restore_env("HOME", prev_home);
    }

    #[test]
    fn default_config_dir_ignores_whitespace_xdg() {
        let _guard = env_lock();
        let home = env::var("HOME").unwrap_or_default();
        if home.trim().is_empty() {
            return;
        }
        let prev_xdg = set_env("XDG_CONFIG_HOME", Some("   "));
        let prev_home = set_env("HOME", Some(&home));
        let dir = Config::default_config_dir().expect("config dir");
        assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
        restore_env("XDG_CONFIG_HOME", prev_xdg);
        restore_env("HOME", prev_home);
    }

    #[test]
    fn default_config_dir_ignores_relative_xdg() {
        let _guard = env_lock();
        let home = env::var("HOME").unwrap_or_default();
        if home.trim().is_empty() {
            return;
        }
        let prev_xdg = set_env("XDG_CONFIG_HOME", Some("relative/path"));
        let prev_home = set_env("HOME", Some(&home));
        let dir = Config::default_config_dir().expect("config dir");
        assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
        restore_env("XDG_CONFIG_HOME", prev_xdg);
        restore_env("HOME", prev_home);
    }

    #[test]
    fn default_config_dir_accepts_absolute_xdg() {
        let _guard = env_lock();
        let home = env::var("HOME").unwrap_or_default();
        if home.trim().is_empty() {
            return;
        }
        let xdg = PathBuf::from(home.clone()).join(".config-test");
        let prev_xdg = set_env("XDG_CONFIG_HOME", Some(xdg.to_string_lossy().as_ref()));
        let prev_home = set_env("HOME", Some(&home));
        let dir = Config::default_config_dir().expect("config dir");
        assert_eq!(dir, xdg.join("unixnotis"));
        restore_env("XDG_CONFIG_HOME", prev_xdg);
        restore_env("HOME", prev_home);
    }
}
