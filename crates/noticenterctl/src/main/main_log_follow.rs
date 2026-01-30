//! Journalctl follower used when the panel opens in debug mode.

use anyhow::{anyhow, Context, Result};
use std::process::Command as ProcCommand;

pub(crate) fn follow_debug_logs() -> Result<()> {
    // Follow the user-level systemd unit so the output matches the active session.
    let status = ProcCommand::new("journalctl")
        .args([
            "--user",
            "-f",
            "-u",
            "unixnotis-daemon.service",
            "-o",
            "cat",
        ])
        .status()
        .context("start journalctl follow")?;

    if status.success() {
        Ok(())
    } else {
        // Propagate a clear failure when the subprocess exits non-zero.
        Err(anyhow!("journalctl exited with status {}", status))
    }
}
