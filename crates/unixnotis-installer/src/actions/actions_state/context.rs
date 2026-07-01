//! Shared action execution context

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc::SyncSender, Arc};

use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;

use super::install_state::InstallState;

pub struct ActionContext<'a> {
    // Read-only compatibility snapshot collected before the action begins
    pub detection: &'a Detection,
    // All filesystem and service-manager paths for the selected backend
    pub paths: &'a InstallPaths,
    // Cached install state keeps the progress view aligned with the selected action
    pub install_state: Option<InstallState>,
    // Bounded UI channel keeps background work from flooding the terminal renderer
    pub log_tx: SyncSender<UiMessage>,
    // Action mode decides whether shared steps are running install, reinstall, or uninstall
    pub action_mode: ActionMode,
    // Reset-config flows keep a backup path so failures can roll back user files
    pub restore_backup: Option<PathBuf>,
    // Tracks whether install changed the service artifact so later steps can skip reloads
    // A skipped reload keeps reinstall cheap when the on-disk artifact already matched
    pub service_reload_required: Arc<AtomicBool>,
}
