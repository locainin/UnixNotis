//! Journalctl follower used when the panel opens in debug mode.

use anyhow::{anyhow, Context, Result};
use std::env;
use std::process::Command as ProcCommand;
use std::process::Stdio;

const DEFAULT_DAEMON_UNIT: &str = "unixnotis-daemon.service";

pub(crate) fn follow_debug_logs() -> Result<()> {
    if !journalctl_is_available() {
        return Err(anyhow!(
            "journalctl is not available; run unixnotis-daemon in a terminal to watch logs directly"
        ));
    }
    let unit =
        env::var("UNIXNOTIS_DAEMON_UNIT").unwrap_or_else(|_| DEFAULT_DAEMON_UNIT.to_string());
    if !journal_has_user_unit_logs(&unit)? {
        return Err(anyhow!(
            "no user journal stream for {}; debug panel open will continue without log follow",
            unit
        ));
    }

    // Follow the user-level systemd unit so the output matches the active session.
    let status = ProcCommand::new("journalctl")
        .args(["--user", "-f", "-u", &unit, "-o", "cat"])
        .status()
        .with_context(|| format!("start journalctl follow for {unit}"))?;

    if status.success() {
        Ok(())
    } else {
        // Propagate a clear failure when the subprocess exits non-zero.
        Err(anyhow!("journalctl exited with status {}", status))
    }
}

fn journalctl_is_available() -> bool {
    ProcCommand::new("journalctl")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn journal_has_user_unit_logs(unit: &str) -> Result<bool> {
    let status = ProcCommand::new("journalctl")
        .args(["--user", "--no-pager", "-n", "1", "-u", unit, "-o", "cat"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("check journal availability for {unit}"))?;
    Ok(status.success())
}
