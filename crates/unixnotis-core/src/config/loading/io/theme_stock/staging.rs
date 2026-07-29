//! Collision-safe versioned stock theme staging

use std::io;
use std::path::{Path, PathBuf};

use crate::filesystem::{regular_file_contents_equal, write_file_if_missing};
use crate::{DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS};

use super::super::{ConfigError, ThemePaths};
use super::files::stock_preview_candidates;
use super::MAX_STOCK_THEME_BYTES;

pub(in crate::config::loading::io) fn stage_current_stock_themes(
    paths: &ThemePaths,
) -> Result<(), ConfigError> {
    // Startup only adds versioned siblings and never replaces an active or preview file
    for (path, contents) in [
        (&paths.panel_css, DEFAULT_PANEL_CSS),
        (&paths.popup_css, DEFAULT_POPUP_CSS),
        (&paths.widgets_css, DEFAULT_WIDGETS_CSS),
        (&paths.media_css, DEFAULT_MEDIA_CSS),
    ] {
        let _preview = stage_stock_preview(path, contents.as_bytes())?;
    }
    Ok(())
}

pub(super) fn stage_stock_preview(path: &Path, contents: &[u8]) -> Result<PathBuf, ConfigError> {
    let candidates = stock_preview_candidates(path)?;
    let mut last_error = None;
    for candidate in candidates {
        match write_file_if_missing(&candidate, contents, 0o644) {
            Ok(true) => return Ok(candidate),
            Ok(false) => {
                // Exact bytes make an existing staged file safe to advertise as stock
                if regular_file_contents_equal(&candidate, contents, MAX_STOCK_THEME_BYTES)
                    .unwrap_or(false)
                {
                    return Ok(candidate);
                }
            }
            Err(error) => last_error = Some(error),
        }
    }

    let error = last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "every versioned stock preview path is occupied",
        )
    });
    Err(staging_error(path, &error))
}

pub(super) fn find_exact_stock_preview(
    path: &Path,
    contents: &[u8],
) -> Result<PathBuf, ConfigError> {
    for candidate in stock_preview_candidates(path)? {
        // Preview never loads a file merely because its name resembles a stock asset
        if regular_file_contents_equal(&candidate, contents, MAX_STOCK_THEME_BYTES).unwrap_or(false)
        {
            return Ok(candidate);
        }
    }
    Err(ConfigError::ReadFailed(
        "verified stock theme preview is unavailable".to_string(),
    ))
}

fn staging_error(path: &Path, error: &io::Error) -> ConfigError {
    ConfigError::ReadFailed(format!(
        "failed to stage current stock theme {}: {error}",
        path.display()
    ))
}
