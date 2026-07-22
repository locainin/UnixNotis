//! Backup snapshot helpers for config and theme files

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::filesystem::copy_file_atomic;
use unixnotis_core::Config;

use crate::paths::format_with_home;

use super::super::super::{log_line, ActionContext};
use super::retention::list_backup_dirs;

pub(in crate::actions::config) fn backup_existing_file(
    ctx: &mut ActionContext,
    path: &Path,
    label: &str,
    backup_dir: Option<&Path>,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let Some(backup_dir) = backup_dir else {
        return Ok(());
    };

    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let backup_path = backup_dir.join(file_name.as_ref());

    // Open the live file once and publish its snapshot without following either path through links
    copy_file_atomic(path, &backup_path).with_context(|| format!("failed to backup {label}"))?;
    log_line(
        ctx,
        format!("Backed up {} to {}", label, format_with_home(&backup_path)),
    );
    Ok(())
}

pub fn list_backup_dirs_for_ui() -> Vec<PathBuf> {
    let Ok(config_dir) = Config::default_config_dir() else {
        return Vec::new();
    };

    let mut backups = list_backup_dirs(&config_dir);
    backups.sort();
    backups
}
