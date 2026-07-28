//! Stable Linux process handles used while stopping an unmanaged daemon

use std::fs;
use std::os::fd::OwnedFd;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::process::{kill_process, pidfd_open, pidfd_send_signal, Pid, PidfdFlags, Signal};

const PROCESS_EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const FALLBACK_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) enum ProcessState {
    Gone,
    Running(ProcessHandle),
}

pub(super) struct ProcessHandle {
    pid: Pid,
    start_time: u64,
    pidfd: Option<OwnedFd>,
}

impl ProcessHandle {
    pub(super) fn open(raw_pid: u32, expected_program: &str) -> Result<ProcessState> {
        let Some(pid) = process_id(raw_pid) else {
            return Ok(ProcessState::Gone);
        };

        // pidfd keeps the process identity stable even if the numeric PID is later reused
        let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(pidfd) => Some(pidfd),
            Err(rustix::io::Errno::SRCH) => return Ok(ProcessState::Gone),
            // Older Linux kernels need the start-time guarded fallback below
            Err(rustix::io::Errno::NOSYS) => None,
            Err(error) => {
                return Err(anyhow!(
                    "failed to open stable handle for pid {raw_pid}: {error}"
                ))
            }
        };

        // Read lifetime evidence around program validation so fallback signaling fails closed
        let Some(start_before) = read_process_start_time(raw_pid)? else {
            return Ok(ProcessState::Gone);
        };
        if !process_matches_program(raw_pid, expected_program) {
            return Err(anyhow!(
                "pid {raw_pid} no longer matches expected daemon {expected_program}; aborting stop"
            ));
        }
        let Some(start_after) = read_process_start_time(raw_pid)? else {
            return Ok(ProcessState::Gone);
        };
        if start_before != start_after {
            return Err(anyhow!(
                "pid {raw_pid} changed while its identity was checked; aborting stop"
            ));
        }

        Ok(ProcessState::Running(Self {
            pid,
            start_time: start_before,
            pidfd,
        }))
    }

    pub(super) fn terminate(&self) -> Result<()> {
        if let Some(pidfd) = &self.pidfd {
            // The signal targets the opened process object instead of a reusable number
            return pidfd_send_signal(pidfd, Signal::TERM)
                .context("failed to terminate notification daemon through pidfd");
        }

        // The fallback repeats the lifetime read immediately before the numeric signal
        self.require_current_lifetime()?;
        kill_process(self.pid, Signal::TERM)
            .context("failed to terminate notification daemon through native signal")
    }

    pub(super) fn wait_for_exit(&self) -> Result<()> {
        if let Some(pidfd) = &self.pidfd {
            return wait_for_pidfd(pidfd);
        }

        let started = Instant::now();
        while started.elapsed() < PROCESS_EXIT_TIMEOUT {
            match read_process_start_time(self.pid.as_raw_pid() as u32)? {
                None => return Ok(()),
                // A new lifetime means the original target exited and must not be inspected
                Some(current) if current != self.start_time => return Ok(()),
                Some(_) => thread::sleep(FALLBACK_POLL_INTERVAL),
            }
        }

        Err(anyhow!(
            "process {} did not exit after 5s",
            self.pid.as_raw_pid()
        ))
    }

    fn require_current_lifetime(&self) -> Result<()> {
        let current = read_process_start_time(self.pid.as_raw_pid() as u32)?;
        if current == Some(self.start_time) {
            return Ok(());
        }
        Err(anyhow!(
            "pid {} changed before signaling; aborting stop",
            self.pid.as_raw_pid()
        ))
    }
}

fn wait_for_pidfd(pidfd: &OwnedFd) -> Result<()> {
    let mut descriptors = [PollFd::new(pidfd, PollFlags::IN)];
    let timeout = Timespec {
        tv_sec: PROCESS_EXIT_TIMEOUT.as_secs() as i64,
        tv_nsec: 0,
    };
    poll(&mut descriptors, Some(&timeout))
        .context("failed while waiting for notification daemon pidfd")?;
    if descriptors[0].revents().contains(PollFlags::IN) {
        return Ok(());
    }
    Err(anyhow!("process did not exit after 5s"))
}

fn process_id(raw_pid: u32) -> Option<Pid> {
    let raw_pid = i32::try_from(raw_pid).ok()?;
    Pid::from_raw(raw_pid)
}

fn process_matches_program(pid: u32, expected: &str) -> bool {
    // Argv preserves daemon basenames longer than Linux's 15-byte comm field
    crate::detect::read_cmdline_program(pid)
        .or_else(|| read_proc_comm(pid))
        .is_some_and(|program| program == expected)
}

fn read_proc_comm(pid: u32) -> Option<String> {
    let contents = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let comm = contents.trim();
    (!comm.is_empty()).then(|| comm.to_string())
}

fn read_process_start_time(pid: u32) -> Result<Option<u64>> {
    let path = format!("/proc/{pid}/stat");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if process_state_is_missing(&error) => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read process state from {path}"))
        }
    };
    parse_process_start_time(&contents)
        .map(Some)
        .ok_or_else(|| anyhow!("failed to parse process start time for pid {pid}"))
}

fn process_state_is_missing(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
}

fn parse_process_start_time(stat: &str) -> Option<u64> {
    // The command field is parenthesized and may itself contain spaces
    let command_end = stat.rfind(')')?;
    let fields_after_command = stat.get(command_end + 2..)?;
    // Field 3 begins here, placing the process start time at zero-based index 19
    fields_after_command
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

#[cfg(test)]
#[path = "tests/process_handle.rs"]
mod tests;
