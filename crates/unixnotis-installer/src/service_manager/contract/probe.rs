//! Bounded service-manager state probes

use std::io;
use std::time::Duration;

use super::command::CommandSpec;

const DEFAULT_SERVICE_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_SERVICE_PROBE_STREAM_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug)]
pub struct ServiceProbe {
    pub(in crate::service_manager::contract) command: CommandSpec,
    pub(in crate::service_manager::contract) interpret:
        fn(ServiceProbeOutput<'_>) -> ServiceProbeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceProbeState {
    // The manager tool or its manager-level transport is unavailable
    Unavailable,
    // The manager exists but has no live UnixNotis service record
    Absent,
    // UnixNotis is known to the manager and stopped
    Inactive,
    // UnixNotis is running or moving through a live transition
    Active,
    // The probe could not establish a trustworthy state
    Indeterminate,
}

#[derive(Clone, Copy)]
pub(in crate::service_manager) struct ServiceProbeOutput<'a> {
    pub(in crate::service_manager::contract) status_success: bool,
    pub(in crate::service_manager::contract) status_code: Option<i32>,
    pub(in crate::service_manager::contract) stdout: &'a str,
    pub(in crate::service_manager::contract) stderr: &'a str,
}

impl<'a> ServiceProbeOutput<'a> {
    pub(in crate::service_manager) const fn status_success(self) -> bool {
        self.status_success
    }

    pub(in crate::service_manager) const fn status_code(self) -> Option<i32> {
        self.status_code
    }

    pub(in crate::service_manager) const fn stdout(self) -> &'a str {
        self.stdout
    }

    pub(in crate::service_manager) const fn stderr(self) -> &'a str {
        self.stderr
    }
}

impl ServiceProbe {
    pub(in crate::service_manager) const fn new(
        command: CommandSpec,
        interpret: fn(ServiceProbeOutput<'_>) -> ServiceProbeState,
    ) -> Self {
        Self { command, interpret }
    }

    pub fn evaluate_state(&self) -> io::Result<ServiceProbeState> {
        self.evaluate_state_with_timeout(DEFAULT_SERVICE_PROBE_TIMEOUT)
    }

    pub(crate) fn evaluate_state_with_timeout(
        &self,
        timeout: Duration,
    ) -> io::Result<ServiceProbeState> {
        let mut command = match self.command.to_command() {
            Ok(command) => command,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ServiceProbeState::Unavailable);
            }
            Err(error) => return Err(error),
        };
        let output = crate::system_tools::output_bounded(
            &mut command,
            timeout,
            MAX_SERVICE_PROBE_STREAM_BYTES,
        )?;
        if output.stdout_truncated || output.stderr_truncated {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "service-manager probe output exceeded its safe byte limit",
            ));
        }
        let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
            return Ok(ServiceProbeState::Indeterminate);
        };
        let Ok(stderr) = std::str::from_utf8(&output.stderr) else {
            return Ok(ServiceProbeState::Indeterminate);
        };
        Ok((self.interpret)(ServiceProbeOutput {
            status_success: output.status.success(),
            // A signal-terminated manager cannot prove a stable service state
            status_code: output.status.code(),
            stdout,
            stderr,
        }))
    }

    pub(crate) const fn default_timeout() -> Duration {
        DEFAULT_SERVICE_PROBE_TIMEOUT
    }
}
