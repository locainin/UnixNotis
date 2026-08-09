//! Config backup entry points

mod listing;
mod restore;
mod restore_transaction;
mod settings;
mod snapshot;

// Keep config reads separate from dated backup directory churn
pub(in crate::actions::config) use settings::{ensure_installer_config, load_installer_config};
// Backup file copies stay separate from restore logic so reset paths stay easy to scan

pub use restore::restore_config;
pub use snapshot::list_backup_dirs_for_ui;

#[cfg(test)]
mod tests;
