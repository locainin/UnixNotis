//! Backup snapshot helpers for config and theme files

use std::path::PathBuf;

use unixnotis_core::Config;

use super::listing::list_backup_dirs;

pub fn list_backup_dirs_for_ui() -> Vec<PathBuf> {
    // The restore screen remains usable when default path discovery fails
    let Ok(config_dir) = Config::default_config_dir() else {
        return Vec::new();
    };

    // Stable ordering keeps keyboard selection and redraws predictable
    let mut backups = list_backup_dirs(&config_dir);
    backups.sort();
    backups
}
