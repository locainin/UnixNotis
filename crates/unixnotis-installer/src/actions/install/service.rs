//! Service artifact install and lifecycle helpers

use std::fs;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};

use crate::paths::{format_with_home, InstallPaths};

use super::super::{
    config::backup::write_atomic,
    ensure_shell_path_entry,
    hyprland::{ensure_hyprland_autostart, remove_hyprland_autostart},
    log_line, run_command, sync_user_environment, ActionContext,
};

pub(crate) fn install_service(ctx: &mut ActionContext) -> Result<()> {
    fs::create_dir_all(ctx.paths.service.artifact_dir()).with_context(|| {
        format!(
            "failed to create {} directory",
            ctx.paths.service.artifact_label()
        )
    })?;

    match write_service_artifact(ctx)? {
        ServiceArtifactWrite::CreatedOrUpdated => {
            log_line(
                ctx,
                format!(
                    "Installed {} to {}",
                    ctx.paths.service.artifact_label(),
                    format_with_home(ctx.paths.service.artifact_path())
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
        let (label, command) = ctx.paths.service.daemon_reload_command();
        run_command(ctx, &label, command, None)?;
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
    sync_user_environment(ctx)?;
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
    let artifact_path = ctx.paths.service.artifact_path();
    let artifact_display = format_with_home(artifact_path);

    if artifact_path.exists() {
        let (label, command) = ctx.paths.service.disable_now_command();
        if let Err(err) = run_command(ctx, &label, command, None) {
            log_line(ctx, format!("Warning: {}", err));
        }

        fs::remove_file(artifact_path)
            .with_context(|| format!("failed to remove {}", ctx.paths.service.artifact_label()))?;
        let (label, command) = ctx.paths.service.daemon_reload_command();
        run_command(ctx, &label, command, None)?;
        log_line(
            ctx,
            format!(
                "Removed {} at {}",
                ctx.paths.service.artifact_label(),
                artifact_display
            ),
        );
    } else {
        log_line(
            ctx,
            format!(
                "{} not found at {}",
                ctx.paths.service.artifact_label(),
                artifact_display
            ),
        );
    }

    remove_hyprland_autostart(ctx);
    Ok(())
}

fn format_exec_start(paths: &InstallPaths) -> String {
    let path = paths.bin_dir.join("unixnotis-daemon");
    let rendered = format_with_home(&path);
    if let Some(tail) = rendered.strip_prefix("$HOME") {
        format!("%h{}", tail)
    } else {
        path.display().to_string()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::install) enum ServiceArtifactWrite {
    CreatedOrUpdated,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::install) enum ServiceStartMode {
    EnableAndStart,
    StartOnly,
}

pub(in crate::actions::install) fn render_service_unit(paths: &InstallPaths) -> String {
    let exec_start = format_exec_start(paths);
    [
        "[Unit]".to_string(),
        "Description=UnixNotis Notification Daemon".to_string(),
        "After=graphical-session.target".to_string(),
        "Wants=graphical-session.target".to_string(),
        "".to_string(),
        "[Service]".to_string(),
        "Type=simple".to_string(),
        format!("ExecStart={}", exec_start),
        "Restart=on-failure".to_string(),
        "RestartSec=1".to_string(),
        "".to_string(),
        "[Install]".to_string(),
        "WantedBy=default.target".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

fn write_service_artifact(ctx: &mut ActionContext) -> Result<ServiceArtifactWrite> {
    // Render first so both compare and write paths operate on the exact same bytes
    let artifact_contents = render_service_unit(ctx.paths);
    let artifact_path = ctx.paths.service.artifact_path();
    let existed_before = artifact_path.exists();
    let write_outcome = match fs::read_to_string(artifact_path) {
        Ok(existing) if existing == artifact_contents => ServiceArtifactWrite::Unchanged,
        Ok(_) | Err(_) => {
            write_atomic(artifact_path, &artifact_contents).with_context(|| {
                format!("failed to write {}", ctx.paths.service.artifact_label())
            })?;
            ServiceArtifactWrite::CreatedOrUpdated
        }
    };

    // Reload only matters when the active service manager has new bytes to pick up
    let reload_required =
        matches!(write_outcome, ServiceArtifactWrite::CreatedOrUpdated) || !existed_before;
    ctx.service_reload_required
        .store(reload_required, Ordering::Release);
    Ok(write_outcome)
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

fn run_service_start(ctx: &mut ActionContext) -> Result<()> {
    match service_start_mode(ctx) {
        ServiceStartMode::EnableAndStart => {
            // First install still needs the symlink creation done by `enable`
            log_line(
                ctx,
                format!("Enabling and starting {}", ctx.paths.service.service_name()),
            );
            let (label, command) = ctx.paths.service.enable_now_command();
            run_command(ctx, &label, command, None)
        }
        ServiceStartMode::StartOnly => {
            // Reinstall can start directly because the service is already enabled
            log_line(
                ctx,
                format!("Starting {}", ctx.paths.service.service_name()),
            );
            let (label, command) = ctx.paths.service.start_command();
            run_command(ctx, &label, command, None)
        }
    }
}
