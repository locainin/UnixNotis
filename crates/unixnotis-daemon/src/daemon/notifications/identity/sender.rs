//! Sender metadata helpers for incoming Notify/CloseNotification calls
//!
//! Sender details are optional and best-effort, so failures here must not reject
//! notification delivery

use std::fs::File;
use std::future::Future;
use std::io::Read;

use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::Connection;

use super::sender_cache::SenderMetadataCache;
use super::{executable_evidence_for_pid, FileIdentity};
use crate::daemon::notifications::identity::desktop_index::InstallProvenance;

const MAX_PROCESS_CMDLINE_BYTES: u64 = 128 * 1024;
const MAX_PROCESS_ARGUMENTS: usize = 256;
const MAX_PROCESS_ANCESTORS: usize = 8;
pub(in crate::daemon) const SENDER_CREDENTIAL_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(500);

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub(in crate::daemon::notifications) enum CommandLineQuality {
    Structured,
    RewrittenProcessTitle,
    Truncated,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::daemon::notifications) struct CommandLineEvidence {
    pub(in crate::daemon::notifications) argv: Vec<Vec<u8>>,
    pub(in crate::daemon::notifications) quality: CommandLineQuality,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub(in crate::daemon) enum SenderMetadataStatus {
    Complete,
    MissingSenderName,
    CredentialLookupFailed,
    CredentialLookupTimedOut,
    #[default]
    ProcessEvidenceUnavailable,
}

/// Stable executable evidence for one same-user process ancestor
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications) struct ProcessLineageEvidence {
    pub(in crate::daemon::notifications) pid: u32,
    pub(in crate::daemon::notifications) start_time: u64,
    pub(in crate::daemon::notifications) uid: u32,
    pub(in crate::daemon::notifications) executable: String,
    pub(in crate::daemon::notifications) executable_identity: FileIdentity,
}

#[derive(Debug, Clone, Default)]
pub(in crate::daemon) struct SenderMetadata {
    // Unique bus sender name (:1.x) used for ownership checks
    pub(in crate::daemon::notifications) sender_name: Option<String>,
    // Process id is paired with start time so reused pids do not inherit ownership
    pub(in crate::daemon::notifications) sender_pid: Option<u32>,
    // Linux start time identifies one concrete process lifetime
    pub(in crate::daemon::notifications) sender_start_time: Option<u64>,
    // The bus credential is used to bound process-lineage inspection
    pub(in crate::daemon::notifications) sender_uid: Option<u32>,
    // Executable path is presentation-only evidence for diagnostics and source labels
    pub(in crate::daemon::notifications) sender_executable: Option<String>,
    // Device and inode bind policy to the open running executable rather than its basename
    pub(in crate::daemon::notifications) sender_executable_identity: Option<FileIdentity>,
    // Package or bundle ownership is supporting evidence for helper and conflict decisions
    pub(in crate::daemon::notifications) install_provenance: InstallProvenance,
    // Quality is explicit because processes may rewrite the visible procfs argument memory
    pub(in crate::daemon::notifications) command_line: CommandLineEvidence,
    // Ancestors remain supporting evidence and never grant actions by themselves
    pub(in crate::daemon::notifications) ancestors: Vec<ProcessLineageEvidence>,
    // The stage that failed remains visible to diagnostics instead of becoming generic unknown
    pub(in crate::daemon::notifications) status: SenderMetadataStatus,
}

fn metadata_with_status(
    sender_name: Option<String>,
    status: SenderMetadataStatus,
) -> SenderMetadata {
    SenderMetadata {
        sender_name,
        status,
        ..SenderMetadata::default()
    }
}

fn metadata_from_credentials(
    sender_name: Option<String>,
    process_id: Option<u32>,
    user_id: Option<u32>,
) -> SenderMetadata {
    let status = if user_id.is_some() && process_id.is_some() {
        SenderMetadataStatus::ProcessEvidenceUnavailable
    } else {
        SenderMetadataStatus::CredentialLookupFailed
    };
    SenderMetadata {
        sender_name,
        sender_pid: process_id,
        // Process start time turns the reusable pid into one lifetime identity
        sender_start_time: process_id.and_then(read_process_start_time),
        sender_uid: user_id,
        status,
        ..SenderMetadata::default()
    }
}

pub(in crate::daemon) async fn resolve_sender_metadata(
    cache: &SenderMetadataCache,
    connection: &Connection,
    header: &Header<'_>,
) -> SenderMetadata {
    // Sender lookup failures are non-fatal and should degrade to "unknown"
    let sender_name = header.sender().map(|sender| sender.as_str().to_string());
    let Some(sender_name_str) = sender_name.as_deref() else {
        return metadata_with_status(sender_name, SenderMetadataStatus::MissingSenderName);
    };

    // Unique names are stable for one bus connection and safe cache identities
    if let Some(metadata) = cache.get(sender_name_str) {
        return metadata;
    }
    let cache_key = sender_name_str.to_string();

    let Ok(bus_name) = zbus::names::BusName::try_from(sender_name_str) else {
        return metadata_with_status(sender_name, SenderMetadataStatus::CredentialLookupFailed);
    };

    let Ok(proxy) = DBusProxy::new(connection).await else {
        return metadata_with_status(sender_name, SenderMetadataStatus::CredentialLookupFailed);
    };

    // Credentials are the only asynchronous pre-attribution work
    let (connection_user_id, connection_process_id) = resolve_connection_credentials(
        proxy.get_connection_unix_user(bus_name.clone()),
        proxy.get_connection_unix_process_id(bus_name),
    )
    .await;
    let metadata =
        metadata_from_credentials(sender_name, connection_process_id, connection_user_id);
    // Credentials remain cached while process evidence is refreshed inside the worker
    if metadata.sender_pid.is_some() && metadata.sender_uid.is_some() {
        cache.insert(cache_key, metadata.clone());
    }
    metadata
}

async fn resolve_connection_credentials<U, P, EU, EP>(
    user_id: U,
    process_id: P,
) -> (Option<u32>, Option<u32>)
where
    U: Future<Output = Result<u32, EU>>,
    P: Future<Output = Result<u32, EP>>,
{
    let (user_id, process_id) = tokio::join!(user_id, process_id);
    (user_id.ok(), process_id.ok())
}

pub(super) fn refresh_sender_security_evidence(metadata: &SenderMetadata) -> SenderMetadata {
    let mut refreshed = metadata.clone();
    let Some(pid) = metadata.sender_pid else {
        return refreshed;
    };
    if metadata.sender_uid.is_none()
        && matches!(
            metadata.status,
            SenderMetadataStatus::CredentialLookupFailed
                | SenderMetadataStatus::CredentialLookupTimedOut
                | SenderMetadataStatus::MissingSenderName
        )
    {
        refreshed.status = SenderMetadataStatus::ProcessEvidenceUnavailable;
        return refreshed;
    }
    // Fresh credential metadata has no expected lifetime yet; capture it in this worker
    let expected_start = metadata
        .sender_start_time
        .or_else(|| read_process_start_time(pid));
    let Some(expected_start) = expected_start else {
        refreshed.status = SenderMetadataStatus::ProcessEvidenceUnavailable;
        return refreshed;
    };

    // Refresh every process-derived field before a security-sensitive association decision
    let start_before = read_process_start_time(pid);
    let executable = executable_evidence_for_pid(pid);
    let command_line = read_process_cmdline(pid, executable.as_ref());
    let start_after = read_process_start_time(pid);
    if !process_lifetime_matches(start_before, expected_start, start_after) {
        // Stale cache entries retain bus context but lose all application identity authority
        refreshed.sender_start_time = None;
        refreshed.sender_uid = None;
        refreshed.sender_executable = None;
        refreshed.sender_executable_identity = None;
        refreshed.command_line = CommandLineEvidence::default();
        refreshed.ancestors.clear();
        refreshed.status = SenderMetadataStatus::ProcessEvidenceUnavailable;
        return refreshed;
    }

    if metadata
        .sender_uid
        .is_some_and(|uid| read_process_real_uid(pid) != Some(uid))
    {
        refreshed.sender_start_time = None;
        refreshed.sender_uid = None;
        refreshed.sender_executable = None;
        refreshed.sender_executable_identity = None;
        refreshed.command_line = CommandLineEvidence::default();
        refreshed.ancestors.clear();
        refreshed.status = SenderMetadataStatus::ProcessEvidenceUnavailable;
        return refreshed;
    }

    refreshed.sender_executable = executable
        .as_ref()
        .map(|evidence| evidence.canonical_path.display().to_string());
    refreshed.sender_executable_identity = executable.map(|evidence| evidence.identity);
    refreshed.command_line = command_line;
    refreshed.ancestors = metadata
        .sender_uid
        .map_or_else(Vec::new, |uid| collect_process_lineage(pid, uid));
    refreshed.sender_start_time = Some(expected_start);
    refreshed.status = if refreshed.sender_executable_identity.is_some() {
        SenderMetadataStatus::Complete
    } else {
        SenderMetadataStatus::ProcessEvidenceUnavailable
    };
    refreshed
}

fn process_lifetime_matches(
    start_before: Option<u64>,
    expected_start: u64,
    start_after: Option<u64>,
) -> bool {
    start_before == Some(expected_start) && start_after == Some(expected_start)
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn read_process_start_time(pid: u32) -> Option<u64> {
    // /proc/<pid>/stat keeps the process lifetime tick count in field 22
    let path = format!("/proc/{pid}/stat");
    let contents = std::fs::read_to_string(path).ok()?;
    parse_process_stat(&contents).map(|stat| stat.start_time)
}

#[cfg(target_os = "linux")]
fn read_process_real_uid(pid: u32) -> Option<u32> {
    let path = format!("/proc/{pid}/status");
    std::fs::read_to_string(path)
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

#[cfg(target_os = "linux")]
fn collect_process_lineage(pid: u32, uid: u32) -> Vec<ProcessLineageEvidence> {
    let Some(sender_stat) = read_process_stat(pid) else {
        return Vec::new();
    };
    let mut parent_pid = sender_stat.parent_pid;
    let mut lineage = Vec::new();

    for _ in 0..MAX_PROCESS_ANCESTORS {
        if parent_pid <= 1 || read_process_real_uid(parent_pid) != Some(uid) {
            break;
        }
        let Some(before) = read_process_stat(parent_pid) else {
            break;
        };
        // Crossing a login session is outside the sender's application launch scope
        if before.session_id != sender_stat.session_id {
            break;
        }
        let Some(executable) = executable_evidence_for_pid(parent_pid) else {
            break;
        };
        let Some(after) = read_process_stat(parent_pid) else {
            break;
        };
        if before != after {
            break;
        }
        lineage.push(ProcessLineageEvidence {
            pid: parent_pid,
            start_time: before.start_time,
            uid,
            executable: executable.canonical_path.display().to_string(),
            executable_identity: executable.identity,
        });
        parent_pid = before.parent_pid;
    }
    lineage
}

#[cfg(target_os = "linux")]
fn read_process_cmdline(
    pid: u32,
    executable: Option<&super::executable::ExecutableEvidence>,
) -> CommandLineEvidence {
    let path = format!("/proc/{pid}/cmdline");
    let mut bytes = Vec::new();
    let Some(file) = File::open(path).ok() else {
        return CommandLineEvidence::default();
    };
    if file
        .take(MAX_PROCESS_CMDLINE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return CommandLineEvidence::default();
    }
    if bytes.len() as u64 > MAX_PROCESS_CMDLINE_BYTES {
        return CommandLineEvidence {
            argv: Vec::new(),
            quality: CommandLineQuality::Truncated,
        };
    }
    let Some(argv) = parse_process_cmdline(bytes) else {
        return CommandLineEvidence::default();
    };
    classify_command_line(argv, executable)
}

#[cfg(target_os = "linux")]
fn parse_process_cmdline(mut bytes: Vec<u8>) -> Option<Vec<Vec<u8>>> {
    if bytes.is_empty()
        || bytes.len() as u64 > MAX_PROCESS_CMDLINE_BYTES
        || bytes.last() != Some(&0)
    {
        return None;
    }
    bytes.pop();
    let arguments = bytes
        .split(|byte| *byte == 0)
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    (!arguments.is_empty() && arguments.len() <= MAX_PROCESS_ARGUMENTS).then_some(arguments)
}

#[cfg(not(target_os = "linux"))]
pub(in crate::daemon) fn read_process_start_time(_pid: u32) -> Option<u64> {
    // Non-Linux builds fall back to bus-name ownership only
    None
}

#[cfg(not(target_os = "linux"))]
fn read_process_real_uid(_pid: u32) -> Option<u32> {
    None
}

#[cfg(not(target_os = "linux"))]
fn collect_process_lineage(_pid: u32, _uid: u32) -> Vec<ProcessLineageEvidence> {
    Vec::new()
}

#[cfg(not(target_os = "linux"))]
fn read_process_cmdline(
    _pid: u32,
    _executable: Option<&super::executable::ExecutableEvidence>,
) -> CommandLineEvidence {
    CommandLineEvidence::default()
}

fn classify_command_line(
    argv: Vec<Vec<u8>>,
    executable: Option<&super::executable::ExecutableEvidence>,
) -> CommandLineEvidence {
    let rewritten = executable.is_some_and(|executable| {
        argv.as_slice().first().is_some_and(|value| {
            let prefix = executable.canonical_path.as_os_str().as_encoded_bytes();
            value.starts_with(prefix) && value.iter().any(u8::is_ascii_whitespace)
        }) && argv.len() == 1
    });
    CommandLineEvidence {
        argv,
        quality: if rewritten {
            CommandLineQuality::RewrittenProcessTitle
        } else {
            CommandLineQuality::Structured
        },
    }
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "kept as a focused process-lifetime test seam")
)]
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

#[cfg(all(target_os = "linux", test))]
fn parse_process_start_time(stat: &str) -> Option<u64> {
    parse_process_stat(stat).map(|stat| stat.start_time)
}

#[cfg(target_os = "linux")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct ProcessStat {
    parent_pid: u32,
    session_id: u32,
    start_time: u64,
}

#[cfg(target_os = "linux")]
fn read_process_stat(pid: u32) -> Option<ProcessStat> {
    let path = format!("/proc/{pid}/stat");
    parse_process_stat(&std::fs::read_to_string(path).ok()?)
}

#[cfg(target_os = "linux")]
fn parse_process_stat(stat: &str) -> Option<ProcessStat> {
    // The comm field is wrapped in parentheses and may contain spaces
    let end = stat.rfind(')')?;
    let remainder = stat.get(end + 2..)?;
    let fields = remainder.split_whitespace().collect::<Vec<_>>();
    // Field three starts here so parent, session, and start time use fixed offsets
    Some(ProcessStat {
        parent_pid: fields.get(1)?.parse().ok()?,
        session_id: fields.get(3)?.parse().ok()?,
        start_time: fields.get(19)?.parse().ok()?,
    })
}

#[cfg(test)]
#[path = "tests/sender.rs"]
mod tests;
