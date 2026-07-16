//! Service-manager backend contract for installer-owned daemon startup
//!
//! Installer actions call this service-manager contract instead of branching on systemd,
//! dinit, runit, or s6 directly. Each backend owns its artifacts, command
//! shapes, and session-startup lines so new init systems do not inherit
//! systemd assumptions by accident

mod backends;
mod contract;
mod orchestration;

pub use contract::{
    managed_directory_marker, managed_directory_marker_is_valid, MANAGED_DIRECTORY_MARKER_CONTENTS,
};
pub use contract::{
    CommandSpec, ReadinessIssue, S6DatabaseRefresh, ServiceArtifact, ServiceArtifactKind,
    ServiceArtifactRefresh,
};
pub use orchestration::ServiceManager;

// Tests assert exact service names to keep refactors behavior-preserving
#[cfg(test)]
pub use contract::use_fake_command_bin;
#[cfg(test)]
pub use contract::ServiceProbe;
#[cfg(test)]
pub use contract::MANAGED_DIRECTORY_MARKER;
#[cfg(test)]
pub use orchestration::{
    UNIXNOTIS_DAEMON_DINIT_SERVICE, UNIXNOTIS_DAEMON_RUNIT_SERVICE, UNIXNOTIS_DAEMON_S6_SERVICE,
    UNIXNOTIS_DAEMON_SERVICE,
};
