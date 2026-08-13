//! Package-manager subprocess supervision

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rustix::process::{kill_process_group, Pid, Signal};
use wait_timeout::ChildExt;

use super::cache::NegativeCause;

const PACKAGE_QUERY_TIMEOUT: Duration = Duration::from_secs(1);
const PACKAGE_PIPE_DRAIN_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub(super) struct PackageQueryOutput {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PackageQueryFailure {
    Spawn,
    Wait,
    Timeout,
    Reader,
    PipeDrainTimeout,
    OutputLimit,
}

impl PackageQueryFailure {
    pub(super) const fn negative_cause(self) -> NegativeCause {
        match self {
            Self::Timeout | Self::PipeDrainTimeout => NegativeCause::Timeout,
            Self::OutputLimit => NegativeCause::MalformedOutput,
            Self::Spawn | Self::Wait | Self::Reader => NegativeCause::ProcessTermination,
        }
    }
}

pub(super) fn run_package_query(
    command: &mut Command,
    output_limit: usize,
) -> Result<PackageQueryOutput, PackageQueryFailure> {
    run_package_query_with_timeout(command, output_limit, PACKAGE_QUERY_TIMEOUT)
}

pub(super) fn run_package_query_with_timeout(
    command: &mut Command,
    output_limit: usize,
    timeout: Duration,
) -> Result<PackageQueryOutput, PackageQueryFailure> {
    // A provider may launch helpers that keep the output pipe open after its leader exits
    command.process_group(0);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_error| PackageQueryFailure::Spawn)?;
    // The child is its new process-group leader because process_group received zero
    let process_group = Pid::from_child(&child);
    let Some(stdout) = child.stdout.take() else {
        terminate_package_query(&mut child, process_group);
        return Err(PackageQueryFailure::Reader);
    };
    let (reader_tx, reader_rx) = mpsc::sync_channel(1);
    let reader = std::thread::Builder::new()
        .name("unixnotis-package-output".to_string())
        .spawn(move || {
            let limit = u64::try_from(output_limit)
                .unwrap_or(u64::MAX)
                .saturating_add(1);
            let mut output = Vec::new();
            let read_result = stdout.take(limit).read_to_end(&mut output);
            let _send_result = reader_tx.send(read_result.map(|_bytes| output));
        })
        .map_err(|_error| {
            terminate_package_query(&mut child, process_group);
            PackageQueryFailure::Reader
        })?;
    // The result channel owns completion; dropping the handle avoids every unbounded join path
    drop(reader);

    let started = Instant::now();
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            terminate_package_query(&mut child, process_group);
            return Err(PackageQueryFailure::Timeout);
        }
        Err(_error) => {
            terminate_package_query(&mut child, process_group);
            return Err(PackageQueryFailure::Wait);
        }
    };
    let remaining = timeout.saturating_sub(started.elapsed());
    let drain_timeout = remaining.min(PACKAGE_PIPE_DRAIN_TIMEOUT);
    let stdout = match reader_rx.recv_timeout(drain_timeout) {
        Ok(Ok(stdout)) => stdout,
        Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Err(PackageQueryFailure::Reader);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // The leader exited, so only inherited pipe holders remain in its process group
            let _kill_result = kill_process_group(process_group, Signal::KILL);
            return Err(PackageQueryFailure::PipeDrainTimeout);
        }
    };
    if stdout.len() > output_limit {
        return Err(PackageQueryFailure::OutputLimit);
    }
    Ok(PackageQueryOutput { status, stdout })
}

pub(super) fn terminate_package_query(child: &mut std::process::Child, process_group: Pid) {
    // Group termination closes ordinary inherited pipes while the bounded reap avoids startup hangs
    if kill_process_group(process_group, Signal::KILL).is_err() {
        let _kill_result = child.kill();
    }
    let _wait_result = child.wait_timeout(PACKAGE_PIPE_DRAIN_TIMEOUT);
}
