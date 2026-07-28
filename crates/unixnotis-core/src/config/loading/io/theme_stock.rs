//! Exact-byte migration for stock theme assets that shipped with older releases

use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::filesystem::{
    open_regular_file, regular_file_contents_equal, write_file_atomic_preserving_mode,
    write_file_if_missing,
};
use crate::{DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_WIDGETS_CSS};

use super::{ConfigError, ThemePaths};

const MAX_STOCK_THEME_BYTES: u64 = 1_048_576;
const MAX_BACKUP_COLLISION_RETRIES: u8 = 8;
const LEGACY_BACKUP_TAG: &str = "unixnotis-stock-9ca42584";
const LEGACY_PANEL_DIGEST: &str =
    "bd2342e4ff91dab10dbdece082d1c58e9352b3b8167e046697dd921b6de4ceb3";
const LEGACY_WIDGETS_DIGEST: &str =
    "72c0ab3c38557ea10adfee7e2b11a18b94317b9100579101c04beeb47092e5d2";
const LEGACY_MEDIA_DIGEST: &str =
    "f3618bdaf411d4b018cb9aa1688c9be0880a5bdc0016fdb5e35d8ec798ae6b36";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileSnapshot {
    device: u64,
    inode: u64,
    size: u64,
    modified: SystemTime,
    digest: blake3::Hash,
}

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
        replace_file_if_snapshot_matches,
    )
}

pub(super) fn migrate_stock_file_with_writer(
    path: &Path,
    current_stock: &[u8],
    legacy_digest: &str,
    backup_tag: &str,
    replace_file: impl FnOnce(&Path, &[u8], &FileSnapshot) -> io::Result<bool>,
) -> Result<bool, ConfigError> {
    // Unknown, unreadable, and oversized files remain user-owned and untouched
    let Ok((original, existing)) = inspect_stock_file(path) else {
        return Ok(false);
    };
    if original.digest.to_hex().as_str() != legacy_digest {
        return Ok(false);
    }

    // The exact previous bytes are recoverable before the current stock file is published
    let Some(_backup) = reserve_stock_backup(path, backup_tag, &existing)? else {
        // A backup problem must retain the old theme without blocking daemon startup
        return Ok(false);
    };

    // The replacement boundary rechecks the exact object and bytes that were backed up
    replace_file(path, current_stock, &original).map_err(|error| migration_error(path, &error))
}

pub(super) fn replace_file_if_snapshot_matches(
    path: &Path,
    current_stock: &[u8],
    original: &FileSnapshot,
) -> io::Result<bool> {
    let (current, _contents) = inspect_stock_file(path)?;
    if &current != original {
        // A concurrent edit always wins over automatic stock migration
        return Ok(false);
    }

    // Atomic publication keeps either the complete old file or complete new file visible
    write_file_atomic_preserving_mode(path, current_stock, 0o644)?;
    Ok(true)
}

fn inspect_stock_file(path: &Path) -> io::Result<(FileSnapshot, Vec<u8>)> {
    let mut file = open_regular_file(path)?;
    let before = file.metadata()?;
    if before.len() > MAX_STOCK_THEME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stock theme exceeds migration size limit",
        ));
    }

    let capacity = usize::try_from(before.len())
        .map_err(|_error| io::Error::new(io::ErrorKind::InvalidData, "theme size is invalid"))?;
    let mut contents = Vec::with_capacity(capacity);
    file.by_ref()
        .take(MAX_STOCK_THEME_BYTES.saturating_add(1))
        .read_to_end(&mut contents)?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > MAX_STOCK_THEME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stock theme exceeds migration size limit",
        ));
    }

    let after = file.metadata()?;
    let before_snapshot = snapshot_for_metadata(&before, &contents)?;
    let after_snapshot = snapshot_for_metadata(&after, &contents)?;
    if before_snapshot != after_snapshot {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "stock theme changed while it was being inspected",
        ));
    }
    Ok((after_snapshot, contents))
}

fn snapshot_for_metadata(
    metadata: &std::fs::Metadata,
    contents: &[u8],
) -> io::Result<FileSnapshot> {
    Ok(FileSnapshot {
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.len(),
        modified: metadata.modified()?,
        digest: blake3::hash(contents),
    })
}

fn reserve_stock_backup(
    path: &Path,
    backup_tag: &str,
    existing: &[u8],
) -> Result<Option<PathBuf>, ConfigError> {
    let base = stock_backup_path(path, backup_tag)?;
    for suffix in 0..=MAX_BACKUP_COLLISION_RETRIES {
        let candidate = backup_candidate(&base, suffix);
        match write_file_if_missing(&candidate, existing, 0o644) {
            Ok(true) => return Ok(Some(candidate)),
            Ok(false) => {
                // Identical content is already a complete valid backup
                if regular_file_contents_equal(&candidate, existing, MAX_STOCK_THEME_BYTES)
                    .unwrap_or(false)
                {
                    return Ok(Some(candidate));
                }
            }
            Err(_) => {
                // Another suffix may still be usable after a single path collision or race
            }
        }
    }
    Ok(None)
}

fn backup_candidate(base: &Path, suffix: u8) -> PathBuf {
    if suffix == 0 {
        return base.to_path_buf();
    }
    let mut name = base.as_os_str().to_os_string();
    name.push(format!(".{suffix}"));
    PathBuf::from(name)
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
