//! Detection and explicitly approved stock theme migration

use std::io;
use std::os::unix::ffi::OsStrExt;

use crate::filesystem::{
    open_regular_file, write_file_atomic_preserving_mode, write_file_if_missing,
};

use super::super::{ConfigError, ThemePaths};
use super::files::{inspect_stock_file, reserve_stock_backup, stock_keep_marker_path};
use super::model::{
    FileSnapshot, StockThemeApplyReport, StockThemeCandidate, StockThemeLayer, StockThemeMigration,
};
use super::staging::find_exact_stock_preview;

const LEGACY_PANEL_DIGEST: &str =
    "bd2342e4ff91dab10dbdece082d1c58e9352b3b8167e046697dd921b6de4ceb3";
const LEGACY_WIDGETS_DIGEST: &str =
    "72c0ab3c38557ea10adfee7e2b11a18b94317b9100579101c04beeb47092e5d2";
const LEGACY_MEDIA_DIGEST: &str =
    "f3618bdaf411d4b018cb9aa1688c9be0880a5bdc0016fdb5e35d8ec798ae6b36";
const KEEP_MARKER_CONTENTS: &[u8] = b"UnixNotis stock theme kept for this release\n";

#[derive(Clone, Copy)]
pub(super) struct LegacyThemeSpec<'a> {
    pub(super) layer: StockThemeLayer,
    pub(super) digest: &'a str,
}

const LEGACY_THEME_SPECS: [LegacyThemeSpec<'static>; 3] = [
    LegacyThemeSpec {
        layer: StockThemeLayer::Panel,
        digest: LEGACY_PANEL_DIGEST,
    },
    LegacyThemeSpec {
        layer: StockThemeLayer::Widgets,
        digest: LEGACY_WIDGETS_DIGEST,
    },
    LegacyThemeSpec {
        layer: StockThemeLayer::Media,
        digest: LEGACY_MEDIA_DIGEST,
    },
];

/// Detect exact theme files shipped by a previous `UnixNotis` release
///
/// Customized, unreadable, linked, oversized, and current files remain outside the plan
///
/// # Errors
///
/// Returns an error when a persisted Keep Current marker has an unsafe file shape
pub fn detect_stock_theme_migration(
    paths: &ThemePaths,
) -> Result<Option<StockThemeMigration>, ConfigError> {
    detect_stock_theme_migration_with_specs(paths, &LEGACY_THEME_SPECS)
}

pub(super) fn detect_stock_theme_migration_with_specs(
    paths: &ThemePaths,
    specs: &[LegacyThemeSpec<'_>],
) -> Result<Option<StockThemeMigration>, ConfigError> {
    if keep_marker_exists(paths)? {
        return Ok(None);
    }

    let mut candidates = Vec::new();
    for spec in specs {
        let path = spec.layer.path(paths);
        // Inspection failures preserve user ownership and cannot create an eligible action
        let Ok((snapshot, original_contents)) = inspect_stock_file(path) else {
            continue;
        };
        if snapshot.digest.to_hex().as_str() != spec.digest {
            continue;
        }
        candidates.push(StockThemeCandidate {
            layer: spec.layer,
            path: path.to_path_buf(),
            snapshot,
            original_contents,
        });
    }

    if candidates.is_empty() {
        return Ok(None);
    }
    let fingerprint = migration_fingerprint(&candidates);
    Ok(Some(StockThemeMigration {
        candidates,
        fingerprint,
    }))
}

impl StockThemeMigration {
    /// Build panel CSS paths that point eligible layers at verified staged stock files
    ///
    /// # Errors
    ///
    /// Returns an error when configuration paths changed or no exact preview remains
    pub fn preview_paths(&self, active: &ThemePaths) -> Result<ThemePaths, ConfigError> {
        validate_plan_paths(active, self)?;
        let mut preview = active.clone();
        for candidate in &self.candidates {
            let path = find_exact_stock_preview(
                candidate.layer.path(active),
                candidate.layer.current_contents(),
            )?;
            candidate.layer.set_path(&mut preview, path);
        }
        Ok(preview)
    }
}

/// Apply one still-current migration plan after an explicit user action
///
/// Every eligible file is backed up and revalidated before any replacement begins
///
/// # Errors
///
/// Returns an error without replacing a stale, edited, linked, or unbacked-up candidate
pub fn apply_stock_theme_migration(
    paths: &ThemePaths,
    migration: &StockThemeMigration,
) -> Result<StockThemeApplyReport, ConfigError> {
    validate_plan_paths(paths, migration)?;

    // Validation before backup prevents obsolete UI actions from creating misleading backups
    for candidate in &migration.candidates {
        require_matching_snapshot(candidate)?;
    }
    for candidate in &migration.candidates {
        let _backup = reserve_stock_backup(&candidate.path, &candidate.original_contents)?;
    }
    // A second whole-plan check ensures backup I/O did not hide an intervening edit
    for candidate in &migration.candidates {
        require_matching_snapshot(candidate)?;
    }

    let mut updated_layers = 0;
    for candidate in &migration.candidates {
        if !replace_file_if_snapshot_matches(
            &candidate.path,
            candidate.layer.current_contents(),
            &candidate.snapshot,
        )
        .map_err(|error| migration_error(&error))?
        {
            return Err(stale_plan_error());
        }
        updated_layers += 1;
    }

    Ok(StockThemeApplyReport { updated_layers })
}

/// Persist the explicit choice to retain current files for this `UnixNotis` release
///
/// # Errors
///
/// Returns an error when the marker cannot be created as a regular file
pub fn keep_current_stock_theme(paths: &ThemePaths) -> Result<(), ConfigError> {
    let marker = stock_keep_marker_path(&paths.base_dir);
    match write_file_if_missing(&marker, KEEP_MARKER_CONTENTS, 0o644) {
        Ok(true) => Ok(()),
        Ok(false) => open_regular_file(&marker)
            .map(|_file| ())
            .map_err(|error| marker_error(&error)),
        Err(error) => Err(marker_error(&error)),
    }
}

pub(super) fn replace_file_if_snapshot_matches(
    path: &std::path::Path,
    current_stock: &[u8],
    original: &FileSnapshot,
) -> io::Result<bool> {
    let (current, _contents) = inspect_stock_file(path)?;
    if &current != original {
        // The editor always wins when the target changed after the approval plan was built
        return Ok(false);
    }
    write_file_atomic_preserving_mode(path, current_stock, 0o644)?;
    Ok(true)
}

fn require_matching_snapshot(candidate: &StockThemeCandidate) -> Result<(), ConfigError> {
    let (current, _contents) =
        inspect_stock_file(&candidate.path).map_err(|error| migration_error(&error))?;
    if current == candidate.snapshot {
        Ok(())
    } else {
        Err(stale_plan_error())
    }
}

fn validate_plan_paths(
    paths: &ThemePaths,
    migration: &StockThemeMigration,
) -> Result<(), ConfigError> {
    let unchanged = migration
        .candidates
        .iter()
        .all(|candidate| candidate.layer.path(paths) == candidate.path);
    if unchanged {
        Ok(())
    } else {
        Err(ConfigError::ReadFailed(
            "theme configuration changed after the migration notice was shown".to_string(),
        ))
    }
}

fn keep_marker_exists(paths: &ThemePaths) -> Result<bool, ConfigError> {
    let marker = stock_keep_marker_path(&paths.base_dir);
    match open_regular_file(&marker) {
        Ok(_file) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(marker_error(&error)),
    }
}

fn migration_fingerprint(candidates: &[StockThemeCandidate]) -> String {
    let mut hasher = blake3::Hasher::new();
    for candidate in candidates {
        hasher.update(candidate.layer.label().as_bytes());
        hasher.update(candidate.path.as_os_str().as_bytes());
        hasher.update(&candidate.snapshot.device.to_le_bytes());
        hasher.update(&candidate.snapshot.inode.to_le_bytes());
        hasher.update(&candidate.snapshot.size.to_le_bytes());
        hasher.update(candidate.snapshot.digest.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn stale_plan_error() -> ConfigError {
    ConfigError::ReadFailed(
        "theme files changed after the migration notice was shown; no stale file was replaced"
            .to_string(),
    )
}

fn migration_error(error: &io::Error) -> ConfigError {
    ConfigError::ReadFailed(format!("failed to apply the approved stock theme: {error}"))
}

fn marker_error(error: &io::Error) -> ConfigError {
    ConfigError::ReadFailed(format!(
        "failed to remember the Keep Current theme choice: {error}"
    ))
}
