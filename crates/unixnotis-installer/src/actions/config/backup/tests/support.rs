use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

pub(super) fn test_paths(root: &std::path::Path) -> InstallPaths {
    InstallPaths {
        repo_root: root.to_path_buf(),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: ServiceManager::systemd_user(root.join("service")),
    }
}

pub(super) fn test_context<'a>(
    detection: &'a Detection,
    paths: &'a InstallPaths,
) -> ActionContext<'a> {
    let (log_tx, _log_rx) = mpsc::sync_channel::<UiMessage>(8);
    ActionContext {
        detection,
        paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
}
