//! Trial-mode helpers for temporarily replacing another notification daemon.
//!
//! Keeps detection, stopping, and restoring logic separate from main startup flow.

use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use anyhow::{anyhow, Result};
use std::process::Command as StdCommand;
use tokio::fs;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;
use tracing::{debug, info, warn};
use unixnotis_core::util;
use zbus::fdo::DBusProxy;

use super::dbus_owner::wait_for_owner_state;
use super::{Args, RestoreStrategy};

#[derive(Default)]
pub(super) struct TrialState {
    restore_action: Option<RestoreAction>,
}

impl TrialState {
    pub(super) fn take_restore_action(&mut self) -> Option<RestoreAction> {
        self.restore_action.take()
    }
}

struct OwnerInfo {
    pid: Option<u32>,
    comm: Option<String>,
    args: Option<Vec<String>>,
}

struct DetectedDaemon {
    name: String,
    systemd_active: bool,
    running_pids: Vec<u32>,
    is_owner: bool,
}

pub(super) enum RestoreAction {
    Systemd { unit: String },
    Command { program: String, args: Vec<String> },
}

struct KnownDaemon {
    name: &'static str,
    unit: &'static str,
}

const KNOWN_DAEMONS: &[KnownDaemon] = &[
    KnownDaemon {
        name: "mako",
        unit: "mako.service",
    },
    KnownDaemon {
        name: "dunst",
        unit: "dunst.service",
    },
    KnownDaemon {
        name: "swaync",
        unit: "swaync.service",
    },
    KnownDaemon {
        name: "notify-osd",
        unit: "notify-osd.service",
    },
];

const TRIAL_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) async fn prepare_trial(
    args: &Args,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
) -> Result<TrialState> {
    debug!("trial mode detection started");
    let owner = detect_owner(dbus_proxy, notifications_name.clone()).await?;
    if owner.is_none() {
        debug!("trial mode: no current notification owner");
        return Ok(TrialState::default());
    }

    if let Some(info) = owner.as_ref() {
        debug!(
            pid = info.pid,
            comm = info.comm.as_deref().unwrap_or("unknown"),
            "trial mode: current owner detected"
        );
    }
    let daemons = detect_known_daemons(&owner).await;
    print_detected_daemons(&daemons, &owner);

    if !args.yes {
        let confirmed = tokio::task::spawn_blocking(confirm_trial)
            .await
            .map_err(|err| anyhow!("trial prompt failed: {err}"))??;
        if !confirmed {
            return Err(anyhow!("trial cancelled"));
        }
    }

    let Some(owner) = owner else {
        return Err(anyhow!("no current owner detected for trial mode"));
    };
    let restore_action = stop_active_owner(args, &owner).await?;
    let released = wait_for_owner_state(
        dbus_proxy,
        notifications_name,
        false,
        Duration::from_millis(args.restore_wait_ms),
    )
    .await?;
    if !released {
        return Err(anyhow!(
            "org.freedesktop.Notifications did not release in time"
        ));
    }

    debug!("trial mode preparation complete");
    Ok(TrialState { restore_action })
}

pub(super) fn restore_previous(action: RestoreAction) -> Result<()> {
    match action {
        RestoreAction::Systemd { unit } => {
            info!(unit, "restarting notification daemon unit");
            let status = StdCommand::new("systemctl")
                .arg("--user")
                .arg("start")
                .arg(&unit)
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(anyhow!("systemctl start failed for {}", unit))
            }
        }
        RestoreAction::Command { program, args } => {
            info!(program, "restarting notification daemon process");
            let mut command = StdCommand::new(program);
            if !args.is_empty() {
                command.args(args);
            }
            let _ = command.spawn()?;
            Ok(())
        }
    }
}

async fn detect_owner(
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
) -> Result<Option<OwnerInfo>> {
    let has_owner = dbus_proxy
        .name_has_owner(notifications_name.clone())
        .await
        .unwrap_or(false);
    if !has_owner {
        return Ok(None);
    }

    let owner = dbus_proxy
        .get_name_owner(notifications_name)
        .await
        .ok()
        .map(|name| name.to_string());
    let Some(unique_name) = owner else {
        return Ok(None);
    };

    let pid = if let Ok(bus_name) = zbus::names::BusName::try_from(unique_name.as_str()) {
        dbus_proxy
            .get_connection_unix_process_id(bus_name)
            .await
            .ok()
    } else {
        None
    };
    let comm = match pid {
        Some(pid) => read_comm(pid).await,
        None => None,
    };
    let args = match pid {
        Some(pid) => read_args(pid).await,
        None => None,
    };

    Ok(Some(OwnerInfo { pid, comm, args }))
}

async fn detect_known_daemons(owner: &Option<OwnerInfo>) -> Vec<DetectedDaemon> {
    let owner_name = owner.as_ref().and_then(|info| info.comm.as_deref());
    let mut entries = Vec::new();
    for daemon in KNOWN_DAEMONS {
        let running_pids = pgrep_exact(daemon.name).await;
        let systemd_active = is_unit_active(daemon.unit).await;
        let is_owner = owner_name == Some(daemon.name);
        entries.push(DetectedDaemon {
            name: daemon.name.to_string(),
            systemd_active,
            running_pids,
            is_owner,
        });
    }
    entries
}

fn print_detected_daemons(daemons: &[DetectedDaemon], owner: &Option<OwnerInfo>) {
    println!("Detected notification daemons:");
    let mut owner_listed = false;
    for daemon in daemons {
        let mut status = Vec::new();
        if daemon.is_owner {
            owner_listed = true;
            status.push("dbus-owner".to_string());
        }
        if daemon.systemd_active {
            status.push("systemd-active".to_string());
        }
        if !daemon.running_pids.is_empty() {
            let ids = daemon
                .running_pids
                .iter()
                .map(|pid| pid.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            status.push(format!("pid {}", ids));
        }
        if status.is_empty() {
            status.push("not running".to_string());
        }
        println!("- {}: {}", daemon.name, status.join(", "));
    }
    if !owner_listed {
        if let Some(owner) = owner.as_ref() {
            let name = owner.comm.as_deref().unwrap_or("unknown");
            let pid = owner
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            println!("- {}: dbus-owner, pid {}", name, pid);
        }
    }
}

fn confirm_trial() -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    print!("Enable UnixNotis trial and stop the active daemon? [y/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_ascii_lowercase();
    Ok(matches!(input.as_str(), "y" | "yes"))
}

async fn stop_active_owner(args: &Args, owner: &OwnerInfo) -> Result<Option<RestoreAction>> {
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
            stop_via_process(pid).await?;
            debug!("trial mode: no restore action requested");
            Ok(None)
        }
        RestoreStrategy::Systemd => {
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
            stop_via_process(pid).await?;
            let (program, args) = build_restart_command(owner, comm);
            let program_snip = util::log_snippet(&program);
            debug!(
                program = %program_snip,
                args_len = args.len(),
                "trial mode: restore via command"
            );
            Ok(Some(RestoreAction::Command { program, args }))
        }
        RestoreStrategy::Auto => {
            if is_unit_active(known.unit).await {
                stop_via_systemd(known.unit).await?;
                debug!(unit = known.unit, "trial mode: restore via systemd (auto)");
                Ok(Some(RestoreAction::Systemd {
                    unit: known.unit.to_string(),
                }))
            } else {
                stop_via_process(pid).await?;
                let (program, args) = build_restart_command(owner, comm);
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
    info!(unit, "stopping notification daemon unit");
    let command_str = format!("systemctl --user stop {unit}");
    let command_snip = util::log_snippet(&command_str);
    let mut command = TokioCommand::new("systemctl");
    command.arg("--user").arg("stop").arg(unit);
    let status = run_command_status(&mut command, &command_snip)
        .await
        .ok_or_else(|| anyhow!("systemctl stop failed for {} (command error)", unit))?;
    if status.success() {
        Ok(())
    } else {
        warn!(command = %command_snip, "systemctl stop returned non-zero status");
        Err(anyhow!("systemctl stop failed for {}", unit))
    }
}

async fn stop_via_process(pid: u32) -> Result<()> {
    info!(pid, "stopping notification daemon process");
    let command_str = format!("kill -TERM {pid}");
    let command_snip = util::log_snippet(&command_str);
    let mut command = TokioCommand::new("kill");
    command.arg("-TERM").arg(pid.to_string());
    let status = run_command_status(&mut command, &command_snip)
        .await
        .ok_or_else(|| anyhow!("failed to stop process {} (command error)", pid))?;
    if status.success() {
        Ok(())
    } else {
        warn!(command = %command_snip, "kill returned non-zero status");
        Err(anyhow!("failed to stop process {}", pid))
    }
}

fn build_restart_command(owner: &OwnerInfo, fallback: &str) -> (String, Vec<String>) {
    if let Some(args) = owner.args.as_ref() {
        let mut parts = args.clone();
        if !parts.is_empty() {
            let program = parts.remove(0);
            return (program, parts);
        }
    }
    (fallback.to_string(), Vec::new())
}

async fn pgrep_exact(name: &str) -> Vec<u32> {
    let command_str = format!("pgrep -x {name}");
    let command_snip = util::log_snippet(&command_str);
    let mut command = TokioCommand::new("pgrep");
    command.arg("-x").arg(name);
    let output = match run_command_output(&mut command, &command_snip).await {
        Some(output) => output,
        None => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect()
}

async fn read_comm(pid: u32) -> Option<String> {
    // Prefer /proc to avoid spawning a process for a single field.
    let path = format!("/proc/{}/comm", pid);
    if let Ok(contents) = fs::read_to_string(path).await {
        let comm = contents.trim().to_string();
        if !comm.is_empty() {
            return Some(comm);
        }
    }
    let command_str = format!("ps -p {pid} -o comm=");
    let command_snip = util::log_snippet(&command_str);
    let mut command = TokioCommand::new("ps");
    command
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("comm=");
    let output = run_command_output(&mut command, &command_snip).await?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn read_args(pid: u32) -> Option<Vec<String>> {
    // Use /proc/cmdline to preserve argument boundaries and quoting.
    let path = format!("/proc/{}/cmdline", pid);
    if let Ok(contents) = fs::read(path).await {
        let parts = contents
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).to_string())
            .collect::<Vec<_>>();
        if !parts.is_empty() {
            return Some(parts);
        }
    }
    let command_str = format!("ps -p {pid} -o args=");
    let command_snip = util::log_snippet(&command_str);
    let mut command = TokioCommand::new("ps");
    command
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("args=");
    let output = run_command_output(&mut command, &command_snip).await?;
    if !output.status.success() {
        return None;
    }
    let fallback = String::from_utf8_lossy(&output.stdout);
    let parts = fallback
        .split_whitespace()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

async fn is_unit_active(unit: &str) -> bool {
    let command_str = format!("systemctl --user is-active --quiet {unit}");
    let command_snip = util::log_snippet(&command_str);
    let mut command = TokioCommand::new("systemctl");
    command
        .arg("--user")
        .arg("is-active")
        .arg("--quiet")
        .arg(unit);
    let status = match run_command_status(&mut command, &command_snip).await {
        Some(status) => status,
        None => return false,
    };
    status.success()
}

async fn run_command_output(
    command: &mut TokioCommand,
    command_snip: &str,
) -> Option<std::process::Output> {
    match timeout(TRIAL_COMMAND_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => Some(output),
        Ok(Err(err)) => {
            warn!(command = %command_snip, ?err, "trial command failed");
            None
        }
        Err(_) => {
            warn!(command = %command_snip, "trial command timed out");
            None
        }
    }
}

async fn run_command_status(
    command: &mut TokioCommand,
    command_snip: &str,
) -> Option<std::process::ExitStatus> {
    match timeout(TRIAL_COMMAND_TIMEOUT, command.status()).await {
        Ok(Ok(status)) => Some(status),
        Ok(Err(err)) => {
            warn!(command = %command_snip, ?err, "trial command failed");
            None
        }
        Err(_) => {
            warn!(command = %command_snip, "trial command timed out");
            None
        }
    }
}
