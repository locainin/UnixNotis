//! Configuration loading, path resolution, and on-disk defaults
//!
//! Focuses on I/O and filesystem-related helpers for config management

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use thiserror::Error;
use tracing::warn;

use crate::filesystem::{write_file_atomic, write_file_if_missing};
use crate::util::expand_tilde;
use crate::{
    DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_OVERRIDES_CSS, DEFAULT_PANEL_CSS,
    DEFAULT_POPUP_CSS, DEFAULT_SCRIPTS, DEFAULT_WIDGETS_CSS,
};

use super::runtime::{apply_brightness_backend, apply_volume_backend, sanitize_config};
use super::Config;

static LEGACY_RENAME_WARNED: AtomicBool = AtomicBool::new(false);
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
    pub overrides_css: PathBuf,
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
    /// Load configuration from a specific path
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let contents =
            fs::read_to_string(path).map_err(|err| ConfigError::ReadFailed(err.to_string()))?;
        let mut ignored_keys = Vec::new();
        // Build the TOML deserializer from the file text
        let deserializer = toml::de::Deserializer::new(&contents);
        let mut config: Config = serde_ignored::deserialize(deserializer, |path| {
            ignored_keys.push(path.to_string());
        })
        .map_err(|err| ConfigError::ParseFailed(err.to_string()))?;
        for key in ignored_keys {
            warn!(key = %key, "unknown config key ignored");
        }
        config.apply_runtime_defaults();
        Ok(config)
    }

    /// Load configuration from the default XDG config location, if present
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = Self::default_config_path()?;
        if !path.exists() {
            let mut config = Self::default();
            config.apply_runtime_defaults();
            return Ok(config);
        }
        Self::load_from_path(&path)
    }

    /// Resolve configured CSS paths relative to the config directory
    pub fn resolve_theme_paths(&self) -> Result<ThemePaths, ConfigError> {
        let base = Self::default_config_dir()?;
        self.resolve_theme_paths_from(&base)
    }

    /// Resolve the config directory that should anchor relative theme paths
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
    pub fn resolve_theme_paths_from(&self, base: &Path) -> Result<ThemePaths, ConfigError> {
        // Resolve relative paths against the supplied config directory
        Ok(ThemePaths {
            base_dir: base.to_path_buf(),
            base_css: Self::resolve_path(base, &self.theme.base_css),
            popup_css: Self::resolve_path(base, &self.theme.popup_css),
            panel_css: Self::resolve_path(base, &self.theme.panel_css),
            widgets_css: Self::resolve_path(base, &self.theme.widgets_css),
            media_css: Self::resolve_path(base, &self.theme.media_css),
            overrides_css: Self::resolve_path(base, &self.theme.overrides_css),
        })
    }

    /// Ensure all theme files exist in the config directory
    pub fn ensure_theme_files(&self, theme_paths: &ThemePaths) -> Result<(), ConfigError> {
        // Use the same base directory used for resolving theme paths
        let config_dir = &theme_paths.base_dir;

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
        write_if_missing(&theme_paths.media_css, DEFAULT_MEDIA_CSS)?;
        write_if_missing(&theme_paths.overrides_css, DEFAULT_OVERRIDES_CSS)?;

        if legacy_contents.is_some() && legacy.exists() {
            let backup = legacy.with_extension("css.bak");
            if !backup.exists() {
                if let Err(err) = fs::rename(&legacy, &backup) {
                    // Non-fatal: leave legacy style.css in place if backup fails (permissions,
                    // existing paths, or filesystem limitations)
                    if LEGACY_RENAME_WARNED
                        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                        .is_ok()
                    {
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

    /// Ensure helper scripts used by the shipped default config exist
    pub fn ensure_default_scripts_in(config_dir: &Path) -> Result<(), ConfigError> {
        for script in DEFAULT_SCRIPTS {
            let path = config_dir.join(script.relative_path);
            // Existing files are preserved so user-edited helpers are not overwritten
            if !path.exists() {
                write_default_script(&path, script.contents)?;
            }
            // Relative commands run the helper directly, so execute bits must be present
            set_executable(&path)?;
        }
        Ok(())
    }

    /// Overwrite helper scripts with the built-in defaults
    pub fn write_default_scripts_in(config_dir: &Path) -> Result<(), ConfigError> {
        for script in DEFAULT_SCRIPTS {
            write_default_script(&config_dir.join(script.relative_path), script.contents)?;
        }
        Ok(())
    }

    fn apply_runtime_defaults(&mut self) {
        apply_volume_backend(&mut self.widgets.volume);
        apply_brightness_backend(&mut self.widgets.brightness);
        sanitize_config(self);
    }

    /// Return the default config directory based on XDG or $HOME
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
        let home = env::var("HOME").map_err(|_| ConfigError::MissingHome)?;
        // Fall back to the standard $HOME/.config path for predictable location
        Ok(PathBuf::from(home).join(".config").join("unixnotis"))
    }

    /// Return the default config file path
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
    write_file_if_missing(path, contents.as_bytes(), 0o644)
        .map(|_created| ())
        .map_err(|err| ConfigError::ReadFailed(err.to_string()))
}

fn write_default_script(path: &Path, contents: &str) -> Result<(), ConfigError> {
    // Script reset uses the same atomic path as startup provisioning
    // This keeps installer resets from leaving half-written helpers behind
    write_file_atomic(path, contents.as_bytes(), 0o755)
        .map_err(|err| ConfigError::ReadFailed(err.to_string()))?;
    set_executable(path)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|err| ConfigError::ReadFailed(err.to_string()))?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions).map_err(|err| ConfigError::ReadFailed(err.to_string()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), ConfigError> {
    Ok(())
}

#[cfg(test)]
#[path = "tests/io.rs"]
mod tests;
