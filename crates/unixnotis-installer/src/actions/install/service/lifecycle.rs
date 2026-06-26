//! Service lifecycle command helpers

use anyhow::{anyhow, Context, Result};

use crate::paths::format_with_home;
use crate::service_manager::CommandSpec;

use super::super::super::{log_line, run_command, ActionContext};
use super::artifacts::{remove_service_artifact, service_artifact_path_exists};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::install) enum ServiceStartMode {
    EnableAndStart,
    StartOnly,
}

fn service_start_mode(ctx: &ActionContext) -> ServiceStartMode {
    // Cached install state keeps the reinstall branch stable for one installer run
    service_start_mode_from_enabled(
        ctx.install_state
            .as_ref()
            .map(|state| state.service_enabled()),
    )
}

pub(in crate::actions::install) fn service_start_mode_from_enabled(
    service_enabled: Option<bool>,
) -> ServiceStartMode {
    if service_enabled == Some(true) {
        // Reinstalls do not need another enable step, which can trigger a costly reload
        ServiceStartMode::StartOnly
    } else {
        ServiceStartMode::EnableAndStart
    }
}

pub(in crate::actions::install) fn run_service_start(ctx: &mut ActionContext) -> Result<()> {
    match service_start_mode(ctx) {
        ServiceStartMode::EnableAndStart => {
            // First install still needs the symlink creation done by `enable`
            log_line(
                ctx,
                format!("Enabling and starting {}", ctx.paths.service.service_name()),
            );
            let spec = ctx
                .paths
                .service
                .enable_now_command()
                .ok_or_else(|| anyhow!("service manager cannot enable and start service"))?;
            run_command_spec(ctx, &spec)
        }
        ServiceStartMode::StartOnly => {
            // Reinstall can start directly because the service is already enabled
            log_line(
                ctx,
                format!("Starting {}", ctx.paths.service.service_name()),
            );
            let spec = ctx
                .paths
                .service
                .start_command()
                .ok_or_else(|| anyhow!("service manager cannot start service"))?;
            run_command_spec(ctx, &spec)
        }
    }
}

pub(in crate::actions::install) fn remove_pre_start_artifacts(
    ctx: &mut ActionContext,
) -> Result<()> {
    let artifacts = ctx.paths.service.pre_start_artifacts_to_remove();
    for artifact in &artifacts {
        // Backend staging files are removed only after env sync has completed
        remove_service_artifact(artifact).with_context(|| {
            format!(
                "failed to remove start gate at {}",
                format_with_home(&artifact.path)
            )
        })?;
    }
    Ok(())
}

pub(in crate::actions::install) fn warn_pre_start_artifacts_left(ctx: &mut ActionContext) {
    for artifact in ctx.paths.service.pre_start_artifacts_to_remove() {
        if service_artifact_path_exists(&artifact) {
            log_line(
                ctx,
                format!(
                    "Warning: service start gate remains at {}; autostart is disabled until it is removed",
                    format_with_home(&artifact.path)
                ),
            );
        }
    }
}

pub(in crate::actions::install) fn run_command_spec(
    ctx: &mut ActionContext,
    spec: &CommandSpec,
) -> Result<()> {
    run_command(ctx, spec.label(), spec.to_command(), None)
}
