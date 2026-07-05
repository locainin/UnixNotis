use anyhow::{anyhow, Context, Result};
use std::env;
use std::process::{Command as ProcCommand, Stdio};

const DEFAULT_DAEMON_UNIT: &str = "unixnotis-daemon.service";

pub(super) fn daemon_unit_from_env(
    get_var: impl FnOnce(&str) -> Result<String, env::VarError>,
) -> String {
    get_var("UNIXNOTIS_DAEMON_UNIT").unwrap_or_else(|_| DEFAULT_DAEMON_UNIT.to_string())
}

pub(super) fn follow_args(unit: &str) -> Vec<String> {
    vec![
        "--user".to_string(),
        "-f".to_string(),
        "-u".to_string(),
        unit.to_string(),
        "-o".to_string(),
        "cat".to_string(),
    ]
}

pub(super) fn probe_args(unit: &str) -> Vec<String> {
    vec![
        "--user".to_string(),
        "--no-pager".to_string(),
        "-n".to_string(),
        "1".to_string(),
        "-u".to_string(),
        unit.to_string(),
        "-o".to_string(),
        "cat".to_string(),
    ]
}

pub(super) fn journalctl_is_available() -> bool {
    ProcCommand::new("journalctl")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(super) fn journal_has_user_unit_logs(unit: &str) -> Result<bool> {
    let status = ProcCommand::new("journalctl")
        .args(probe_args(unit))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("check journal availability for {unit}"))?;
    Ok(status.success())
}

pub(super) fn follow_user_unit_logs(unit: &str) -> Result<()> {
    // Follow the user-level systemd unit so the output matches the active session
    let status = ProcCommand::new("journalctl")
        .args(follow_args(unit))
        .status()
        .with_context(|| format!("start journalctl follow for {unit}"))?;

    if status.success() {
        Ok(())
    } else {
        // Propagate a clear failure when the subprocess exits non-zero
        Err(anyhow!("journalctl exited with status {}", status))
    }
}
