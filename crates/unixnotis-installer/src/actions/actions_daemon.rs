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

    let owner_pid = owner.pid;
    let owner_comm = owner.comm.as_deref();
    // Prefer the bus-reported command name, but fall back to PID matching when comm is unavailable.
    let known = owner_comm
        .and_then(|comm| ctx.detection.daemons.iter().find(|daemon| daemon.name == comm))
        .or_else(|| {
            owner_pid.and_then(|pid| {
                ctx.detection
                    .daemons
                    .iter()
                    .find(|daemon| daemon.running_pids.contains(&pid))
            })
        });

    if let Some(daemon) = known {
        if owner_comm.is_none() {
            log_line(
                ctx,
                format!(
                    "Active owner detected without command name; matched pid to {}",
                    daemon.name
                ),
            );
        }
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

        if let Some(pid) = owner_pid {
            log_line(ctx, format!("Stopping {} (pid {})", daemon.name, pid));
            // Re-check the command name to avoid signaling a recycled PID.
            if !pid_matches_comm(pid, &daemon.name)? {
                return Err(anyhow!(
                    "pid {} no longer matches expected daemon {}; aborting stop",
                    pid,
                    daemon.name
                ));
            }
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

    if let Some(comm) = owner_comm {
        log_line(
            ctx,
            format!("Detected owner '{}' is not managed by a known unit.", comm),
        );
    } else if let Some(pid) = owner_pid {
        log_line(
            ctx,
            format!("Detected owner pid {} is not managed by a known unit.", pid),
        );
    } else {
        log_line(ctx, "Detected owner is not managed by a known unit.");
    }
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

fn pid_matches_comm(pid: u32, expected: &str) -> Result<bool> {
    // Validate the process name with ps before sending signals to avoid PID reuse hazards.
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .with_context(|| format!("failed to read comm for pid {pid}"))?;
    if !output.status.success() {
        return Ok(false);
    }
    let comm = String::from_utf8_lossy(&output.stdout);
    let comm = comm.trim();
    if comm.is_empty() {
        return Ok(false);
    }
    Ok(comm == expected)
}
