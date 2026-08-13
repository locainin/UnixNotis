use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

use super::super::worker::InstallLifecycle;
use crate::actions::DaemonActivationReservation;
use crate::app::{App, ProgressState};
use crate::model::{ActionStep, StepStatus};
use anyhow::{Context, Result};

pub(super) fn app_with_steps() -> App {
    let _lock = crate::test_support::env::test_env_lock();
    let mut app = App::new(None);
    app.steps = vec![
        ActionStep {
            name: "first",
            status: StepStatus::Pending,
        },
        ActionStep {
            name: "second",
            status: StepStatus::Pending,
        },
    ];
    app.progress_state = ProgressState::Running;
    app
}

pub(super) fn recovery_context(
    paths: &crate::paths::InstallPaths,
) -> crate::actions::ActionContext<'_> {
    let (tx, _rx) = mpsc::sync_channel(8);
    crate::actions::ActionContext {
        paths,
        install_state: None,
        log_tx: tx,
        action_mode: crate::model::ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
}

pub(super) fn recovery_paths(root: &std::path::Path) -> crate::paths::InstallPaths {
    std::fs::create_dir_all(root).expect("create recovery fixture");
    crate::paths::InstallPaths {
        repo_root: root.to_path_buf(),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: crate::service_manager::ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    }
}

pub(super) fn guarded_lifecycle(alive: &Arc<AtomicBool>) -> InstallLifecycle {
    InstallLifecycle {
        activation: Some(crate::actions::DaemonActivationReservation::test_guard(
            Arc::clone(alive),
        )),
        release_pending: false,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the test seam names each lifecycle boundary explicitly"
)]
pub(super) fn run_install_lifecycle_with_hooks<
    Stop,
    Acquire,
    Check,
    Binary,
    Service,
    Prepare,
    Start,
>(
    lifecycle: &mut InstallLifecycle,
    stop: Stop,
    acquire: Acquire,
    mut check_after_guard: Check,
    install_binaries: Binary,
    install_service: Service,
    prepare_service: Prepare,
    start: Start,
) -> Result<()>
where
    Stop: FnOnce() -> Result<()>,
    Acquire: FnOnce() -> Result<DaemonActivationReservation>,
    Check: FnMut(&DaemonActivationReservation) -> Result<()>,
    Binary: FnOnce(&DaemonActivationReservation) -> Result<()>,
    Service: FnOnce(&DaemonActivationReservation) -> Result<()>,
    Prepare: FnOnce(&DaemonActivationReservation) -> Result<()>,
    Start: FnOnce() -> Result<()>,
{
    stop()?;
    lifecycle.activation = Some(acquire()?);

    let reservation = lifecycle
        .activation
        .as_ref()
        .context("test lifecycle lost activation reservation")?;
    check_after_guard(reservation)?;
    install_binaries(reservation)?;
    install_service(reservation)?;
    prepare_service(reservation)?;
    check_after_guard(reservation)?;

    // The controlled start is the only point where the names may be released
    drop(
        lifecycle
            .activation
            .take()
            .context("test lifecycle missing activation handoff")?,
    );
    start()
}
