//! Authorization helpers for privileged control methods
//!
//! The control D-Bus interface calls into this module before mutating daemon
//! state. The implementation is split by responsibility because each layer has
//! a different security job: caller identity, executable path trust, metadata
//! checks, and startup-time fingerprint pinning

#[path = "auth/authorization.rs"]
mod authorization;
#[path = "auth/filesystem.rs"]
mod filesystem;
#[path = "auth/fingerprint.rs"]
mod fingerprint;
#[path = "auth/metadata.rs"]
mod metadata;
#[path = "auth/paths.rs"]
mod paths;
#[path = "auth/policy.rs"]
mod policy;
#[path = "auth/process.rs"]
mod process;
#[path = "auth/snapshots.rs"]
mod snapshots;

pub(super) use authorization::{authorize_control_call, authorize_panel_readiness_call};

#[cfg(test)]
#[path = "auth/tests/authorization.rs"]
mod authorization_tests;
#[cfg(test)]
#[path = "auth/tests/cache.rs"]
mod cache_tests;
#[cfg(test)]
#[path = "auth/tests/metadata.rs"]
mod metadata_tests;
#[cfg(test)]
#[path = "auth/tests/paths.rs"]
mod paths_tests;
#[cfg(test)]
#[path = "auth/tests/procfs.rs"]
mod procfs_tests;
#[cfg(test)]
#[path = "auth/tests/snapshot.rs"]
mod snapshot_tests;
#[cfg(test)]
#[path = "auth/tests/strict.rs"]
mod strict_tests;
#[cfg(test)]
#[path = "auth/tests/support.rs"]
mod support;
