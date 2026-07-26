//! Sender metadata helpers for incoming Notify/CloseNotification calls
//!
//! Sender details are optional and best-effort, so failures here must not reject
//! notification delivery

use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::Connection;

use super::sender_cache::SenderMetadataCache;
use super::{executable_evidence_for_pid, FileIdentity};

#[derive(Debug, Clone, Default)]
pub(in crate::daemon) struct SenderMetadata {
    // Unique bus sender name (:1.x) used for ownership checks
    pub(in crate::daemon::notifications) sender_name: Option<String>,
    // Process id is paired with start time so reused pids do not inherit ownership
    pub(in crate::daemon::notifications) sender_pid: Option<u32>,
    // Linux start time identifies one concrete process lifetime
    pub(in crate::daemon::notifications) sender_start_time: Option<u64>,
    // Executable path is presentation-only evidence for diagnostics and source labels
    pub(in crate::daemon::notifications) sender_executable: Option<String>,
    // Device and inode bind policy to the open running executable rather than its basename
    pub(in crate::daemon::notifications) sender_executable_identity: Option<FileIdentity>,
}

pub(in crate::daemon) async fn resolve_sender_metadata(
    cache: &SenderMetadataCache,
    connection: &Connection,
    header: &Header<'_>,
) -> SenderMetadata {
    // Sender lookup failures are non-fatal and should degrade to "unknown"
    let sender_name = header.sender().map(|sender| sender.as_str().to_string());
    let Some(sender_name_str) = sender_name.as_deref() else {
        return SenderMetadata {
            sender_name,
            sender_pid: None,
            sender_start_time: None,
            sender_executable: None,
            sender_executable_identity: None,
        };
    };

    // Unique names are stable for one bus connection and safe cache identities
    if let Some(metadata) = cache.get(sender_name_str) {
        return metadata;
    }
    let cache_key = sender_name_str.to_string();

    let Ok(bus_name) = zbus::names::BusName::try_from(sender_name_str) else {
        return SenderMetadata {
            sender_name,
            sender_pid: None,
            sender_start_time: None,
            sender_executable: None,
            sender_executable_identity: None,
        };
    };

    let Ok(proxy) = DBusProxy::new(connection).await else {
        return SenderMetadata {
            sender_name,
            sender_pid: None,
            sender_start_time: None,
            sender_executable: None,
            sender_executable_identity: None,
        };
    };

    // PID and executable come from the bus owner, not caller-provided payload fields
    let sender_pid = proxy.get_connection_unix_process_id(bus_name).await.ok();
    let (sender_start_time, executable_evidence) = sender_pid.map_or((None, None), |pid| {
        let start_before = read_process_start_time(pid);
        let evidence = executable_evidence_for_pid(pid);
        let start_after = read_process_start_time(pid);
        stable_process_evidence(start_before, evidence, start_after)
    });
    let sender_executable = executable_evidence
        .as_ref()
        .map(|evidence| evidence.canonical_path.display().to_string());
    let sender_executable_identity = executable_evidence.map(|evidence| evidence.identity);

    let metadata = SenderMetadata {
        sender_name,
        sender_pid,
        sender_start_time,
        sender_executable,
        sender_executable_identity,
    };
    // Failed lookups remain retryable instead of becoming persistent unknown identities
    if metadata.sender_start_time.is_some() && metadata.sender_executable_identity.is_some() {
        cache.insert(cache_key, metadata.clone());
    }
    metadata
}

#[cfg(target_os = "linux")]
#[cfg(test)]
async fn read_process_executable_path(pid: u32) -> Option<std::path::PathBuf> {
    executable_evidence_for_pid(pid).map(|evidence| evidence.canonical_path)
}

#[cfg(target_os = "linux")]
fn read_process_start_time(pid: u32) -> Option<u64> {
    // /proc/<pid>/stat keeps the process lifetime tick count in field 22
    let path = format!("/proc/{pid}/stat");
    let contents = std::fs::read_to_string(path).ok()?;
    parse_process_start_time(&contents)
}

#[cfg(not(target_os = "linux"))]
#[cfg(test)]
async fn read_process_executable_path(_pid: u32) -> Option<std::path::PathBuf> {
    // On other platforms this metadata is optional
    None
}

#[cfg(not(target_os = "linux"))]
fn read_process_start_time(_pid: u32) -> Option<u64> {
    // Non-Linux builds fall back to bus-name ownership only
    None
}

fn stable_process_evidence<T>(
    start_before: Option<u64>,
    evidence: Option<T>,
    start_after: Option<u64>,
) -> (Option<u64>, Option<T>) {
    // Both lifetime reads must name the same process before executable evidence is trusted
    if start_before.is_some() && start_before == start_after {
        (start_before, evidence)
    } else {
        (None, None)
    }
}

#[cfg(target_os = "linux")]
fn parse_process_start_time(stat: &str) -> Option<u64> {
    // The comm field is wrapped in parentheses and may contain spaces
    let end = stat.rfind(')')?;
    let remainder = stat.get(end + 2..)?;
    // Field 3 starts here, so field 22 lives at index 19
    let start_time = remainder.split_whitespace().nth(19)?;
    start_time.parse().ok()
}

#[cfg(test)]
#[path = "tests/sender.rs"]
mod tests;
