//! Install and uninstall filesystem assets.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::paths::{format_with_home, InstallPaths};

use super::{log_line, run_command, ActionContext};

pub fn install_binaries(ctx: &mut ActionContext) -> Result<()> {
    let binaries = [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ];

    fs::create_dir_all(&ctx.paths.bin_dir).with_context(|| "failed to create bin directory")?;

    for binary in binaries {
        let source = ctx.paths.release_dir.join(binary);
        let destination = ctx.paths.bin_dir.join(binary);
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

    Ok(())
}

pub fn remove_binaries(ctx: &mut ActionContext) -> Result<()> {
    let binaries = [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ];

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
    fs::copy(source, destination).map_err(|err| {
        anyhow!(
            "failed to install {} -> {}: {}",
            source_display,
            destination_display,
            err
        )
    })?;
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
