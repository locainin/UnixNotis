//! Service unit install and lifecycle helpers

use std::fs;
use std::process::Command;
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
    fs::create_dir_all(&ctx.paths.unit_dir)
        .with_context(|| "failed to create systemd user directory")?;

    match write_service_unit(ctx)? {
        ServiceUnitWrite::CreatedOrUpdated => {
            log_line(
                ctx,
                format!(
                    "Installed systemd unit to {}",
                    format_with_home(&ctx.paths.unit_path)
                ),
            );
        }
        ServiceUnitWrite::Unchanged => {
            log_line(ctx, "Systemd unit already up to date");
        }
    }

    Ok(())
}

pub(crate) fn enable_service(ctx: &mut ActionContext) -> Result<()> {
    if ctx.service_unit_reload_required.load(Ordering::Acquire) {
        // A full user-manager reload is expensive on some setups, so run it only when needed
        log_line(ctx, "Reloading systemd user manager");
        let mut daemon_reload = Command::new("systemctl");
        daemon_reload.args(["--user", "daemon-reload"]);
        run_command(ctx, "systemctl --user daemon-reload", daemon_reload, None)?;
    } else {
        log_line(
            ctx,
            "Skipping systemd user reload because unit is unchanged",
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
    let unit = &ctx.paths.unit_path;
    let unit_display = format_with_home(unit);

    if unit.exists() {
        let mut disable = Command::new("systemctl");
        disable.args(["--user", "disable", "--now", "unixnotis-daemon.service"]);
        if let Err(err) = run_command(
            ctx,
            "systemctl --user disable --now unixnotis-daemon.service",
            disable,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        }

        let mut daemon_reload = Command::new("systemctl");
        daemon_reload.args(["--user", "daemon-reload"]);
        fs::remove_file(unit).with_context(|| "failed to remove systemd unit")?;
        run_command(ctx, "systemctl --user daemon-reload", daemon_reload, None)?;
        log_line(ctx, format!("Removed systemd unit at {}", unit_display));
    } else {
        log_line(ctx, format!("Systemd unit not found at {}", unit_display));
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
pub(in crate::actions::install) enum ServiceUnitWrite {
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

fn write_service_unit(ctx: &mut ActionContext) -> Result<ServiceUnitWrite> {
    // Render first so both compare and write paths operate on the exact same bytes
    let unit_contents = render_service_unit(ctx.paths);
    let existed_before = ctx.paths.unit_path.exists();
    let write_outcome = match fs::read_to_string(&ctx.paths.unit_path) {
        Ok(existing) if existing == unit_contents => ServiceUnitWrite::Unchanged,
        Ok(_) | Err(_) => {
            write_atomic(&ctx.paths.unit_path, &unit_contents)
                .with_context(|| "failed to write systemd user unit")?;
            ServiceUnitWrite::CreatedOrUpdated
        }
    };

    // Reload only matters when systemd has new unit bytes to pick up
    let reload_required =
        matches!(write_outcome, ServiceUnitWrite::CreatedOrUpdated) || !existed_before;
    ctx.service_unit_reload_required
        .store(reload_required, Ordering::Release);
    Ok(write_outcome)
}

fn service_start_mode(ctx: &ActionContext) -> ServiceStartMode {
    // Cached install state keeps the reinstall branch stable for one installer run
    service_start_mode_from_enabled(ctx.install_state.as_ref().map(|state| state.unit_enabled()))
}

pub(in crate::actions::install) fn service_start_mode_from_enabled(
    unit_enabled: Option<bool>,
) -> ServiceStartMode {
    if unit_enabled == Some(true) {
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
            log_line(ctx, "Enabling and starting unixnotis-daemon.service");
            let mut enable = Command::new("systemctl");
            enable.args(["--user", "enable", "--now", "unixnotis-daemon.service"]);
            run_command(
                ctx,
                "systemctl --user enable --now unixnotis-daemon.service",
                enable,
                None,
            )
        }
        ServiceStartMode::StartOnly => {
            // Reinstall can start directly because the unit is already enabled
            log_line(ctx, "Starting unixnotis-daemon.service");
            let mut start = Command::new("systemctl");
            start.args(["--user", "start", "unixnotis-daemon.service"]);
            run_command(
                ctx,
                "systemctl --user start unixnotis-daemon.service",
                start,
                None,
            )
        }
    }
}
