//! Provisioning and migration for configured theme files

use std::sync::atomic::{AtomicBool, Ordering};

use tracing::warn;

use crate::filesystem::{read_regular_file_bounded, rename_regular_file_no_replace};
use crate::{
    Config, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

use super::write::write_if_missing;
use super::{ConfigError, ThemePaths};

static LEGACY_RENAME_WARNED: AtomicBool = AtomicBool::new(false);
const MAX_LEGACY_THEME_BYTES: u64 = 16 * 1024 * 1024;

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
        let legacy_contents = (!base_exists).then(|| read_legacy_theme(&legacy)).flatten();

        write_if_missing(
            &theme_paths.base_css,
            legacy_contents.as_deref().unwrap_or(DEFAULT_BASE_CSS),
        )?;
        write_if_missing(&theme_paths.panel_css, DEFAULT_PANEL_CSS)?;
        write_if_missing(&theme_paths.popup_css, DEFAULT_POPUP_CSS)?;
        write_if_missing(&theme_paths.widgets_css, DEFAULT_WIDGETS_CSS)?;
        write_if_missing(&theme_paths.media_css, DEFAULT_MEDIA_CSS)?;

        if legacy_contents.is_some() {
            let backup = legacy.with_extension("css.bak");
            if let Err(err) = rename_regular_file_no_replace(&legacy, &backup) {
                // Base CSS is already safe, so a failed backup move remains non-fatal
                warn_legacy_rename_once(&legacy, &backup, &err);
            }
        }

        Ok(())
    }
}

fn read_legacy_theme(path: &std::path::Path) -> Option<String> {
    // Legacy migration accepts only bounded UTF-8 from one stable regular-file descriptor
    let bytes = read_regular_file_bounded(path, MAX_LEGACY_THEME_BYTES).ok()?;
    String::from_utf8(bytes)
        .ok()
        .filter(|contents| !contents.trim().is_empty())
}

pub(super) fn warn_legacy_rename_once(
    source: &std::path::Path,
    backup: &std::path::Path,
    err: &std::io::Error,
) -> bool {
    if LEGACY_RENAME_WARNED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        warn!(
            ?err,
            legacy = %source.display(),
            backup = %backup.display(),
            "failed to rename legacy style.css"
        );
        true
    } else {
        false
    }
}
