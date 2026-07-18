//! Bounded systemd journal acquisition

use std::env;
use std::process::Stdio;
use std::time::Duration;

use crate::debug_logs::journal::{daemon_unit_from_env, recent_args};
use crate::system_tools;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::super::report::safe_doctor_text;
use super::super::report::{DoctorLogResult, DoctorLogSource};
use super::sanitize::{
    journal_output_exceeds_limit, sanitize_journal, JOURNAL_LINE_LIMIT, JOURNAL_TOTAL_BYTE_LIMIT,
};
const JOURNAL_TIMEOUT: Duration = Duration::from_secs(3);
const LOGGING_HINT: &str =
    "Reproduce the problem by running unixnotis-daemon in a terminal, or configure logging through the service manager";

pub(super) async fn collect_systemd_logs(verbose: bool) -> DoctorLogResult {
    // Journal contents are opt-in because they may include application metadata
    if !verbose {
        return super::routing::unavailable_logs(
            DoctorLogSource::SystemdJournal,
            "Journal collection is disabled unless doctor runs with --verbose",
        );
    }
    // Reuse the debug-log unit contract so both commands inspect the same service
    let unit = match daemon_unit_from_env(|key| env::var(key)) {
        Ok(unit) => unit,
        Err(error) => {
            return DoctorLogResult::Unavailable {
                source: DoctorLogSource::SystemdJournal,
                reason: safe_doctor_text(&error.to_string()),
                hint: Some("Correct UNIXNOTIS_DAEMON_UNIT and run doctor again".to_string()),
            };
        }
    };
    match read_recent_journal(&unit).await {
        Ok(collection) => {
            let truncated = collection.was_truncated();
            DoctorLogResult::Collected {
                source: DoctorLogSource::SystemdJournal,
                lines: collection.lines,
                truncated,
                line_limit: JOURNAL_LINE_LIMIT,
                byte_limit: JOURNAL_TOTAL_BYTE_LIMIT,
            }
        }
        Err(error) => DoctorLogResult::Unavailable {
            source: DoctorLogSource::SystemdJournal,
            reason: safe_doctor_text(&error),
            hint: Some(LOGGING_HINT.to_string()),
        },
    }
}

pub(super) async fn read_recent_journal(
    unit: &str,
) -> Result<super::sanitize::JournalCollection, String> {
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
        let mut collection = sanitize_journal(&bytes);
        // Keep the byte-cap reason separate from line and character clipping
        //
        // This makes the boundary observable even when oversized output also
        // crosses a smaller presentation limit
        collection.byte_truncated = was_truncated;
        Ok(collection)
    };

    tokio::time::timeout(JOURNAL_TIMEOUT, work)
        .await
        .map_err(|_elapsed| "journalctl collection timed out".to_string())?
}
