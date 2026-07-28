//! Availability, state probes, and lifecycle command dispatch

use super::super::backends::{dinit, runit, s6, systemd};
use super::super::contract::{CommandSpec, ServiceProbe};
use super::model::{ServiceManager, ServiceManagerKind};

impl ServiceManager {
    pub fn availability_command(&self) -> Option<CommandSpec> {
        // Availability checks must stay read-only and must not start a service
        match self.kind {
            ServiceManagerKind::Systemd => Some(systemd::availability_command()),
            ServiceManagerKind::Dinit => Some(dinit::availability_command()),
            ServiceManagerKind::Runit => Some(runit::availability_command()),
            ServiceManagerKind::S6 => s6::availability_command(),
        }
    }

    pub fn is_enabled_command(&self) -> Option<CommandSpec> {
        // Some artifact-backed managers have no separate enabled-state command
        match self.kind {
            ServiceManagerKind::Systemd => Some(systemd::is_enabled_command()),
            ServiceManagerKind::Dinit => dinit::is_enabled_command(),
            ServiceManagerKind::Runit => runit::is_enabled_command(),
            ServiceManagerKind::S6 => s6::is_enabled_command(),
        }
    }

    pub fn enabled_by_artifacts(&self) -> Option<bool> {
        // Systemd owns enabled state while the other backends expose installed artifacts
        match self.kind {
            ServiceManagerKind::Systemd => None,
            ServiceManagerKind::Dinit => Some(dinit::enabled_by_artifacts(&self.artifact_root)),
            ServiceManagerKind::Runit => Some(runit::enabled_by_artifacts(&self.artifact_root)),
            ServiceManagerKind::S6 => Some(s6::enabled_by_artifacts(&self.artifact_root)),
        }
    }

    pub fn active_probe(&self) -> ServiceProbe {
        // Probe parsing stays inside each backend because status formats differ
        match self.kind {
            ServiceManagerKind::Systemd => ServiceProbe::exit_status(systemd::is_active_command()),
            ServiceManagerKind::Dinit => ServiceProbe::exit_status(dinit::is_active_command()),
            ServiceManagerKind::Runit => runit::active_probe(&self.artifact_root),
            ServiceManagerKind::S6 => s6::active_probe(self.live_root()),
        }
    }

    pub fn enable_now_command(&self) -> CommandSpec {
        // Enable-and-start is used only when the backend provides one atomic operation
        match self.kind {
            ServiceManagerKind::Systemd => systemd::enable_now_command(),
            ServiceManagerKind::Dinit => dinit::enable_now_command(),
            ServiceManagerKind::Runit => runit::enable_now_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::enable_now_command(self.live_root()),
        }
    }

    pub fn start_command(&self) -> CommandSpec {
        // Start commands operate on the already installed backend artifact
        match self.kind {
            ServiceManagerKind::Systemd => systemd::start_command(),
            ServiceManagerKind::Dinit => dinit::start_command(),
            ServiceManagerKind::Runit => runit::start_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::start_command(self.live_root()),
        }
    }

    pub fn disable_now_command(&self) -> CommandSpec {
        // Disable commands stop the service while removing persistent activation
        match self.kind {
            ServiceManagerKind::Systemd => systemd::disable_now_command(),
            ServiceManagerKind::Dinit => dinit::disable_now_command(),
            ServiceManagerKind::Runit => runit::disable_now_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::disable_now_command(self.live_root()),
        }
    }

    pub fn stop_for_reinstall_command(&self) -> CommandSpec {
        // Reinstall stops the old process without discarding persistent enablement
        match self.kind {
            ServiceManagerKind::Systemd => systemd::stop_for_reinstall_command(),
            ServiceManagerKind::Dinit => dinit::stop_for_reinstall_command(),
            ServiceManagerKind::Runit => runit::stop_for_reinstall_command(&self.artifact_root),
            ServiceManagerKind::S6 => s6::stop_for_reinstall_command(self.live_root()),
        }
    }
}
