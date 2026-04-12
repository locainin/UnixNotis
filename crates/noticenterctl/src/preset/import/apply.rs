//! Import apply helpers for writing a preset plan into the live config tree
//!
//! This module keeps the high-risk write path together:
//! secure dir-fd writes, root drift checks, rollback, and the final backup commit

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::super::filesystem::ensure_dir_fd_matches_live_path;
use super::super::filesystem::{
    create_backup_dir_secure, open_secure_dir_all, read_relative_file_secure,
    remove_empty_relative_dirs_secure, remove_relative_dir_secure, remove_relative_file_secure,
    write_relative_file_atomic_secure,
};
use super::plan::ImportPlan;

pub(super) struct ImportTransaction {
    // The visible config root path is kept so later checks can spot root swaps
    config_dir: PathBuf,
    // All import writes stay pinned to this one opened directory until commit or rollback
    config_root_fd: std::os::fd::OwnedFd,
    // The captured pre-import file state drives both rollback and the final backup snapshot
    applied_items: Vec<AppliedImportItem>,
}

#[derive(Debug)]
struct AppliedImportItem {
    // Relative path written through the secure config-root fd
    relative_path: PathBuf,
    // Overwrite cases keep the old bytes so rollback can put them back exactly
    previous_contents: Option<Vec<u8>>,
    // Original mode is restored with the old file contents during rollback
    previous_mode: Option<u32>,
}

pub(super) fn apply_import_plan(config_dir: &Path, plan: &ImportPlan) -> Result<ImportTransaction> {
    // Real import pins the visible config root to one open directory fd before any write starts
    let config_root_fd = open_secure_dir_all(config_dir)
        .with_context(|| format!("open secure config directory {}", config_dir.display()))?;
    let mut applied_items = Vec::new();

    for item in &plan.items {
        // If the live root moved, stop and undo anything already applied through the old fd
        ensure_live_config_root_or_rollback(&config_root_fd, config_dir, &applied_items)?;

        let mut previous_contents = None;
        let mut previous_mode = None;

        if item.overwrite_existing {
            // Existing bytes are kept in memory until commit so rollback can restore them exactly
            let (existing_bytes, existing_mode) =
                read_relative_file_secure(&config_root_fd, &item.file.relative_path).with_context(
                    || {
                        format!(
                            "read existing imported file {}",
                            item.file.relative_path.display()
                        )
                    },
                )?;
            previous_contents = Some(existing_bytes);
            previous_mode = Some(existing_mode);
        }

        // The final payload write stays beneath the open config-root fd even if the path is raced
        write_relative_file_atomic_secure(
            &config_root_fd,
            &item.file.relative_path,
            &item.file.contents,
            item.file.mode,
        )
        .with_context(|| format!("write imported file {}", item.target_path.display()))?;

        applied_items.push(AppliedImportItem {
            relative_path: item.file.relative_path.clone(),
            previous_contents,
            previous_mode,
        });
    }

    // One last root check closes the window between the final write and the staged return
    ensure_live_config_root_or_rollback(&config_root_fd, config_dir, &applied_items)?;
    Ok(ImportTransaction {
        config_dir: config_dir.to_path_buf(),
        config_root_fd,
        applied_items,
    })
}

pub(super) fn finalize_import_transaction(
    transaction: ImportTransaction,
) -> Result<Option<PathBuf>> {
    // Commit only happens if the live root still points at the same directory after post-checks
    ensure_import_root_matches_live_path(&transaction)?;

    let overwritten_items = transaction
        .applied_items
        .iter()
        .filter(|item| item.previous_contents.is_some())
        .count();
    if overwritten_items == 0 {
        return Ok(None);
    }

    // The user-visible backup is created under the same secure root as the import transaction
    let (backup_relative_dir, backup_root_fd) =
        create_backup_dir_secure(&transaction.config_root_fd)
            .context("create secure backup snapshot")?;
    let mut written_backup_paths = Vec::new();

    for item in &transaction.applied_items {
        let (Some(previous_contents), Some(previous_mode)) =
            (item.previous_contents.as_ref(), item.previous_mode)
        else {
            continue;
        };

        // Backup bytes come from the captured pre-import state, not from the live tree after apply
        if let Err(err) = write_relative_file_atomic_secure(
            &backup_root_fd,
            &item.relative_path,
            previous_contents,
            previous_mode,
        )
        .with_context(|| {
            format!(
                "write backup file {}",
                transaction
                    .config_dir
                    .join(&backup_relative_dir)
                    .join(&item.relative_path)
                    .display()
            )
        }) {
            cleanup_backup_snapshot(
                &transaction.config_root_fd,
                &backup_relative_dir,
                &backup_root_fd,
                &written_backup_paths,
            )?;
            rollback_applied_import_items(&transaction.config_root_fd, &transaction.applied_items)?;
            return Err(err);
        }

        written_backup_paths.push(item.relative_path.clone());
    }

    // One last root check catches a root swap that lands during the backup commit itself
    if let Err(err) = ensure_import_root_matches_live_path(&transaction) {
        cleanup_backup_snapshot(
            &transaction.config_root_fd,
            &backup_relative_dir,
            &backup_root_fd,
            &written_backup_paths,
        )?;
        rollback_applied_import_items(&transaction.config_root_fd, &transaction.applied_items)?;
        return Err(err);
    }

    Ok(Some(transaction.config_dir.join(&backup_relative_dir)))
}

pub(super) fn rollback_import_transaction(transaction: ImportTransaction) -> Result<()> {
    rollback_applied_import_items(&transaction.config_root_fd, &transaction.applied_items)
}

fn ensure_import_root_matches_live_path(transaction: &ImportTransaction) -> Result<()> {
    ensure_dir_fd_matches_live_path(&transaction.config_root_fd, &transaction.config_dir)
}

fn ensure_live_config_root_or_rollback(
    config_root_fd: &std::os::fd::OwnedFd,
    config_dir: &Path,
    applied_items: &[AppliedImportItem],
) -> Result<()> {
    if let Err(err) = ensure_dir_fd_matches_live_path(config_root_fd, config_dir) {
        // Once the visible root drifts, every earlier write is treated as suspect and unwound
        rollback_applied_import_items(config_root_fd, applied_items)?;
        return Err(err);
    }
    Ok(())
}

fn rollback_applied_import_items(
    config_root_fd: &std::os::fd::OwnedFd,
    applied_items: &[AppliedImportItem],
) -> Result<()> {
    for item in applied_items.iter().rev() {
        if let (Some(previous_contents), Some(previous_mode)) =
            (item.previous_contents.as_ref(), item.previous_mode)
        {
            // Overwrites are restored byte-for-byte so failed imports do not leave drift behind
            write_relative_file_atomic_secure(
                config_root_fd,
                &item.relative_path,
                previous_contents,
                previous_mode,
            )
            .with_context(|| format!("restore imported file {}", item.relative_path.display()))?;
        } else {
            // Brand-new files can simply be removed when rollback unwinds the apply loop
            remove_relative_file_secure(config_root_fd, &item.relative_path).with_context(
                || format!("remove imported file {}", item.relative_path.display()),
            )?;
        }
    }

    Ok(())
}

fn cleanup_backup_snapshot(
    config_root_fd: &std::os::fd::OwnedFd,
    backup_relative_dir: &Path,
    backup_root_fd: &std::os::fd::OwnedFd,
    written_backup_paths: &[PathBuf],
) -> Result<()> {
    for relative_path in written_backup_paths.iter().rev() {
        // Files are removed first so parent directory cleanup can collapse empty branches afterward
        remove_relative_file_secure(backup_root_fd, relative_path)
            .with_context(|| format!("remove backup file {}", relative_path.display()))?;
        remove_empty_relative_dirs_secure(backup_root_fd, relative_path)?;
    }

    // The snapshot root itself is removed last once every nested path has been cleaned
    remove_relative_dir_secure(config_root_fd, backup_relative_dir)
        .with_context(|| format!("remove backup directory {}", backup_relative_dir.display()))?;
    Ok(())
}
