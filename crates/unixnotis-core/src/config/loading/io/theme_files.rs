//! Provisioning and migration for configured theme files

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use tracing::warn;

use crate::{
    Config, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

use super::write::write_if_missing;
use super::{ConfigError, ThemePaths};

static LEGACY_RENAME_WARNED: AtomicBool = AtomicBool::new(false);

impl Config {
    /// Ensure all theme files exist in the config directory
    ///
    /// # Errors
    ///
    /// Returns an error when a missing theme file cannot be created safely
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

        if legacy_contents.is_some() && legacy.exists() {
            let backup = legacy.with_extension("css.bak");
            if !backup.exists() {
                if let Err(err) = fs::rename(&legacy, &backup) {
                    // Non-fatal: leave legacy style.css in place if backup fails
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
}
