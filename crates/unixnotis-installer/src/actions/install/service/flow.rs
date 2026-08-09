//! Service installation, enablement, and uninstall flow

use std::sync::atomic::Ordering;

use anyhow::{Context, Result};

use crate::actions::DaemonActivationReservation;
use crate::paths::format_with_home;

use super::super::super::{
    ensure_shell_path_entry,
    hyprland::{ensure_hyprland_autostart, remove_hyprland_autostart},
    log_line, remove_shell_path_entry, sync_user_environment, ActionContext,
};

use super::artifacts::{
    remove_service_artifact, service_artifact_path_conflicts, service_artifact_path_exists,
    write_service_artifacts, ServiceArtifactWrite,
};
use super::lifecycle::{
    remove_pre_start_artifacts, run_command_spec, run_service_start, warn_pre_start_artifacts_left,
};
use super::refresh::refresh_service_artifacts;

pub(in crate::actions::install) fn install_service(ctx: &mut ActionContext) -> Result<()> {
    install_service_impl(ctx)
}

pub fn install_service_under_reservation(
    ctx: &mut ActionContext,
    _reservation: &DaemonActivationReservation,
) -> Result<()> {
    // The reservation is held by the worker while service artifacts are replaced
    install_service(ctx)
}

fn install_service_impl(ctx: &mut ActionContext) -> Result<()> {
    match write_service_artifacts(ctx)? {
        ServiceArtifactWrite::CreatedOrUpdated => {
            log_line(
                ctx,
                format!(
                    "Installed {} at {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(&ctx.paths.service.primary_artifact_path())
                ),
            );
        }
        ServiceArtifactWrite::Unchanged => {
            log_line(
                ctx,
                format!("{} already up to date", ctx.paths.service.artifact_label()),
            );
        }
    }

    Ok(())
}

pub(in crate::actions) fn prepare_service_start(ctx: &mut ActionContext) -> Result<()> {
    if ctx.service_reload_required.load(Ordering::Acquire) {
        // Refresh work can be a single reload command or a backend-owned database update
        refresh_service_artifacts(ctx)?;
    } else {
        log_line(
            ctx,
            format!(
                "Skipping {} refresh because {} is unchanged",
                ctx.paths.service.manager_label(),
                ctx.paths.service.artifact_label()
            ),
        );
    }

    // Import the live session env first so the first daemon start picks it up
    if let Err(err) = sync_user_environment(ctx) {
        warn_pre_start_artifacts_left(ctx);
        return Err(err);
    }
    remove_pre_start_artifacts(ctx)?;
    Ok(())
}

pub fn prepare_service_start_under_reservation(
    ctx: &mut ActionContext,
    _reservation: &DaemonActivationReservation,
) -> Result<()> {
    // Manager refresh and pre-start cleanup remain inside the activation exclusion
    prepare_service_start(ctx)
}

pub fn start_service_and_verify<F>(ctx: &mut ActionContext, readiness: F) -> Result<()>
where
    F: Fn(&mut ActionContext) -> Result<()>,
{
    run_service_start(ctx)?;
    readiness(ctx)?;

    // Shell startup files are updated so new terminals can resolve the installed commands
    if let Err(err) = ensure_shell_path_entry(ctx) {
        log_line(
            ctx,
            format!("Warning: failed to update shell PATH files ({err})"),
        );
    }

    // Hyprland gets one managed exec-once block so session env sync happens once per login
    ensure_hyprland_autostart(ctx);
    Ok(())
}

pub fn rollback_failed_activation<F>(
    ctx: &mut ActionContext,
    readiness: &F,
    activation_error: anyhow::Error,
) -> Result<()>
where
    F: Fn(&mut ActionContext) -> Result<()>,
{
    rollback_failed_activation_with_quiescence(ctx, readiness, activation_error, |paths| {
        crate::actions::daemon::wait_until_no_conflicting_live_daemon(
            paths,
            crate::actions::daemon::STOP_QUIESCENCE_TIMEOUT,
        )
    })
}

pub(in crate::actions::install) fn rollback_failed_activation_with_quiescence<F, Q>(
    ctx: &mut ActionContext,
    readiness: &F,
    activation_error: anyhow::Error,
    mut wait_for_quiescence: Q,
) -> Result<()>
where
    F: Fn(&mut ActionContext) -> Result<()>,
    Q: FnMut(&crate::paths::InstallPaths) -> Result<()>,
{
    if !crate::actions::releases::pending_release_exists(ctx.paths)? {
        return Err(activation_error);
    }
    let restart_previous =
        crate::actions::releases::pending_release_has_runtime_rollback(ctx.paths)?;
    // Disk generation must not move backward while the failed new daemon is still live
    let stop = ctx.paths.service.stop_for_reinstall_command();
    let stop_result = run_command_spec(ctx, &stop);
    let quiescence_result = wait_for_quiescence(ctx.paths);
    match (stop_result, quiescence_result) {
        (Ok(()), Ok(())) => {}
        (Err(stop_error), Ok(())) => {
            // Live state is authoritative when the manager command reports a stale failure
            log_line(
                ctx,
                format!(
                    "Warning: rejected release stop command failed after runtime became quiescent ({stop_error:#})"
                ),
            );
        }
        (Ok(()), Err(state_error)) => {
            return Err(activation_error.context(format!(
                "service manager reported a successful stop but the rejected runtime remains live: {state_error:#}"
            )));
        }
        (Err(stop_error), Err(state_error)) => {
            return Err(activation_error.context(format!(
                "failed to stop the rejected release before rollback: {stop_error:#}; runtime remains live or indeterminate: {state_error:#}"
            )));
        }
    }
    // Current may move backward only after both broker and manager state prove quiescence
    crate::actions::releases::rollback_pending_release(ctx.paths)
        .context("roll back rejected binary release generation")?;
    if restart_previous {
        run_service_start(ctx).context("restart previous release generation")?;
        readiness(ctx).context("previous release did not recover after rollback")?;
    }
    Err(activation_error)
}

pub fn rollback_pending_under_activation_reservation(
    ctx: &mut ActionContext,
    _reservation: &DaemonActivationReservation,
) -> Result<bool> {
    let restart_previous =
        crate::actions::releases::pending_release_has_runtime_rollback(ctx.paths)?;
    // Direct service-manager starts remain possible while the D-Bus names are reserved
    let stop = ctx.paths.service.stop_for_reinstall_command();
    let stop_result = run_command_spec(ctx, &stop);
    let service_quiescence_result = crate::actions::daemon::wait_until_selected_service_inactive(
        ctx.paths,
        crate::actions::daemon::STOP_QUIESCENCE_TIMEOUT,
    );
    match (stop_result, service_quiescence_result) {
        (Ok(()), Ok(())) => {}
        (Err(stop_error), Ok(())) => {
            log_line(
                ctx,
                format!(
                    "Warning: rejected release stop command failed after service became inactive ({stop_error:#})"
                ),
            );
        }
        (Ok(()), Err(state_error)) => {
            return Err(
                state_error.context("service remained active after the guarded release failure")
            );
        }
        (Err(stop_error), Err(state_error)) => {
            return Err(state_error.context(format!(
                "failed to stop the rejected release while activation remained reserved ({stop_error:#})"
            )));
        }
    }

    crate::actions::releases::rollback_pending_release(ctx.paths)
        .context("roll back rejected binary release generation while activation is reserved")?;
    Ok(restart_previous)
}

pub fn restart_previous_service<F>(ctx: &mut ActionContext, readiness: &F) -> Result<()>
where
    F: Fn(&mut ActionContext) -> Result<()>,
{
    run_service_start(ctx).context("restart previous release generation")?;
    readiness(ctx).context("previous release did not recover after rollback")
}

pub fn uninstall_service(ctx: &mut ActionContext) -> Result<()> {
    let artifacts = ctx.paths.service.install_artifacts(&ctx.paths.bin_dir);
    let artifact_exists = artifacts.iter().any(service_artifact_path_exists);
    let unsafe_artifact_exists = log_unsafe_service_artifacts(ctx, &artifacts);

    if artifact_exists {
        let spec = ctx.paths.service.disable_now_command();
        if let Err(err) = run_command_spec(ctx, &spec) {
            log_line(ctx, format!("Warning: {err}"));
        }

        for artifact in artifacts.iter().rev() {
            if service_artifact_path_conflicts(artifact) {
                // Unsafe paths were already logged and must not be passed into removers
                continue;
            }

            let artifact_removed = remove_service_artifact(artifact).with_context(|| {
                format!(
                    "failed to remove {} at {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(&artifact.path)
                )
            })?;

            if artifact_removed {
                log_line(
                    ctx,
                    format!(
                        "Removed {} at {}",
                        ctx.paths.service.artifact_label(),
                        format_with_home(&artifact.path)
                    ),
                );
            }
        }
        refresh_service_artifacts(ctx)?;
    } else if !unsafe_artifact_exists {
        log_line(
            ctx,
            format!(
                "{} not found at {}",
                ctx.paths.service.artifact_label(),
                format_with_home(&ctx.paths.service.primary_artifact_path())
            ),
        );
    }

    // Remove any Hyprland autostart entry managed by the installer before
    // cleaning up shell startup files
    remove_hyprland_autostart(ctx);

    // Shell PATH cleanup is non-fatal: uninstall should continue even if one
    // startup file cannot be read or updated
    if let Err(err) = remove_shell_path_entry(ctx) {
        log_line(
            ctx,
            format!("Warning: failed to remove shell PATH entries ({err})"),
        );
    }

    Ok(())
}

fn log_unsafe_service_artifacts(
    ctx: &mut ActionContext,
    artifacts: &[crate::service_manager::ServiceArtifact],
) -> bool {
    // Track whether any unsafe artifact path was detected so the caller can
    // decide whether cleanup should proceed
    let mut found = false;

    for artifact in artifacts {
        // Only warn about service artifacts whose paths are considered unsafe
        // to remove automatically
        if service_artifact_path_conflicts(artifact) {
            found = true;

            // Log the unsafe path in a user-friendly form and leave the file in
            // place rather than risking deletion of something not owned by us
            log_line(
                ctx,
                format!(
                    "Warning: unsafe service artifact path exists at {}; refusing to remove it automatically",
                    format_with_home(&artifact.path)
                ),
            );
        }
    }

    found
}
