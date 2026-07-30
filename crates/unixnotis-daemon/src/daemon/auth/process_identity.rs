//! Process metadata helpers for authorization checks

use std::path::PathBuf;

#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use std::os::fd::{AsFd, AsRawFd, OwnedFd};

#[cfg(target_os = "linux")]
const MAX_PIDFD_INFO_BYTES: u64 = 4_096;

#[cfg(any(not(target_os = "linux"), test))]
pub(in crate::daemon) async fn read_process_executable_path(pid: u32) -> Option<PathBuf> {
    // Linux exposes the real executable path via /proc
    let path = format!("/proc/{pid}/exe");
    tokio::fs::read_link(path).await.ok()
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn read_process_executable_path_from_pidfd<Fd: AsFd>(
    pidfd: &Fd,
    expected_pid: u32,
) -> Option<PathBuf> {
    // A ready pidfd means its process has exited and its pid must not be followed
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }

    // The live pidfd prevents this pid from being reused during the path lookup
    let executable = std::fs::read_link(format!("/proc/{expected_pid}/exe")).ok()?;

    // A second check closes the small window where the process exits during readlink
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }
    Some(executable)
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn open_process_executable_from_pidfd<Fd: AsFd>(
    pidfd: &Fd,
    expected_pid: u32,
) -> Option<OwnedFd> {
    // A ready pidfd means its process has exited and its pid must not be followed
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }

    // Open /proc/<pid>/exe as a file descriptor. This follows the procfs
    // magic symlink to the actual executable object. The resulting descriptor
    // refers to the kernel file object, not a pathname that could be shadowed
    // by a mount namespace.
    let fd = std::fs::OpenOptions::new()
        .read(true)
        .open(format!("/proc/{expected_pid}/exe"))
        .ok()?;

    // A second check closes the small window where the process exits during open
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }
    Some(fd.into())
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn read_pidfd_process_id<Fd: AsFd>(pidfd: &Fd) -> Option<u32> {
    let raw_fd = pidfd.as_fd().as_raw_fd();
    let file = std::fs::File::open(format!("/proc/self/fdinfo/{raw_fd}")).ok()?;

    let bytes = read_pidfd_info_bytes(file, MAX_PIDFD_INFO_BYTES)?;

    parse_pidfd_process_id(std::str::from_utf8(&bytes).ok()?)
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn read_pidfd_info_bytes(
    reader: impl Read,
    max_bytes: u64,
) -> Option<Vec<u8>> {
    // procfs data is tiny, but the explicit cap keeps this parser bounded
    let mut bytes = Vec::with_capacity(512);
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if u64::try_from(bytes.len()).ok()? > max_bytes {
        return None;
    }
    Some(bytes)
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn parse_pidfd_process_id(contents: &str) -> Option<u32> {
    let mut values = contents.lines().filter_map(|line| {
        line.strip_prefix("Pid:")
            .and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|pid| *pid > 0)
    });
    let pid = values.next()?;

    // Duplicate identity fields make the kernel record ambiguous
    if values.next().is_some() {
        return None;
    }
    Some(pid)
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn pidfd_matches_live_process<Fd: AsFd>(
    pidfd: &Fd,
    expected_pid: u32,
) -> bool {
    pidfd_is_live(pidfd) && read_pidfd_process_id(pidfd) == Some(expected_pid)
}

#[cfg(target_os = "linux")]
pub(in crate::daemon) fn pidfd_is_live<Fd: AsFd>(pidfd: &Fd) -> bool {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};

    let mut poll_fds = [PollFd::new(pidfd, PollFlags::IN)];
    let zero_timeout = Timespec::default();

    // A live process has no readable event on its pidfd
    // With one descriptor, a zero count also guarantees an empty event set
    poll(&mut poll_fds, Some(&zero_timeout)).is_ok_and(|ready| ready == 0)
}
