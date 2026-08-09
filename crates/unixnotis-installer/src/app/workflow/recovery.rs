//! Failure classification and guarded rollback

use anyhow::Result;
use std::sync::mpsc;
use std::thread;

use crate::actions::{
    pending_release_exists, restart_previous_service, rollback_failed_activation,
    rollback_pending_under_activation_reservation, ActionContext, DaemonActivationReservation,
};
use crate::app::events::{UiMessage, WorkerEvent};
use crate::app::workflow::worker::InstallLifecycle;

pub enum InstallFailureRecovery {
    Recovered(anyhow::Error),
    ActivationInhibited(anyhow::Error),
}

pub fn send_worker_failure(ui_tx: &mpsc::SyncSender<UiMessage>, index: usize, err: &anyhow::Error) {
    let summary = err.to_string();
    let detail = format!("{err:#}");
    let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::StepFailed {
        index,
        summary,
        detail,
    }));
}

pub fn send_recovery_required(
    ui_tx: &mpsc::SyncSender<UiMessage>,
    index: usize,
    err: &anyhow::Error,
) {
    let summary = err.to_string();
    let detail = format!("{err:#}");
    let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::RecoveryRequired {
        index,
        summary,
        detail,
    }));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the owned lifecycle must remain alive while this thread is parked"
)]
pub fn hold_activation_inhibition(lifecycle: InstallLifecycle) -> ! {
    debug_assert!(
        lifecycle.activation.is_some(),
        "catastrophic recovery must retain the activation reservation"
    );

    // Keeping this stack frame alive keeps both the reservation and installer lock alive
    loop {
        thread::park();
    }
}

pub fn recover_install_failure(
    ctx: &mut ActionContext,
    lifecycle: &mut InstallLifecycle,
    activation_error: anyhow::Error,
) -> InstallFailureRecovery {
    if lifecycle.activation.is_none() {
        if lifecycle.release_pending {
            return match rollback_failed_activation(
                ctx,
                &crate::actions::enforce_service_readiness,
                activation_error,
            ) {
                Ok(()) => InstallFailureRecovery::Recovered(anyhow::anyhow!(
                    "failed install unexpectedly completed generation rollback without an error"
                )),
                Err(error) => InstallFailureRecovery::Recovered(error),
            };
        }

        return InstallFailureRecovery::Recovered(activation_error);
    }

    recover_guarded_failure_with_hooks(
        ctx,
        lifecycle,
        activation_error,
        pending_release_exists(ctx.paths),
        rollback_pending_under_activation_reservation,
        |ctx| restart_previous_service(ctx, &crate::actions::enforce_service_readiness),
    )
}

pub fn recover_guarded_failure_with_hooks<F, R>(
    ctx: &mut ActionContext,
    lifecycle: &mut InstallLifecycle,
    activation_error: anyhow::Error,
    pending: Result<bool>,
    guarded_rollback: F,
    restart_previous: R,
) -> InstallFailureRecovery
where
    F: FnOnce(&mut ActionContext, &DaemonActivationReservation) -> Result<bool>,
    R: FnOnce(&mut ActionContext) -> Result<()>,
{
    let pending = match pending {
        Ok(value) => value,
        Err(error) => {
            return InstallFailureRecovery::ActivationInhibited(activation_error.context(
                format!(
                    "could not determine pending release state; daemon activation remains inhibited: {error:#}"
                ),
            ));
        }
    };

    if !pending {
        if lifecycle.release_pending {
            return InstallFailureRecovery::ActivationInhibited(activation_error.context(
                "release state is inconsistent: worker expected a pending release but the recovery journal is missing; daemon activation remains inhibited",
            ));
        }

        lifecycle.activation.take();
        return InstallFailureRecovery::Recovered(activation_error);
    }

    let rollback_result = {
        let Some(reservation) = lifecycle.activation.as_ref() else {
            return InstallFailureRecovery::ActivationInhibited(
                activation_error
                    .context("activation reservation disappeared before guarded rollback"),
            );
        };
        guarded_rollback(ctx, reservation)
    };

    match rollback_result {
        Ok(restart) => {
            lifecycle.activation.take();

            if restart {
                if let Err(error) = restart_previous(ctx) {
                    return InstallFailureRecovery::Recovered(activation_error.context(format!(
                        "previous generation failed after rollback: {error:#}"
                    )));
                }
            }

            InstallFailureRecovery::Recovered(activation_error)
        }
        Err(rollback_error) => InstallFailureRecovery::ActivationInhibited(rollback_error.context(
            format!(
                "guarded rollback failed after the original install error: {activation_error:#}; daemon activation remains inhibited"
            ),
        )),
    }
}
