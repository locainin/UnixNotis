//! Worker execution and guarded installation lifecycle

use anyhow::{Context, Result};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

use crate::actions::{
    commit_pending_release, ensure_selected_service_inactive, run_step_with_reservation,
    start_service_and_verify, stop_active_daemon, ActionContext, DaemonActivationReservation,
    StepKind,
};
use crate::app::events::{UiMessage, WorkerEvent};
use crate::app::workflow::recovery::{
    hold_activation_inhibition, recover_install_failure, send_recovery_required,
    send_worker_failure, InstallFailureRecovery,
};
use crate::model::ActionMode;
use crate::paths::InstallPaths;

pub const fn action_requires_install_state(mode: ActionMode) -> bool {
    matches!(mode, ActionMode::Install)
}

pub struct InstallLifecycle {
    // This guard lives across StopDaemon, binary publication, and service preparation
    pub(super) activation: Option<DaemonActivationReservation>,
    // A committed binary step leaves a reversible pending generation behind
    pub(super) release_pending: bool,
}

impl InstallLifecycle {
    pub(super) const fn new() -> Self {
        Self {
            activation: None,
            release_pending: false,
        }
    }
}

pub fn run_action_worker(
    plan: &[StepKind],
    mode: ActionMode,
    paths: &InstallPaths,
    install_state: Option<&crate::actions::InstallState>,
    restore_backup: Option<&std::path::Path>,
    ui_tx: &mpsc::SyncSender<UiMessage>,
) {
    // Run plan steps on the worker thread and stream progress events to the UI
    // The flag lives across steps so install can decide later whether reload is needed
    let service_reload_required = Arc::new(AtomicBool::new(true));
    let mut lifecycle = InstallLifecycle::new();
    for (index, step) in plan.iter().enumerate() {
        // Index maps to app.steps in the UI state
        let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::StepStarted(index)));

        // Build per-step context; clone install_state to avoid borrow issues
        let mut ctx = ActionContext {
            paths,
            install_state: install_state.cloned(),
            log_tx: ui_tx.clone(),
            action_mode: mode,
            restore_backup: restore_backup.map(std::path::Path::to_path_buf),
            service_reload_required: service_reload_required.clone(),
        };
        let result = if mode == ActionMode::Install {
            run_install_step(*step, &mut ctx, &mut lifecycle)
        } else {
            run_step_with_reservation(*step, &mut ctx, None)
        };

        match result {
            Ok(()) => {
                lifecycle.release_pending =
                    release_pending_after_completed_step(lifecycle.release_pending, *step);
                // Successful steps advance the progress list in order
                let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::StepCompleted(index)));
            }
            Err(err) => match recover_install_failure(&mut ctx, &mut lifecycle, err) {
                InstallFailureRecovery::Recovered(err) => {
                    send_worker_failure(ui_tx, index, &err);
                    // Stop the worker after the first failed step so later steps cannot compound damage
                    let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::Finished));
                    return;
                }
                InstallFailureRecovery::ActivationInhibited(err) => {
                    send_recovery_required(ui_tx, index, &err);
                    // The worker and its installer lock remain alive while this guard is held
                    hold_activation_inhibition(lifecycle);
                }
            },
        }
    }

    let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::Finished));
}

pub fn run_install_step(
    step: StepKind,
    ctx: &mut ActionContext,
    lifecycle: &mut InstallLifecycle,
) -> Result<()> {
    match step {
        StepKind::StopDaemon => {
            stop_active_daemon(ctx)?;
            let reservation = DaemonActivationReservation::acquire()
                .context("reserve daemon activation after shutdown")?;
            ensure_selected_service_inactive(ctx.paths)
                .context("recheck selected service after activation reservation")?;
            lifecycle.activation = Some(reservation);
            Ok(())
        }
        StepKind::EnableService => {
            {
                let reservation = lifecycle
                    .activation
                    .as_ref()
                    .context("service start requires daemon activation reservation")?;
                crate::actions::prepare_service_start_under_reservation(ctx, reservation)?;
                ensure_selected_service_inactive(ctx.paths)
                    .context("verify service remains inactive after artifact refresh")?;
            }

            // The next operation is the intentional handoff to the new daemon
            let reservation = lifecycle
                .activation
                .take()
                .context("missing activation reservation before controlled service start")?;
            drop(reservation);
            start_service_and_verify(ctx, crate::actions::enforce_service_readiness)?;
            commit_pending_release(ctx.paths).context("commit ready binary release generation")?;
            Ok(())
        }
        _ => run_step_with_reservation(step, ctx, lifecycle.activation.as_ref()),
    }
}

pub(super) const fn release_pending_after_completed_step(current: bool, step: StepKind) -> bool {
    match step {
        // Binary activation stays reversible until the matching service passes readiness
        StepKind::InstallBinaries => true,
        StepKind::EnableService => false,
        _ => current,
    }
}
