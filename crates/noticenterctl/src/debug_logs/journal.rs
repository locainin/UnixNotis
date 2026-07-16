use anyhow::{anyhow, Context, Result};
use std::env;
use std::process::Stdio;
use thiserror::Error;

use crate::system_tools;

const DEFAULT_DAEMON_UNIT: &str = "unixnotis-daemon.service";
const SYSTEMD_UNIT_NAME_LIMIT: usize = 255;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("UNIXNOTIS_DAEMON_UNIT must be a valid service unit name")]
pub struct InvalidDaemonUnit;

pub fn daemon_unit_from_env(
    get_var: impl FnOnce(&str) -> Result<String, env::VarError>,
) -> std::result::Result<String, InvalidDaemonUnit> {
    // Keep debug logging and doctor aligned on the same optional unit override
    let unit =
        get_var("UNIXNOTIS_DAEMON_UNIT").unwrap_or_else(|_error| DEFAULT_DAEMON_UNIT.to_string());
    validate_daemon_unit(&unit)?;
    Ok(unit)
}

pub fn validate_daemon_unit(unit: &str) -> std::result::Result<(), InvalidDaemonUnit> {
    // systemd limits complete unit names to 255 bytes and requires a typed suffix
    if unit.is_empty()
        || unit.len() > SYSTEMD_UNIT_NAME_LIMIT
        || unit.starts_with('-')
        || !unit.ends_with(".service")
    {
        return Err(InvalidDaemonUnit);
    }
    // Unit names use a narrow ASCII alphabet plus one template instance separator
    let mut instance_separator_seen = false;
    for character in unit.chars() {
        let valid = character.is_ascii_alphanumeric()
            || matches!(character, ':' | '-' | '_' | '.' | '\\')
            || (character == '@' && !std::mem::replace(&mut instance_separator_seen, true));
        if !valid {
            return Err(InvalidDaemonUnit);
        }
    }
    Ok(())
}

pub fn recent_args(unit: &str, line_limit: usize) -> Vec<String> {
    // Argument values are joined to their options so unit names cannot become flags
    vec![
        "--user".to_string(),
        "--no-pager".to_string(),
        format!("--lines={line_limit}"),
        format!("--unit={unit}"),
        "-o".to_string(),
        "cat".to_string(),
    ]
}

pub(super) fn follow_args(unit: &str) -> Vec<String> {
    // Follow mode intentionally omits a line cap because the caller owns its lifetime
    vec![
        "--user".to_string(),
        "-f".to_string(),
        format!("--unit={unit}"),
        "-o".to_string(),
        "cat".to_string(),
    ]
}

pub(super) fn probe_args(unit: &str) -> Vec<String> {
    // One quiet entry is enough to prove the selected user journal has content
    vec![
        "--user".to_string(),
        "--no-pager".to_string(),
        "--lines=1".to_string(),
        format!("--unit={unit}"),
        "-o".to_string(),
        "cat".to_string(),
    ]
}

pub(super) fn journalctl_is_available() -> bool {
    // Resolve through the trusted system-tool policy rather than inherited PATH entries
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
    // Probe output is discarded because only the trusted process status matters here
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
