use anyhow::{Context, Result};
use tokio::fs;
use tokio::time::timeout;
use tracing::warn;
use unixnotis_core::util;
use zbus::fdo::DBusProxy;

use crate::system_tools;

use super::state::NotificationOwnerState;
use super::{DetectedDaemon, OwnerInfo, KNOWN_DAEMONS, TRIAL_COMMAND_TIMEOUT};

pub(super) async fn detect_owner(
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
) -> Result<NotificationOwnerState> {
    // Quick owner check avoids extra calls when Notifications is unclaimed
    let has_owner = dbus_proxy
        .name_has_owner(notifications_name.clone())
        .await
        .context("query Notifications ownership")?;
    if !has_owner {
        return Ok(NotificationOwnerState::Unowned);
    }

    let owner = dbus_proxy
        .get_name_owner(notifications_name)
        .await
        .context("resolve Notifications owner")?;
    let unique_name = owner.to_string();

    // Resolve PID from unique bus name when possible
    let pid = if let Ok(bus_name) = zbus::names::BusName::try_from(unique_name.as_str()) {
        dbus_proxy
            .get_connection_unix_process_id(bus_name)
            .await
            .ok()
    } else {
        None
    };
    let args = match pid {
        Some(pid) => read_args(pid).await,
        None => None,
    };
    // Argv keeps long executable names intact while /proc comm truncates after 15 bytes
    let comm = args
        .as_deref()
        .and_then(command_program_name)
        .or(match pid {
            Some(pid) => read_comm(pid).await,
            None => None,
        });

    Ok(NotificationOwnerState::Owned(OwnerInfo {
        unique_name,
        pid,
        comm,
        args,
    }))
}

pub(super) async fn ensure_owner_is_current(
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
    inspected: &OwnerInfo,
) -> Result<()> {
    let current = dbus_proxy
        .get_name_owner(notifications_name)
        .await
        .context("revalidate Notifications owner before trial stop")?;
    anyhow::ensure!(
        current.as_str() == inspected.unique_name,
        "Notifications owner changed during trial preparation; refusing to stop either process"
    );
    Ok(())
}

pub(super) async fn detect_known_daemons(owner: &Option<OwnerInfo>) -> Vec<DetectedDaemon> {
    // Owner process name is used to tag which known daemon currently owns the bus
    let owner_name = owner.as_ref().and_then(|info| info.comm.as_deref());
    let mut entries = Vec::new();
    for daemon in KNOWN_DAEMONS {
        let running_pids = pgrep_exact(daemon.name).await;
        let systemd_active = match daemon.systemd_unit {
            Some(unit) => is_unit_active(unit).await,
            None => false,
        };
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

fn command_program_name(args: &[String]) -> Option<String> {
    let program = args.first()?;
    std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

pub(super) fn print_detected_daemons(daemons: &[DetectedDaemon], owner: &Option<OwnerInfo>) {
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
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            status.push(format!("pid {ids}"));
        }
        if status.is_empty() {
            status.push("not running".to_string());
        }
        println!("- {}: {}", daemon.name, status.join(", "));
    }
    if !owner_listed {
        if let Some(owner) = owner.as_ref() {
            // Show unknown owner so operators still have PID and process name context
            let name = owner.comm.as_deref().unwrap_or("unknown");
            let pid = owner
                .pid
                .map_or_else(|| "unknown".to_string(), |pid| pid.to_string());
            println!("- {name}: dbus-owner, pid {pid}");
        }
    }
}

pub(super) async fn is_unit_active(unit: &str) -> bool {
    // systemctl exit code is enough here, stdout is not needed
    let command_str = format!("systemctl --user is-active --quiet {unit}");
    let command_snip = util::log_snippet(&command_str);
    let Ok(mut command) = system_tools::tokio_command("systemctl") else {
        return false;
    };
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

pub(super) async fn run_command_output(
    command: &mut tokio::process::Command,
    command_snip: &str,
) -> Option<std::process::Output> {
    // Shared timeout wrapper prevents trial mode from hanging on slow commands
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

pub(super) async fn run_command_status(
    command: &mut tokio::process::Command,
    command_snip: &str,
) -> Option<std::process::ExitStatus> {
    // Status wrapper mirrors output wrapper and keeps logging consistent
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

async fn pgrep_exact(name: &str) -> Vec<u32> {
    // pgrep -x avoids partial-name matches
    let command_str = format!("pgrep -x {name}");
    let command_snip = util::log_snippet(&command_str);
    let Ok(mut command) = system_tools::tokio_command("pgrep") else {
        return Vec::new();
    };
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
    // Prefer /proc to avoid spawning a process for a single field
    let path = format!("/proc/{pid}/comm");
    if let Ok(contents) = fs::read_to_string(path).await {
        let comm = contents.trim().to_string();
        if !comm.is_empty() {
            return Some(comm);
        }
    }
    let command_str = format!("ps -p {pid} -o comm=");
    let command_snip = util::log_snippet(&command_str);
    let mut command = system_tools::tokio_command("ps").ok()?;
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
    // Use /proc/cmdline to preserve argument boundaries and quoting
    let path = format!("/proc/{pid}/cmdline");
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
    let mut command = system_tools::tokio_command("ps").ok()?;
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
    // split_whitespace fallback is less exact than /proc but better than missing args
    let parts = fallback
        .split_whitespace()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

#[cfg(test)]
#[path = "tests/owner.rs"]
mod tests;
