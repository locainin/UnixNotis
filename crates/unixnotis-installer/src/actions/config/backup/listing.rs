//! Backup-directory listing for the installer restore view

use std::fs;
use std::path::{Path, PathBuf};

pub(in crate::actions::config::backup) const BACKUP_PREFIX: &str = "Backup-";

pub(in crate::actions::config::backup) fn list_backup_dirs(config_dir: &Path) -> Vec<PathBuf> {
    // A missing config directory simply means there is nothing to restore
    let Ok(entries) = fs::read_dir(config_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            // Restore only real directories so backup-like files cannot enter the picker
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // The prefix keeps unrelated user directories out of the restore list
            if !name.starts_with(BACKUP_PREFIX) {
                return None;
            }
            Some(entry.path())
        })
        .collect()
}
