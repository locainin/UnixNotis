//! Sender metadata helpers for incoming Notify/CloseNotification calls
//!
//! Sender details are optional and best-effort, so failures here must not reject
//! notification delivery

use std::path::Path;

use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::Connection;

#[derive(Debug, Clone)]
pub(super) struct SenderMetadata {
    // Unique bus sender name (:1.x) used for ownership checks
    pub(super) sender_name: Option<String>,
    // Process id is paired with start time so reused pids do not inherit ownership
    pub(super) sender_pid: Option<u32>,
    // Linux start time identifies one concrete process lifetime
    pub(super) sender_start_time: Option<u64>,
    // Executable path is used for diagnostics and app-name mismatch logging
    pub(super) sender_executable: Option<String>,
}

pub(super) async fn resolve_sender_metadata(
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
        };
    };

    let Ok(bus_name) = zbus::names::BusName::try_from(sender_name_str) else {
        return SenderMetadata {
            sender_name,
            sender_pid: None,
            sender_start_time: None,
            sender_executable: None,
        };
    };

    let Ok(proxy) = DBusProxy::new(connection).await else {
        return SenderMetadata {
            sender_name,
            sender_pid: None,
            sender_start_time: None,
            sender_executable: None,
        };
    };

    // PID and executable come from the bus owner, not caller-provided payload fields
    let sender_pid = proxy.get_connection_unix_process_id(bus_name).await.ok();
    let sender_start_time = sender_pid.and_then(read_process_start_time);
    let sender_executable = match sender_pid {
        Some(pid) => read_process_executable_path(pid)
            .await
            .map(|path| path.display().to_string()),
        None => None,
    };

    SenderMetadata {
        sender_name,
        sender_pid,
        sender_start_time,
        sender_executable,
    }
}

pub(super) fn app_name_matches_sender(app_name: &str, sender_executable: &str) -> bool {
    // This check is advisory only; many apps use display names that differ from binary names
    let app = app_name.trim().to_ascii_lowercase();
    if app.is_empty() {
        return true;
    }

    let Some(exe_name) = Path::new(sender_executable)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
    else {
        return true;
    };

    app == exe_name || app.replace(' ', "-") == exe_name || exe_name.contains(&app)
}

#[cfg(target_os = "linux")]
async fn read_process_executable_path(pid: u32) -> Option<std::path::PathBuf> {
    // Linux path to the executable behind this process id
    let path = format!("/proc/{pid}/exe");
    tokio::fs::read_link(path).await.ok()
}

#[cfg(target_os = "linux")]
fn read_process_start_time(pid: u32) -> Option<u64> {
    // /proc/<pid>/stat keeps the process lifetime tick count in field 22
    let path = format!("/proc/{pid}/stat");
    let contents = std::fs::read_to_string(path).ok()?;
    parse_process_start_time(&contents)
}

#[cfg(not(target_os = "linux"))]
async fn read_process_executable_path(_pid: u32) -> Option<std::path::PathBuf> {
    // On other platforms this metadata is optional
    None
}

#[cfg(not(target_os = "linux"))]
fn read_process_start_time(_pid: u32) -> Option<u64> {
    // Non-Linux builds fall back to bus-name ownership only
    None
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
mod tests {
    use super::parse_process_start_time;

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_process_start_time_handles_spaces_in_comm() {
        let stat =
            "42 (player with spaces) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654 20";
        assert_eq!(parse_process_start_time(stat), Some(987654));
    }
}
