use anyhow::{anyhow, Context, Result};
use std::env;
use std::process::Stdio;

use crate::system_tools;

const DEFAULT_DAEMON_UNIT: &str = "unixnotis-daemon.service";

pub fn daemon_unit_from_env(get_var: impl FnOnce(&str) -> Result<String, env::VarError>) -> String {
    get_var("UNIXNOTIS_DAEMON_UNIT").unwrap_or_else(|_| DEFAULT_DAEMON_UNIT.to_string())
}

pub fn recent_args(unit: &str, line_limit: usize) -> Vec<String> {
    vec![
        "--user".to_string(),
        "--no-pager".to_string(),
        "-n".to_string(),
        line_limit.to_string(),
        "-u".to_string(),
        unit.to_string(),
        "-o".to_string(),
        "cat".to_string(),
    ]
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
    let Ok(mut command) = system_tools::command("journalctl") else {
        return false;
    };
    command
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub(super) fn journal_has_user_unit_logs(unit: &str) -> Result<bool> {
    let mut command = system_tools::command("journalctl").context("resolve trusted journalctl")?;
    let status = command
        .args(probe_args(unit))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("check journal availability for {unit}"))?;
    Ok(status.success())
}

pub(super) fn follow_user_unit_logs(unit: &str) -> Result<()> {
    // Follow the user-level systemd unit so the output matches the active session
    let mut command = system_tools::command("journalctl").context("resolve trusted journalctl")?;
    let status = command
        .args(follow_args(unit))
        .status()
        .with_context(|| format!("start journalctl follow for {unit}"))?;

    if status.success() {
        Ok(())
    } else {
        // Propagate a clear failure when the subprocess exits non-zero
        Err(anyhow!("journalctl exited with status {status}"))
    }
}
