//! Install and uninstall filesystem assets.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::paths::{format_with_home, InstallPaths};

use super::{
    actions_binaries::resolve_install_binaries,
    actions_env::sync_user_environment,
    actions_hyprland::{ensure_hyprland_autostart, remove_hyprland_autostart},
    log_line, run_command, ActionContext,
};

pub fn install_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Resolve the installable binaries from workspace metadata to avoid hardcoded duplication.
    let binaries = resolve_install_binaries(ctx.paths)?;

    fs::create_dir_all(&ctx.paths.bin_dir).with_context(|| "failed to create bin directory")?;

    for binary in binaries {
        let source = ctx.paths.release_dir.join(&binary);
        let destination = ctx.paths.bin_dir.join(&binary);
        copy_binary(ctx, &source, &destination)?;
    }

    Ok(())
}

pub fn install_service(ctx: &mut ActionContext) -> Result<()> {
    fs::create_dir_all(&ctx.paths.unit_dir)
        .with_context(|| "failed to create systemd user directory")?;

    let exec_start = format_exec_start(ctx.paths);
    let unit_contents = [
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
    .join("\n");

    fs::write(&ctx.paths.unit_path, unit_contents)
        .with_context(|| "failed to write systemd user unit")?;

    log_line(
        ctx,
        format!(
            "Installed systemd unit to {}",
            format_with_home(&ctx.paths.unit_path)
        ),
    );

    Ok(())
}

pub fn enable_service(ctx: &mut ActionContext) -> Result<()> {
    let mut daemon_reload = Command::new("systemctl");
    daemon_reload.args(["--user", "daemon-reload"]);
    run_command(ctx, "systemctl --user daemon-reload", daemon_reload, None)?;
    let mut enable = Command::new("systemctl");
    enable.args(["--user", "enable", "--now", "unixnotis-daemon.service"]);
    run_command(
        ctx,
        "systemctl --user enable --now unixnotis-daemon.service",
        enable,
        None,
    )?;
    // Keep the environment in sync after enabling the service so future restarts inherit it.
    sync_user_environment(ctx)?;
    // Hyprland exec-once ensures session vars are synced once per login without extra hooks.
    ensure_hyprland_autostart(ctx);
    Ok(())
}

pub fn uninstall_service(ctx: &mut ActionContext) -> Result<()> {
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

pub fn remove_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Use the same discovery logic as installation to keep uninstall symmetric.
    let binaries = resolve_install_binaries(ctx.paths)?;

    for binary in binaries {
        let path = ctx.paths.bin_dir.join(binary);
        if path.exists() {
            fs::remove_file(&path).with_context(|| "failed to remove binary")?;
            log_line(ctx, format!("Removed binary {}", format_with_home(&path)));
        } else {
            log_line(
                ctx,
                format!("Binary not found at {}", format_with_home(&path)),
            );
        }
    }

    Ok(())
}

fn copy_binary(ctx: &mut ActionContext, source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        return Err(anyhow!(
            "missing build artifact: {}",
            format_with_home(source)
        ));
    }

    let source_display = format_with_home(source);
    let destination_display = format_with_home(destination);
    // Copy to a temporary file in the target directory to keep updates atomic.
    let temp_name = format!(
        "{}.tmp-{}",
        destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        std::process::id()
    );
    let temp_path = destination.with_file_name(temp_name);

    if temp_path.exists() {
        // Clear stale temp files from previous interrupted installs.
        fs::remove_file(&temp_path).with_context(|| "failed to remove stale temp file")?;
    }

    fs::copy(source, &temp_path).map_err(|err| {
        anyhow!(
            "failed to stage {} -> {}: {}",
            source_display,
            format_with_home(&temp_path),
            err
        )
    })?;

    if destination.exists() {
        // Remove the existing file to avoid rename failures on platforms that forbid overwrite.
        fs::remove_file(destination)
            .with_context(|| "failed to remove existing destination binary")?;
    }

    if let Err(err) = fs::rename(&temp_path, destination) {
        let _ = fs::remove_file(&temp_path);
        return Err(anyhow!(
            "failed to install {} -> {}: {}",
            source_display,
            destination_display,
            err
        ));
    }
    log_line(
        ctx,
        format!(
            "Installed {} -> {}",
            source.file_name().unwrap_or_default().to_string_lossy(),
            format_with_home(destination)
        ),
    );
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
