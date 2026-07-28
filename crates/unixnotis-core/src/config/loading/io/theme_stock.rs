//! Exact-byte migration for stock theme assets that shipped with older releases

use std::io;
use std::path::{Path, PathBuf};

use crate::filesystem::{
    read_regular_file_bounded, regular_file_contents_equal, write_file_atomic_preserving_mode,
    write_file_if_missing,
};
use crate::{DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_WIDGETS_CSS};

use super::{ConfigError, ThemePaths};

const MAX_STOCK_THEME_BYTES: u64 = 1_048_576;
const LEGACY_BACKUP_TAG: &str = "unixnotis-stock-9ca42584";
const LEGACY_PANEL_DIGEST: &str =
    "bd2342e4ff91dab10dbdece082d1c58e9352b3b8167e046697dd921b6de4ceb3";
const LEGACY_WIDGETS_DIGEST: &str =
    "72c0ab3c38557ea10adfee7e2b11a18b94317b9100579101c04beeb47092e5d2";
const LEGACY_MEDIA_DIGEST: &str =
    "f3618bdaf411d4b018cb9aa1688c9be0880a5bdc0016fdb5e35d8ec798ae6b36";

pub(super) fn migrate_known_stock_themes(paths: &ThemePaths) -> Result<(), ConfigError> {
    // Each file migrates independently so one customized layer never changes another layer
    migrate_known_stock_file(
        &paths.panel_css,
        DEFAULT_PANEL_CSS.as_bytes(),
        LEGACY_PANEL_DIGEST,
        LEGACY_BACKUP_TAG,
    )?;
    migrate_known_stock_file(
        &paths.widgets_css,
        DEFAULT_WIDGETS_CSS.as_bytes(),
        LEGACY_WIDGETS_DIGEST,
        LEGACY_BACKUP_TAG,
    )?;
    migrate_known_stock_file(
        &paths.media_css,
        DEFAULT_MEDIA_CSS.as_bytes(),
        LEGACY_MEDIA_DIGEST,
        LEGACY_BACKUP_TAG,
    )?;
    Ok(())
}

pub(super) fn migrate_known_stock_file(
    path: &Path,
    current_stock: &[u8],
    legacy_digest: &str,
    backup_tag: &str,
) -> Result<bool, ConfigError> {
    migrate_stock_file_with_writer(
        path,
        current_stock,
        legacy_digest,
        backup_tag,
        |target, contents| write_file_atomic_preserving_mode(target, contents, 0o644),
    )
}

pub(super) fn migrate_stock_file_with_writer(
    path: &Path,
    current_stock: &[u8],
    legacy_digest: &str,
    backup_tag: &str,
    replace_file: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
) -> Result<bool, ConfigError> {
    // Unknown, unreadable, and oversized files remain user-owned and untouched
    let Ok(existing) = read_regular_file_bounded(path, MAX_STOCK_THEME_BYTES) else {
        return Ok(false);
    };
    if blake3::hash(&existing).to_hex().as_str() != legacy_digest {
        return Ok(false);
    }

    // The exact previous bytes are recoverable before the current stock file is published
    let backup = stock_backup_path(path, backup_tag)?;
    let backup_created = write_file_if_missing(&backup, &existing, 0o644)
        .map_err(|error| migration_error(path, &error))?;
    if !backup_created
        && !regular_file_contents_equal(&backup, &existing, MAX_STOCK_THEME_BYTES)
            .map_err(|error| migration_error(path, &error))?
    {
        return Err(ConfigError::ReadFailed(format!(
            "refusing to replace stock theme because backup differs: {}",
            backup.display()
        )));
    }

    // Atomic replacement keeps the prior complete file visible if publication is interrupted
    replace_file(path, current_stock).map_err(|error| migration_error(path, &error))?;
    Ok(true)
}

pub(super) fn stock_backup_path(path: &Path, backup_tag: &str) -> Result<PathBuf, ConfigError> {
    let file_name = path.file_name().ok_or_else(|| {
        ConfigError::ReadFailed(format!("theme path has no file name: {}", path.display()))
    })?;
    let mut backup_name = file_name.to_os_string();
    backup_name.push(".");
    backup_name.push(backup_tag);
    backup_name.push(".bak");
    Ok(path.with_file_name(backup_name))
}

fn migration_error(path: &Path, error: &io::Error) -> ConfigError {
    ConfigError::ReadFailed(format!(
        "failed to migrate exact stock theme {}: {error}",
        path.display()
    ))
}
