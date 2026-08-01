//! Stable process-object checks for MPRIS authorization

#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use std::os::fd::{AsFd, AsRawFd};

#[cfg(target_os = "linux")]
const MAX_PIDFD_INFO_BYTES: u64 = 4_096;

#[cfg(target_os = "linux")]
pub(super) fn read_process_executable_path_from_pidfd<Fd: AsFd>(
    pidfd: &Fd,
    expected_pid: u32,
) -> Option<std::path::PathBuf> {
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }
    let path = std::fs::read_link(format!("/proc/{expected_pid}/exe")).ok()?;
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }
    Some(path)
}

#[cfg(target_os = "linux")]
pub(super) fn open_process_executable_from_pidfd<Fd: AsFd>(
    pidfd: &Fd,
    expected_pid: u32,
) -> Option<std::fs::File> {
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }
    let file = std::fs::File::open(format!("/proc/{expected_pid}/exe")).ok()?;
    if !pidfd_matches_live_process(pidfd, expected_pid) {
        return None;
    }
    Some(file)
}

#[cfg(target_os = "linux")]
fn pidfd_matches_live_process<Fd: AsFd>(pidfd: &Fd, expected_pid: u32) -> bool {
    pidfd_is_live(pidfd) && read_pidfd_process_id(pidfd) == Some(expected_pid)
}

#[cfg(target_os = "linux")]
fn pidfd_is_live<Fd: AsFd>(pidfd: &Fd) -> bool {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};

    let mut poll_fds = [PollFd::new(pidfd, PollFlags::IN)];
    poll(&mut poll_fds, Some(&Timespec::default())).is_ok_and(|ready| ready == 0)
}

#[cfg(target_os = "linux")]
fn read_pidfd_process_id<Fd: AsFd>(pidfd: &Fd) -> Option<u32> {
    let raw_fd = pidfd.as_fd().as_raw_fd();
    let file = std::fs::File::open(format!("/proc/self/fdinfo/{raw_fd}")).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_PIDFD_INFO_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if u64::try_from(bytes.len()).ok()? > MAX_PIDFD_INFO_BYTES {
        return None;
    }
    let mut values = std::str::from_utf8(&bytes)
        .ok()?
        .lines()
        .filter_map(|line| {
            line.strip_prefix("Pid:")
                .and_then(|value| value.trim().parse::<u32>().ok())
                .filter(|pid| *pid > 0)
        });
    let pid = values.next()?;
    values.next().is_none().then_some(pid)
}

#[cfg(target_os = "linux")]
pub(super) fn executable_allowed_from_pidfd(
    pidfd: &impl AsFd,
    expected_pid: u32,
    allowlist: &[String],
) -> bool {
    if allowlist.is_empty() {
        return false;
    }
    let Some(owner_file) = open_process_executable_from_pidfd(pidfd, expected_pid) else {
        return false;
    };
    super::admission::executable_file_matches_allowlist(owner_file, allowlist)
}
