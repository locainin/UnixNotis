//! Service-manager backend contract for installer-owned daemon startup
//!
//! Installer actions call this service-manager contract instead of branching on systemd,
//! dinit, runit, or s6 directly. Each backend owns its artifacts, command
//! shapes, and session-startup lines so new init systems do not inherit
//! systemd assumptions by accident

mod backends;
pub mod contract;
mod orchestration;

pub use contract::MANAGED_DIRECTORY_MARKER_CONTENTS;
pub use contract::{
    CommandSpec, ReadinessIssue, S6DatabaseRefresh, ServiceArtifact, ServiceArtifactKind,
    ServiceArtifactRefresh, ServiceArtifactState,
};
pub use orchestration::ServiceManager;
