//! Stop and verify the currently running notification daemon.

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use super::{log_line, run_command, ActionContext};

pub fn stop_active_daemon(ctx: &mut ActionContext) -> Result<()> {
    let Some(owner) = ctx.detection.owner.as_ref() else {
        log_line(ctx, "No active notification daemon detected.");
        return Ok(());
    };

    let Some(comm) = owner.comm.as_deref() else {
        log_line(
            ctx,
            "Active owner detected, but command name is unavailable.",
        );
        return Ok(());
    };

    let known = ctx
        .detection
        .daemons
        .iter()
        .find(|daemon| daemon.name == comm);

    if let Some(daemon) = known {
        if daemon.systemd_active {
            let is_unixnotis = daemon.name == "unixnotis-daemon";
            log_line(ctx, format!("Stopping systemd unit {}", daemon.unit));
            let mut command = Command::new("systemctl");
            if is_unixnotis {
                command.args(["--user", "stop", daemon.unit.as_str()]);
            } else {
                command.args(["--user", "disable", "--now", daemon.unit.as_str()]);
            }
            let label = if is_unixnotis {
                format!("systemctl --user stop {}", daemon.unit)
            } else {
                format!("systemctl --user disable --now {}", daemon.unit)
            };
            run_command(ctx, &label, command, None)?;
            return Ok(());
        }

        if let Some(pid) = owner.pid {
            log_line(ctx, format!("Stopping {} (pid {})", daemon.name, pid));
            let status = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .context("failed to terminate notification daemon")?;
            if status.success() {
                wait_for_exit(ctx, pid)?;
                return Ok(());
            }
            return Err(anyhow!("failed to stop {}", daemon.name));
        }
    }

    log_line(
        ctx,
        format!("Detected owner '{}' is not managed by a known unit.", comm),
    );
    Ok(())
}

fn wait_for_exit(ctx: &mut ActionContext, pid: u32) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let poll = Duration::from_millis(100);

    while start.elapsed() < timeout {
        if !pid_alive(pid)? {
            log_line(ctx, format!("Process {} stopped.", pid));
            return Ok(());
        }
        thread::sleep(poll);
    }

    Err(anyhow!("process {} did not exit after 5s", pid))
}

fn pid_alive(pid: u32) -> Result<bool> {
    let status = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .with_context(|| format!("failed to probe pid {pid}"))?;
    Ok(status.success())
}
