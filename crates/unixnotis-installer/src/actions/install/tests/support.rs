use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManagerPaths;

use super::super::super::ActionContext;

pub(super) fn test_root(name: &str) -> std::path::PathBuf {
    // Unique temp roots keep parallel tests from stepping on the same fake workspace
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "unixnotis-installer-{name}-{}-{stamp}",
        std::process::id()
    ))
}

pub(super) fn test_paths(root: &std::path::Path) -> InstallPaths {
    let service_artifact_dir = root
        .join("home")
        .join(".config")
        .join("systemd")
        .join("user");
    InstallPaths {
        repo_root: root.to_path_buf(),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: ServiceManagerPaths::systemd_user(service_artifact_dir),
    }
}

pub(super) fn write_fake_workspace(root: &std::path::Path, binaries: &[&str]) {
    // Cargo metadata only needs a valid workspace root to report the target directory
    fs::create_dir_all(root).expect("make fake workspace");
    let quoted = binaries
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let cargo_toml = format!(
        "[workspace]\nmembers = []\n\n[workspace.metadata.unixnotis.installer]\nbinaries = [{quoted}]\n"
    );
    fs::write(root.join("Cargo.toml"), cargo_toml).expect("write fake Cargo.toml");
}

pub(super) fn test_context<'a>(
    detection: &'a Detection,
    paths: &'a InstallPaths,
    action_mode: ActionMode,
) -> ActionContext<'a> {
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(32);
    ActionContext {
        detection,
        paths,
        install_state: None,
        log_tx: tx,
        action_mode,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
}
