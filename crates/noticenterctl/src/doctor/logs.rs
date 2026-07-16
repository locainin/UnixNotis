//! Bounded log acquisition with honest status-only fallbacks

use std::env;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use unixnotis_core::util::sanitize_inline_display_text;

use crate::debug_logs::journal::{daemon_unit_from_env, recent_args};
use crate::system_tools;

use super::config::redact_home_text;
use super::model::{DoctorCheck, DoctorLogResult, DoctorLogSource, DoctorSeverity};
use super::service::SelectedServiceManager;

const JOURNAL_LINE_LIMIT: usize = 30;
const JOURNAL_TOTAL_BYTE_LIMIT: usize = 32 * 1024;
const JOURNAL_LINE_CHAR_LIMIT: usize = 512;
const JOURNAL_TIMEOUT: Duration = Duration::from_secs(3);
const LOGGING_HINT: &str =
    "Reproduce the problem by running unixnotis-daemon in a terminal, or configure logging through the service manager";

pub(super) async fn collect_logs(
    selected: SelectedServiceManager,
    verbose: bool,
) -> (DoctorLogResult, DoctorCheck) {
    // Only the installed systemd backend provides a configured persistent source
    match selected {
        SelectedServiceManager::Managed(
            unixnotis_core::service_manager::ServiceManagerKind::Systemd,
        ) => collect_systemd_logs(verbose).await,
        SelectedServiceManager::Managed(manager) => unavailable_manager_logs(manager.label()),
        SelectedServiceManager::Manual => unavailable_logs(
            DoctorLogSource::Manual,
            "Manual launches do not provide a UnixNotis-managed persistent log source",
        ),
        SelectedServiceManager::Unknown => unavailable_logs(
            DoctorLogSource::Unknown,
            "The active service manager is unknown, so no persistent log source can be selected safely",
        ),
    }
}

async fn collect_systemd_logs(verbose: bool) -> (DoctorLogResult, DoctorCheck) {
    // Journal contents are opt-in because they may include application metadata
    if !verbose {
        return unavailable_logs(
            DoctorLogSource::SystemdJournal,
            "Journal collection is disabled unless doctor runs with --verbose",
        );
    }
    // Reuse the debug-log unit contract so both commands inspect the same service
    let unit = daemon_unit_from_env(|key| env::var(key));
    match read_recent_journal(&unit).await {
        Ok(lines) => {
            let check = DoctorCheck::new(
                "logs.availability",
                "Logs",
                DoctorSeverity::Pass,
                format!("Collected {} bounded journal line(s)", lines.len()),
            )
            .details(format!("Source: systemd journal\nUnit: {unit}"));
            (
                DoctorLogResult::Collected {
                    source: DoctorLogSource::SystemdJournal,
                    lines,
                },
                check,
            )
        }
        Err(error) => {
            let result = DoctorLogResult::Unavailable {
                source: DoctorLogSource::SystemdJournal,
                reason: error.clone(),
                hint: Some(LOGGING_HINT.to_string()),
            };
            let check = DoctorCheck::new(
                "logs.availability",
                "Logs",
                DoctorSeverity::Note,
                "Persistent logs are unavailable",
            )
            .details(error)
            .hint(LOGGING_HINT);
            (result, check)
        }
    }
}

fn unavailable_manager_logs(manager: &str) -> (DoctorLogResult, DoctorCheck) {
    // Backend identity remains explicit even when no log source exists
    let source = match manager {
        "dinit" => DoctorLogSource::Dinit,
        "runit" => DoctorLogSource::Runit,
        "s6-rc" => DoctorLogSource::S6Rc,
        _ => DoctorLogSource::Unknown,
    };
    unavailable_logs(
        source,
        &format!(
            "Persistent logs unavailable: the installed UnixNotis {manager} service does not configure a manager logger"
        ),
    )
}

fn unavailable_logs(source: DoctorLogSource, reason: &str) -> (DoctorLogResult, DoctorCheck) {
    // Unavailable logging is informational and never changes doctor exit status
    let result = DoctorLogResult::Unavailable {
        source,
        reason: reason.to_string(),
        hint: Some(LOGGING_HINT.to_string()),
    };
    let check = DoctorCheck::new(
        "logs.availability",
        "Logs",
        DoctorSeverity::Note,
        "Persistent logs are unavailable",
    )
    .details(reason)
    .hint(LOGGING_HINT);
    (result, check)
}

async fn read_recent_journal(unit: &str) -> Result<Vec<String>, String> {
    // Fixed trusted lookup prevents a PATH entry from impersonating journalctl
    let path = system_tools::trusted_program_path("journalctl")
        .ok_or_else(|| "journalctl was not found in trusted system directories".to_string())?;
    let mut command = Command::new(path);
    command
        .args(recent_args(unit, JOURNAL_LINE_LIMIT))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    // The outer timeout covers spawn, streaming, termination, and final wait
    let work = async move {
        let mut child = command
            .spawn()
            .map_err(|error| format!("journalctl could not start: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "journalctl stdout was unavailable".to_string())?;
        let mut bytes = Vec::new();
        // Read one extra byte so exact-limit output remains distinguishable
        stdout
            .take((JOURNAL_TOTAL_BYTE_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| format!("journalctl output could not be read: {error}"))?;
        let was_truncated = journal_output_exceeds_limit(bytes.len());
        if was_truncated {
            // Stop producers that ignore the requested line window once the byte cap is full
            child
                .kill()
                .await
                .map_err(|error| format!("oversized journalctl process could not stop: {error}"))?;
            bytes.truncate(JOURNAL_TOTAL_BYTE_LIMIT);
        }
        let status = child
            .wait()
            .await
            .map_err(|error| format!("journalctl status could not be read: {error}"))?;
        // A deliberate kill after reaching the cap still produced a valid bounded report
        if !status.success() && !was_truncated {
            return Err(format!("journalctl exited with status {status}"));
        }
        Ok(sanitize_journal(&bytes))
    };

    tokio::time::timeout(JOURNAL_TIMEOUT, work)
        .await
        .map_err(|_elapsed| "journalctl collection timed out".to_string())?
}

const fn journal_output_exceeds_limit(byte_count: usize) -> bool {
    byte_count > JOURNAL_TOTAL_BYTE_LIMIT
}

fn sanitize_journal(bytes: &[u8]) -> Vec<String> {
    // Apply line and character caps after terminal-control sanitization
    String::from_utf8_lossy(bytes)
        .lines()
        .take(JOURNAL_LINE_LIMIT)
        .map(sanitize_inline_display_text)
        .map(|line| redact_home_text(&line))
        .map(|line| line.chars().take(JOURNAL_LINE_CHAR_LIMIT).collect())
        .collect()
}

#[cfg(test)]
#[path = "tests/logs.rs"]
mod tests;
