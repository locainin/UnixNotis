use anyhow::{anyhow, Result};
use std::process::Command as StdCommand;
use tracing::{debug, info, warn};
use unixnotis_core::util;

use crate::cli::{Args, RestoreStrategy};
use crate::system_tools;

use super::owner::{is_unit_active, run_command_status};
use super::{OwnerInfo, RestoreAction, KNOWN_DAEMONS};

pub fn restore_previous(action: RestoreAction) -> Result<()> {
    match action {
        RestoreAction::Systemd { unit } => {
            // Unit-based restore is safest when daemon was systemd-managed
            info!(unit, "restarting notification daemon unit");
            let mut command = system_tools::command("systemctl")?;
            let status = command.arg("--user").arg("start").arg(&unit).status()?;
            if status.success() {
                Ok(())
            } else {
                Err(anyhow!("systemctl start failed for {unit}"))
            }
        }
        RestoreAction::Command { program, args } => {
            // Command restore is used when no active user unit is detected
            info!(program, "restarting notification daemon process");
            let mut command = StdCommand::new(program);
            if !args.is_empty() {
                command.args(args);
            }
            let child = command.spawn()?;
            std::thread::Builder::new()
                .name("unixnotis-trial-restore-reaper".to_string())
                .spawn(move || {
                    // Detached reaper avoids child zombies after restore spawn
                    if let Err(err) = child.wait_with_output() {
                        warn!(?err, "failed to reap restore process");
                    }
                })
                .map_err(|err| anyhow!("failed to spawn restore reaper thread: {err}"))?;
            Ok(())
        }
    }
}

pub(super) async fn stop_active_owner(
    args: &Args,
    owner: &OwnerInfo,
) -> Result<Option<RestoreAction>> {
    // Trial mode only targets known daemons to avoid stopping unrelated services
    let comm = owner
        .comm
        .as_deref()
        .ok_or_else(|| anyhow!("current owner command name unavailable"))?;
    let pid = owner
        .pid
        .ok_or_else(|| anyhow!("current owner PID unavailable"))?;
    let known = KNOWN_DAEMONS
        .iter()
        .find(|daemon| daemon.name == comm)
        .ok_or_else(|| anyhow!("current owner is not in the known daemon list"))?;

    debug!(
        pid,
        comm,
        strategy = ?args.restore,
        "trial mode: stopping current daemon"
    );
    match args.restore {
        RestoreStrategy::None => {
            // Stop and leave system as-is after trial ends
            stop_via_process(pid).await?;
            debug!("trial mode: no restore action requested");
            Ok(None)
        }
        RestoreStrategy::Systemd => {
            // Strict systemd mode errors if the matched unit is not active
            if !is_unit_active(known.unit).await {
                return Err(anyhow!(
                    "systemd restore requested but {} is not active",
                    known.unit
                ));
            }
            stop_via_systemd(known.unit).await?;
            debug!(unit = known.unit, "trial mode: restore via systemd");
            Ok(Some(RestoreAction::Systemd {
                unit: known.unit.to_string(),
            }))
        }
        RestoreStrategy::Process => {
            // Strict process mode always captures argv for later spawn
            let (program, args) = build_restart_command(owner, comm)?;
            // A complete restart command must exist before the current owner is touched
            stop_via_process(pid).await?;
            let program_snip = util::log_snippet(&program);
            debug!(
                program = %program_snip,
                args_len = args.len(),
                "trial mode: restore via command"
            );
            Ok(Some(RestoreAction::Command { program, args }))
        }
        RestoreStrategy::Auto => {
            // Auto prefers systemd when unit is active, otherwise process restore
            if is_unit_active(known.unit).await {
                stop_via_systemd(known.unit).await?;
                debug!(unit = known.unit, "trial mode: restore via systemd (auto)");
                Ok(Some(RestoreAction::Systemd {
                    unit: known.unit.to_string(),
                }))
            } else {
                let (program, args) = build_restart_command(owner, comm)?;
                // Auto mode follows the same prepare-before-stop transaction as strict mode
                stop_via_process(pid).await?;
                let program_snip = util::log_snippet(&program);
                debug!(
                    program = %program_snip,
                    args_len = args.len(),
                    "trial mode: restore via command (auto)"
                );
                Ok(Some(RestoreAction::Command { program, args }))
            }
        }
    }
}

async fn stop_via_systemd(unit: &str) -> Result<()> {
    // Unit stop keeps process ownership and journald semantics consistent
    info!(unit, "stopping notification daemon unit");
    let command_str = format!("systemctl --user stop {unit}");
    let command_snip = util::log_snippet(&command_str);
    let mut command = system_tools::tokio_command("systemctl")?;
    command.arg("--user").arg("stop").arg(unit);
    let status = run_command_status(&mut command, &command_snip)
        .await
        .ok_or_else(|| anyhow!("systemctl stop failed for {unit} (command error)"))?;
    if status.success() {
        Ok(())
    } else {
        warn!(command = %command_snip, "systemctl stop returned non-zero status");
        Err(anyhow!("systemctl stop failed for {unit}"))
    }
}

async fn stop_via_process(pid: u32) -> Result<()> {
    // TERM is used to allow clean daemon shutdown and socket release
    info!(pid, "stopping notification daemon process");
    let command_str = format!("kill -TERM {pid}");
    let command_snip = util::log_snippet(&command_str);
    let mut command = system_tools::tokio_command("kill")?;
    command.arg("-TERM").arg(pid.to_string());
    let status = run_command_status(&mut command, &command_snip)
        .await
        .ok_or_else(|| anyhow!("failed to stop process {pid} (command error)"))?;
    if status.success() {
        Ok(())
    } else {
        warn!(command = %command_snip, "kill returned non-zero status");
        Err(anyhow!("failed to stop process {pid}"))
    }
}

fn build_restart_command(owner: &OwnerInfo, fallback: &str) -> Result<(String, Vec<String>)> {
    if let Some(args) = owner.args.as_ref() {
        let mut parts = args.clone();
        // Argv[0] is the executable path; the rest are forwarded as-is
        if !parts.is_empty() {
            let program = parts.remove(0);
            return Ok((program, parts));
        }
    }
    // Missing argv leaves only a bare name, so resolve it through trusted system directories
    let program = system_tools::program_path(fallback)?;
    Ok((program.display().to_string(), Vec::new()))
}

#[cfg(test)]
#[path = "tests/control.rs"]
mod tests;
