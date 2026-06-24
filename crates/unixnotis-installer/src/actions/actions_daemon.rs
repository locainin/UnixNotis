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
            let mut command = Command::new("systemctl");
            if is_unixnotis {
                // Reinstall can race with session hooks that start the daemon when the bus name drops
                // The irreversible stop job keeps that start request from canceling the stop in flight
                command.args([
                    "--user",
                    "--job-mode=replace-irreversibly",
                    "stop",
                    daemon.unit.as_str(),
                ]);
            } else {
                command.args(["--user", "disable", "--now", daemon.unit.as_str()]);
            }
            let label = if is_unixnotis {
                format!(
                    "systemctl --user --job-mode=replace-irreversibly stop {}",
                    daemon.unit
                )
            } else {
                format!("systemctl --user disable --now {}", daemon.unit)
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
            // If the process is already gone, the stop goal is satisfied.
            if !pid_alive(pid)? {
                log_line(ctx, format!("Process {} already stopped.", pid));
                return Ok(());
            }
            // Re-check the command name to avoid signaling a recycled PID.
            if !pid_matches_comm(pid, &daemon.name)? {
                // Re-check liveness to treat a natural exit as success.
                if !pid_alive(pid)? {
                    log_line(ctx, format!("Process {} already stopped.", pid));
                    return Ok(());
                }
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
                wait_for_exit(ctx, pid, &daemon.name)?;
                return Ok(());
            }
            return Err(anyhow!("failed to stop {}", daemon.name));
        }
    }

    if let Some(comm) = owner_comm {
        let message = format!(
            "Detected owner '{}' is not managed by a known unit; stop it manually before install.",
            comm
        );
        log_line(ctx, message.clone());
        return Err(anyhow!(message));
    }
    if let Some(pid) = owner_pid {
        let message = format!(
            "Detected owner pid {} is not managed by a known unit; stop it manually before install.",
            pid
        );
        log_line(ctx, message.clone());
        return Err(anyhow!(message));
    }
    let message = "Detected owner is not managed by a known unit; stop it manually before install."
        .to_string();
    log_line(ctx, message.clone());
    Err(anyhow!(message))
}

fn wait_for_exit(ctx: &mut ActionContext, pid: u32, expected_comm: &str) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let poll = Duration::from_millis(100);

    while start.elapsed() < timeout {
        if !pid_alive(pid)? {
            log_line(ctx, format!("Process {} stopped.", pid));
            return Ok(());
        }
        // PID reuse protection: verify the command name during the wait loop.
        if !pid_matches_comm(pid, expected_comm)? {
            return Err(anyhow!(
                "pid {} no longer matches expected daemon {}; aborting wait",
                pid,
                expected_comm
            ));
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

fn is_systemd_unit_inactive(unit: &str) -> Result<bool> {
    // A failed stop command is only recoverable when systemd agrees the unit is no longer running
    let output = Command::new("systemctl")
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
mod tests {
    use super::systemd_stop_error_is_satisfied_by_state;

    #[test]
    fn systemd_stop_error_can_continue_when_unit_is_inactive() {
        assert!(systemd_stop_error_is_satisfied_by_state("inactive"));
    }

    #[test]
    fn systemd_stop_error_can_continue_when_unit_is_failed() {
        assert!(systemd_stop_error_is_satisfied_by_state("failed"));
    }

    #[test]
    fn systemd_stop_error_still_fails_when_unit_stays_active() {
        assert!(!systemd_stop_error_is_satisfied_by_state("active"));
    }

    #[test]
    fn systemd_stop_error_still_fails_when_unit_is_transitioning() {
        assert!(!systemd_stop_error_is_satisfied_by_state("deactivating"));
    }
}
