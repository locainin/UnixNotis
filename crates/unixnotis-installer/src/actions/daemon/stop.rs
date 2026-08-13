//! Exact-owner shutdown for notification daemons discovered during installation

use anyhow::{anyhow, Context, Result};

use super::process_handle::{ProcessHandle, ProcessState};
use super::quiescence::{wait_until_no_conflicting_live_daemon, STOP_QUIESCENCE_TIMEOUT};
use crate::actions::{log_line, run_command, ActionContext};
use crate::system_tools;

pub fn stop_active_daemon(ctx: &mut ActionContext) -> Result<()> {
    let detection = crate::detect::detect_for_mutation()
        .context("refresh notification ownership immediately before stopping the daemon")?;
    if let Some(expected_unique_name) = detection
        .owner
        .as_ref()
        .and_then(|owner| owner.unique_name.as_deref())
    {
        // Process metadata has authority only while the inspected broker owner remains current
        crate::detect::ensure_owner_is_current(expected_unique_name)
            .context("revalidate notification ownership immediately before stopping the daemon")?;
    }
    // A successful manager command is only the start of shutdown; broker and
    // service state must converge before the next installer step is marked done
    stop_active_daemon_with_quiescence(ctx, &detection, |paths| {
        wait_until_no_conflicting_live_daemon(paths, STOP_QUIESCENCE_TIMEOUT)
    })
}

fn stop_active_daemon_with_quiescence<Q>(
    ctx: &mut ActionContext,
    detection: &crate::detect::Detection,
    wait_for_quiescence: Q,
) -> Result<()>
where
    Q: FnOnce(&crate::paths::InstallPaths) -> Result<()>,
{
    let stop_result = stop_active_daemon_with_detection(ctx, detection);
    let quiescence_result = wait_for_quiescence(ctx.paths);

    match (stop_result, quiescence_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(stop_error), Ok(())) => {
            // Runtime truth wins when a service-manager command reports a stale failure
            log_line(
                ctx,
                format!(
                    "Warning: stop command failed after runtime became quiescent ({stop_error:#})"
                ),
            );
            Ok(())
        }
        (Ok(()), Err(state_error)) => Err(state_error).context(
            "service manager reported a successful stop but notification runtime remains live",
        ),
        (Err(stop_error), Err(state_error)) => Err(state_error).context(format!(
            "failed to stop notification daemon ({stop_error:#}); runtime remains live or indeterminate"
        )),
    }
}

fn stop_active_daemon_with_detection(
    ctx: &mut ActionContext,
    detection: &crate::detect::Detection,
) -> Result<()> {
    let Some(owner) = detection.owner.as_ref() else {
        log_line(ctx, "No active notification daemon detected.");
        return Ok(());
    };

    let owner_pid = owner.pid;
    let owner_comm = owner.comm.as_deref();
    // Prefer the bus-reported command name, but fall back to PID matching when comm is unavailable
    let known = owner_comm
        .and_then(|comm| detection.daemons.iter().find(|daemon| daemon.name == comm))
        .or_else(|| {
            owner_pid.and_then(|pid| {
                detection
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
            return stop_systemd_daemon(ctx, daemon);
        }

        if let Some(pid) = owner_pid {
            return stop_process_daemon(ctx, &daemon.name, pid);
        }
    }

    unmanaged_owner_error(ctx, owner_comm, owner_pid)
}

fn stop_systemd_daemon(
    ctx: &mut ActionContext,
    daemon: &crate::detect::DetectedDaemon,
) -> Result<()> {
    let is_unixnotis = daemon.name == "unixnotis-daemon";
    log_line(ctx, format!("Stopping systemd unit {}", daemon.unit));
    let (label, command) = if is_unixnotis {
        // Reinstall can race with session hooks that start the daemon when the bus name drops
        // The irreversible stop job keeps that start request from canceling the stop in flight
        let spec = ctx.paths.service.stop_for_reinstall_command();
        (spec.label().to_string(), spec.to_command()?)
    } else {
        let mut command =
            system_tools::command("systemctl").context("failed to locate trusted systemctl")?;
        command.args(["--user", "disable", "--now", daemon.unit.as_str()]);
        (
            format!("systemctl --user disable --now {}", daemon.unit),
            command,
        )
    };
    if let Err(error) = run_command(ctx, &label, command, None) {
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
        return Err(error);
    }
    Ok(())
}

fn stop_process_daemon(ctx: &mut ActionContext, daemon_name: &str, pid: u32) -> Result<()> {
    log_line(ctx, format!("Stopping {daemon_name} (pid {pid})"));
    // A stable process handle prevents a recycled PID from receiving the signal
    let handle = match ProcessHandle::open(pid, daemon_name)? {
        ProcessState::Gone => {
            log_line(ctx, format!("Process {pid} already stopped."));
            return Ok(());
        }
        ProcessState::Running(handle) => handle,
    };
    handle.terminate()?;
    handle.wait_for_exit()?;
    log_line(ctx, format!("Process {pid} stopped."));
    Ok(())
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
#[path = "tests/stop.rs"]
mod tests;
