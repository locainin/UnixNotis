//! Stop and verify the currently running notification daemon

use anyhow::{anyhow, Context, Result};

use super::{log_line, run_command, ActionContext};
use crate::system_tools;

mod process_handle;

use process_handle::{ProcessHandle, ProcessState};

pub fn stop_active_daemon(ctx: &mut ActionContext) -> Result<()> {
    let Some(owner) = ctx.detection.owner.as_ref() else {
        log_line(ctx, "No active notification daemon detected.");
        return Ok(());
    };

    let owner_pid = owner.pid;
    let owner_comm = owner.comm.as_deref();
    // Prefer the bus-reported command name, but fall back to PID matching when comm is unavailable
    let known = owner_comm
        .and_then(|comm| {
            ctx.detection
                .daemons
                .iter()
                .find(|daemon| daemon.name == comm)
        })
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
            let (label, command) = if is_unixnotis {
                // Reinstall can race with session hooks that start the daemon when the bus name drops
                // The irreversible stop job keeps that start request from canceling the stop in flight
                let spec = ctx.paths.service.stop_for_reinstall_command();
                (spec.label().to_string(), spec.to_command()?)
            } else {
                let mut command = system_tools::command("systemctl")
                    .context("failed to locate trusted systemctl")?;
                command.args(["--user", "disable", "--now", daemon.unit.as_str()]);
                (
                    format!("systemctl --user disable --now {}", daemon.unit),
                    command,
                )
            };
            if let Err(err) = run_command(ctx, &label, command, None) {
                if is_systemd_unit_inactive(&daemon.unit)? {
                    // A canceled stop job can still leave the unit stopped, which satisfies reinstall
                    log_line(
                        ctx,
                        format!(
                            "Systemd unit {} is inactive after stop error; continuing.",
                            daemon.unit
                        ),
                    );
                    return Ok(());
                }
                return Err(err);
            }
            return Ok(());
        }

        if let Some(pid) = owner_pid {
            log_line(ctx, format!("Stopping {} (pid {})", daemon.name, pid));
            // A stable process handle prevents a recycled PID from receiving the signal
            let handle = match ProcessHandle::open(pid, &daemon.name)? {
                ProcessState::Gone => {
                    log_line(ctx, format!("Process {pid} already stopped."));
                    return Ok(());
                }
                ProcessState::Running(handle) => handle,
            };
            handle.terminate()?;
            handle.wait_for_exit()?;
            log_line(ctx, format!("Process {pid} stopped."));
            return Ok(());
        }
    }

    unmanaged_owner_error(ctx, owner_comm, owner_pid)
}

fn unmanaged_owner_error(
    ctx: &mut ActionContext,
    owner_comm: Option<&str>,
    owner_pid: Option<u32>,
) -> Result<()> {
    // Preserve the strongest broker identity available in the manual-stop instruction
    let message = owner_comm.map_or_else(
        || {
            owner_pid.map_or_else(
                || {
                    "Detected owner is not managed by a known unit; stop it manually before install."
                        .to_string()
                },
                |pid| {
                    format!(
                        "Detected owner pid {pid} is not managed by a known unit; stop it manually before install."
                    )
                },
            )
        },
        |comm| {
            format!(
                "Detected owner '{comm}' is not managed by a known unit; stop it manually before install."
            )
        },
    );
    log_line(ctx, message.clone());
    Err(anyhow!(message))
}

fn is_systemd_unit_inactive(unit: &str) -> Result<bool> {
    // A failed stop command is only recoverable when systemd agrees the unit is no longer running
    let output = system_tools::command("systemctl")
        .context("failed to locate trusted systemctl")?
        .args(["--user", "is-active", unit])
        .output()
        .with_context(|| format!("failed to check systemd unit state for {unit}"))?;
    let state = String::from_utf8_lossy(&output.stdout);
    let state = state.trim();
    if state.is_empty() && !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "failed to read systemd unit state for {unit}: {}",
            stderr.trim()
        ));
    }
    Ok(systemd_stop_error_is_satisfied_by_state(state))
}

fn systemd_stop_error_is_satisfied_by_state(state: &str) -> bool {
    // Only known non-running states should turn a failed stop command into success
    matches!(state.trim(), "inactive" | "failed" | "unknown")
}

#[cfg(test)]
#[path = "tests/daemon.rs"]
mod tests;
