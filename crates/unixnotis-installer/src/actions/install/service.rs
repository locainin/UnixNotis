//! Service artifact install and lifecycle helpers

use std::sync::atomic::Ordering;

use anyhow::{Context, Result};

use crate::paths::format_with_home;

use super::super::{
    ensure_shell_path_entry,
    hyprland::{ensure_hyprland_autostart, remove_hyprland_autostart},
    log_line, sync_user_environment, ActionContext,
};

mod artifacts;
mod dirs;
mod files;
mod lifecycle;
mod symlinks;

pub(in crate::actions::install) use artifacts::remove_service_artifact;
pub(crate) use artifacts::write_service_artifact;
#[cfg(test)]
pub(in crate::actions::install) use lifecycle::{
    service_start_mode_from_enabled, ServiceStartMode,
};

use artifacts::{service_artifact_path_exists, write_service_artifacts, ServiceArtifactWrite};
use lifecycle::{
    remove_pre_start_artifacts, run_command_spec, run_service_start, warn_pre_start_artifacts_left,
};

pub(crate) fn install_service(ctx: &mut ActionContext) -> Result<()> {
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

pub(crate) fn enable_service(ctx: &mut ActionContext) -> Result<()> {
    if ctx.service_reload_required.load(Ordering::Acquire) {
        // A full user-manager reload is expensive on some setups, so run it only when needed
        log_line(
            ctx,
            format!("Reloading {}", ctx.paths.service.manager_label()),
        );
        if let Some(spec) = ctx.paths.service.reload_after_artifact_change() {
            run_command_spec(ctx, &spec)?;
        }
    } else {
        log_line(
            ctx,
            format!(
                "Skipping {} reload because {} is unchanged",
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
    run_service_start(ctx)?;

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

pub(crate) fn uninstall_service(ctx: &mut ActionContext) -> Result<()> {
    let artifacts = ctx.paths.service.install_artifacts(&ctx.paths.bin_dir);
    let artifact_exists = artifacts.iter().any(service_artifact_path_exists);

    if artifact_exists {
        if let Some(spec) = ctx.paths.service.disable_now_command() {
            if let Err(err) = run_command_spec(ctx, &spec) {
                log_line(ctx, format!("Warning: {}", err));
            }
        } else {
            log_line(
                ctx,
                format!(
                    "Skipping disable; {} has no disable command",
                    ctx.paths.service.label()
                ),
            );
        }

        for artifact in artifacts.iter().rev() {
            remove_service_artifact(artifact).with_context(|| {
                format!(
                    "failed to remove {} at {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(&artifact.path)
                )
            })?;
            log_line(
                ctx,
                format!(
                    "Removed {} at {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(&artifact.path)
                ),
            );
        }
        if let Some(spec) = ctx.paths.service.reload_after_artifact_change() {
            run_command_spec(ctx, &spec)?;
        }
    } else {
        log_line(
            ctx,
            format!(
                "{} not found at {}",
                ctx.paths.service.artifact_label(),
                format_with_home(&ctx.paths.service.primary_artifact_path())
            ),
        );
    }

    remove_hyprland_autostart(ctx);
    Ok(())
}
