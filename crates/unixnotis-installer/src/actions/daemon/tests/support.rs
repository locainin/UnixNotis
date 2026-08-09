use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::detect::{DetectedDaemon, Detection, OwnerInfo};
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

pub(super) fn known_daemon_detection(
    name: &str,
    systemd_active: bool,
    running_pids: Vec<u32>,
) -> Detection {
    Detection {
        owner: Some(OwnerInfo {
            unique_name: None,
            pid: Some(42),
            comm: Some(name.to_string()),
        }),
        daemons: vec![DetectedDaemon {
            name: name.to_string(),
            unit: format!("{name}.service"),
            systemd_active,
            systemd_error: None,
            running_pids,
            is_owner: true,
        }],
    }
}

pub(super) fn test_install_paths() -> InstallPaths {
    InstallPaths {
        repo_root: std::env::temp_dir(),
        bin_dir: std::env::temp_dir(),
        service: ServiceManager::systemd_user(std::env::temp_dir()),
    }
}

pub(super) fn action_context(
    paths: &InstallPaths,
    log_tx: mpsc::SyncSender<UiMessage>,
) -> ActionContext<'_> {
    ActionContext {
        paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
}

pub(super) fn fake_daemon_tool_root(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-daemon-{label}-{}-{stamp}",
        std::process::id()
    ));
    let _cleanup = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("fake daemon tool bin");
    root
}
