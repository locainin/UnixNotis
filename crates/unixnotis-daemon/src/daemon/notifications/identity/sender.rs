//! Sender metadata helpers for incoming Notify/CloseNotification calls
//!
//! Sender details are optional and best-effort, so failures here must not reject
//! notification delivery

use std::fs::File;
use std::io::Read;

use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::Connection;

use super::sender_cache::SenderMetadataCache;
use super::{executable_evidence_for_pid, FileIdentity};

const MAX_PROCESS_CMDLINE_BYTES: u64 = 128 * 1024;
const MAX_PROCESS_ARGUMENTS: usize = 256;

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
    // Quality is explicit because processes may rewrite the visible procfs argument memory
    pub(in crate::daemon::notifications) command_line: CommandLineEvidence,
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
            command_line: CommandLineEvidence::default(),
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
            command_line: CommandLineEvidence::default(),
        };
    };

    let Ok(proxy) = DBusProxy::new(connection).await else {
        return SenderMetadata {
            sender_name,
            sender_pid: None,
            sender_start_time: None,
            sender_executable: None,
            sender_executable_identity: None,
            command_line: CommandLineEvidence::default(),
        };
    };

    // PID and executable come from the bus owner, not caller-provided payload fields
    let sender_pid = proxy.get_connection_unix_process_id(bus_name).await.ok();
    let (sender_start_time, process_evidence) = sender_pid.map_or((None, None), |pid| {
        let start_before = read_process_start_time(pid);
        let executable = executable_evidence_for_pid(pid);
        let command_line = read_process_cmdline(pid, executable.as_ref());
        let evidence = (executable, command_line);
        let start_after = read_process_start_time(pid);
        stable_process_evidence(start_before, Some(evidence), start_after)
    });
    let (executable_evidence, command_line) =
        process_evidence.unwrap_or_else(|| (None, CommandLineEvidence::default()));
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
        command_line,
    };
    // Failed lookups remain retryable instead of becoming persistent unknown identities
    if metadata.sender_start_time.is_some() && metadata.sender_executable_identity.is_some() {
        cache.insert(cache_key, metadata.clone());
    }
    metadata
}

pub(super) fn refresh_sender_security_evidence(metadata: &SenderMetadata) -> SenderMetadata {
    let mut refreshed = metadata.clone();
    let (Some(pid), Some(expected_start)) = (metadata.sender_pid, metadata.sender_start_time)
    else {
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
        refreshed.sender_executable = None;
        refreshed.sender_executable_identity = None;
        refreshed.command_line = CommandLineEvidence::default();
        return refreshed;
    }

    refreshed.sender_executable = executable
        .as_ref()
        .map(|evidence| evidence.canonical_path.display().to_string());
    refreshed.sender_executable_identity = executable.map(|evidence| evidence.identity);
    refreshed.command_line = command_line;
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
fn read_process_start_time(pid: u32) -> Option<u64> {
    // /proc/<pid>/stat keeps the process lifetime tick count in field 22
    let path = format!("/proc/{pid}/stat");
    let contents = std::fs::read_to_string(path).ok()?;
    parse_process_start_time(&contents)
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
fn read_process_start_time(_pid: u32) -> Option<u64> {
    // Non-Linux builds fall back to bus-name ownership only
    None
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
