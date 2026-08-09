//! Bounded manager-transport availability probes

use std::io;
use std::time::Duration;

use super::CommandSpec;

const DEFAULT_AVAILABILITY_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_AVAILABILITY_STREAM_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceManagerAvailability {
    // The manager transport accepted a read-only query
    Available,
    // The command is missing or its manager transport is not reachable
    Unavailable,
    // The manager query ran but did not prove whether its transport is reachable
    Indeterminate,
}

pub struct ServiceManagerAvailabilityProbe {
    command: CommandSpec,
    interpret: fn(ServiceManagerAvailabilityOutput<'_>) -> ServiceManagerAvailability,
}

impl ServiceManagerAvailabilityProbe {
    pub(in crate::service_manager) const fn new(
        command: CommandSpec,
        interpret: fn(ServiceManagerAvailabilityOutput<'_>) -> ServiceManagerAvailability,
    ) -> Self {
        Self { command, interpret }
    }

    pub fn evaluate(&self) -> io::Result<ServiceManagerAvailability> {
        self.evaluate_with_timeout(DEFAULT_AVAILABILITY_TIMEOUT)
    }

    pub(crate) fn evaluate_with_timeout(
        &self,
        timeout: Duration,
    ) -> io::Result<ServiceManagerAvailability> {
        let mut command = match self.command.to_command() {
            Ok(command) => command,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ServiceManagerAvailability::Unavailable);
            }
            Err(error) => return Err(error),
        };
        let output = crate::system_tools::output_bounded(
            &mut command,
            timeout,
            MAX_AVAILABILITY_STREAM_BYTES,
        )?;
        if output.stdout_truncated || output.stderr_truncated {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "service-manager availability output exceeded its safe byte limit",
            ));
        }
        let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
            return Ok(ServiceManagerAvailability::Indeterminate);
        };
        let Ok(stderr) = std::str::from_utf8(&output.stderr) else {
            return Ok(ServiceManagerAvailability::Indeterminate);
        };
        Ok((self.interpret)(ServiceManagerAvailabilityOutput {
            status_success: output.status.success(),
            did_exit: output.status.code().is_some(),
            stdout,
            stderr,
        }))
    }
}

#[derive(Clone, Copy)]
pub(in crate::service_manager) struct ServiceManagerAvailabilityOutput<'a> {
    status_success: bool,
    did_exit: bool,
    stdout: &'a str,
    stderr: &'a str,
}

impl<'a> ServiceManagerAvailabilityOutput<'a> {
    pub(in crate::service_manager) const fn status_success(self) -> bool {
        self.status_success
    }

    pub(in crate::service_manager) const fn did_exit(self) -> bool {
        self.did_exit
    }

    pub(in crate::service_manager) const fn stdout(self) -> &'a str {
        self.stdout
    }

    pub(in crate::service_manager) const fn stderr(self) -> &'a str {
        self.stderr
    }
}
