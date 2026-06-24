use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::{run_command_without_stdout, ActionContext};
use crate::detect::Detection;
use crate::events::{UiMessage, WorkerEvent};
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManagerPaths;

#[test]
fn env_sync_command_stdout_is_not_copied_into_logs() {
    let (tx, rx) = mpsc::sync_channel::<UiMessage>(16);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: std::env::temp_dir(),
        bin_dir: std::env::temp_dir().join("bin"),
        service: ServiceManagerPaths::systemd_user(std::env::temp_dir().join("systemd")),
    };
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let mut command = Command::new("sh");
    command.args([
        "-c",
        // Stdout stands in for environment dumps that should not reach the TUI logs
        "printf 'SECRET_ENV=value\\n'; printf 'visible error\\n' >&2",
    ]);

    run_command_without_stdout(&mut ctx, "test env sync", command, None)
        .expect("command should succeed");

    let logs = rx
        .try_iter()
        .filter_map(|message| match message {
            UiMessage::Worker(WorkerEvent::LogLine(line)) => Some(line),
            UiMessage::Input(_) | UiMessage::Worker(_) => None,
        })
        .collect::<Vec<_>>();

    assert!(
        logs.iter().all(|line| !line.contains("SECRET_ENV")),
        "stdout should not be logged: {logs:?}"
    );
    assert!(
        // Stderr remains visible so failed sync commands still have useful diagnostics
        logs.iter().any(|line| line.contains("visible error")),
        "stderr should stay visible: {logs:?}"
    );
}
