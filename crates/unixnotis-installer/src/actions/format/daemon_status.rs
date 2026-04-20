//! Formatting helpers for detection summaries.

use crate::detect::DetectedDaemon;

pub fn summarize_owner(owner: &Option<crate::detect::OwnerInfo>) -> String {
    match owner {
        Some(info) => {
            // Keep missing fields readable instead of showing an empty tuple
            let name = info.comm.as_deref().unwrap_or("unknown");
            let pid = info
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            format!("{} (pid {})", name, pid)
        }
        None => "none detected".to_string(),
    }
}

pub fn format_daemon_status(daemon: &DetectedDaemon) -> String {
    let mut status = Vec::new();
    // Add only the signals that are true so the summary stays short in the UI
    if daemon.is_owner {
        status.push("dbus-owner".to_string());
    }
    if daemon.systemd_active {
        status.push("systemd-active".to_string());
    }
    if let Some(err) = daemon.systemd_error.as_ref() {
        status.push(format!("systemd-error: {}", err));
    }
    if !daemon.running_pids.is_empty() {
        // Join all live pids into one field so callers do not need to format them again
        let ids = daemon
            .running_pids
            .iter()
            .map(|pid| pid.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        status.push(format!("pid {}", ids));
    }
    if status.is_empty() {
        // Fall back to one stable string when no daemon signal is present
        status.push("not running".to_string());
    }
    status.join(", ")
}
