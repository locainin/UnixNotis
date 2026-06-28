//! Service-manager backend contract for installer-owned daemon startup.
//!
//! Installer actions call this service-manager contract instead of branching on systemd,
//! dinit, runit, or s6 directly. Each backend owns its artifacts, command
//! shapes, and session-startup lines so new init systems do not inherit
//! systemd assumptions by accident.

// Artifact data stays separate from writer code so tests can inspect planned writes
mod artifact;
// CommandSpec stores argv as data so lifecycle commands never require shell parsing
mod command;
// Backend modules own manager-specific behavior and keep manager.rs as a dispatcher
mod dinit;
mod manager;
// ServiceProbe separates exact exit-status checks from stdout-parsed service status
mod probe;
// ReadinessIssue lets backends separate hard blockers from setup hints
mod readiness;
// Refresh plans cover simple reload commands and multi-step database updates
mod refresh;
mod runit;
mod s6;
// Shell helpers are limited to generated Hyprland bootstrap snippets
mod shell;
mod systemd;

pub use artifact::{
    managed_directory_marker, managed_directory_marker_is_valid, MANAGED_DIRECTORY_MARKER_CONTENTS,
};
pub use artifact::{ServiceArtifact, ServiceArtifactKind};
pub use command::CommandSpec;
pub use manager::ServiceManager;
pub use probe::ServiceProbe;
pub use readiness::ReadinessIssue;
pub use refresh::{S6DatabaseRefresh, ServiceArtifactRefresh};

// Tests assert exact service names to keep refactors behavior-preserving
#[cfg(test)]
pub use artifact::MANAGED_DIRECTORY_MARKER;
#[cfg(test)]
pub(crate) use command::use_fake_command_bin;
#[cfg(test)]
pub use manager::{
    UNIXNOTIS_DAEMON_DINIT_SERVICE, UNIXNOTIS_DAEMON_RUNIT_SERVICE, UNIXNOTIS_DAEMON_S6_SERVICE,
    UNIXNOTIS_DAEMON_SERVICE,
};

// Unit tests live under service_manager/tests so backend modules do not become test dumps
#[cfg(test)]
#[path = "service_manager/tests/artifacts.rs"]
mod artifact_tests;

#[cfg(test)]
#[path = "service_manager/tests/systemd.rs"]
mod systemd_tests;

#[cfg(test)]
#[path = "service_manager/tests/dinit.rs"]
mod dinit_tests;

#[cfg(test)]
#[path = "service_manager/tests/runit.rs"]
mod runit_tests;

#[cfg(test)]
#[path = "service_manager/tests/s6.rs"]
mod s6_tests;

#[cfg(test)]
#[path = "service_manager/tests/shell.rs"]
mod shell_tests;
