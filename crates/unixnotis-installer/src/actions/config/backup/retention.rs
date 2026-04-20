//! Backup directory creation and retention policy helpers

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;

use crate::paths::format_with_home;

use super::super::super::{log_line, ActionContext};

pub(in crate::actions::config::backup) const BACKUP_PREFIX: &str = "Backup-";

pub(in crate::actions::config) fn create_backup_dir(
    ctx: &mut ActionContext,
    config_dir: &Path,
    keep: usize,
) -> Result<Option<PathBuf>> {
    if keep == 0 {
        log_line(ctx, "Backups disabled (installer.toml keep = 0)");
        return Ok(None);
    }

    // Each reset gets its own dated directory so filenames stay simple
    let stamp = backup_stamp_from_system_time()?;
    let base_name = format!("{BACKUP_PREFIX}{stamp}");
    let mut candidate = config_dir.join(base_name);

    // If a backup already exists for that day, add a zero-padded suffix
    let mut suffix = 1;
    while candidate.exists() {
        candidate = config_dir.join(format!("{BACKUP_PREFIX}{stamp}-{suffix:03}"));
        suffix += 1;
    }

    fs::create_dir_all(&candidate).with_context(|| "failed to create backup directory")?;
    log_line(
        ctx,
        format!("Backup directory created: {}", format_with_home(&candidate)),
    );

    prune_old_backups(ctx, config_dir, keep)?;
    Ok(Some(candidate))
}

pub(in crate::actions::config::backup) fn list_backup_dirs(config_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(config_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(BACKUP_PREFIX) {
                return None;
            }
            Some(entry.path())
        })
        .collect()
}

pub(in crate::actions::config::backup) fn prune_old_backups(
    ctx: &mut ActionContext,
    config_dir: &Path,
    keep: usize,
) -> Result<()> {
    if keep == 0 {
        return Ok(());
    }

    let mut backups = list_backup_dirs(config_dir);
    // YYYY-MM-DD names and zero-padded suffixes sort in age order
    backups.sort();

    if backups.len() <= keep {
        return Ok(());
    }

    let excess = backups.len().saturating_sub(keep);
    for path in backups.into_iter().take(excess) {
        if let Err(err) = fs::remove_dir_all(&path) {
            log_line(
                ctx,
                format!(
                    "Warning: failed to remove old backup {}: {}",
                    format_with_home(&path),
                    err
                ),
            );
        } else {
            log_line(
                ctx,
                format!("Removed old backup {}", format_with_home(&path)),
            );
        }
    }

    Ok(())
}

fn backup_stamp_from_system_time() -> Result<String> {
    // Use chrono for a stable YYYY-MM-DD stamp without hand-rolled time math
    Ok(Local::now().format("%Y-%m-%d").to_string())
}
