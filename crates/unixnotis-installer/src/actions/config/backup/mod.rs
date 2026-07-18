//! Config backup entry points

mod restore;
mod retention;
mod settings;
mod snapshot;
mod write;

// Keep config reads separate from dated backup directory churn
pub(in crate::actions::config) use settings::{ensure_installer_config, load_installer_config};
// Backup file copies stay separate from restore logic so reset paths stay easy to scan
pub(in crate::actions::config) use retention::create_backup_dir;
pub(in crate::actions::config) use snapshot::backup_existing_file;

pub use restore::restore_config;
pub use snapshot::list_backup_dirs_for_ui;
pub use write::write_atomic;

#[cfg(test)]
mod tests;
