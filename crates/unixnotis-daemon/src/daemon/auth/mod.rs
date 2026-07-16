//! Authorization helpers for privileged control methods
//!
//! The control D-Bus interface calls into this module before mutating daemon
//! state. The implementation is split by responsibility because each layer has
//! a different security job: caller identity, executable path trust, metadata
//! checks, and startup-time fingerprint pinning

mod authorization;
mod filesystem;
mod fingerprint;
mod metadata;
mod paths;
mod policy;
mod process;
mod snapshots;

pub(super) use authorization::{authorize_control_call, authorize_panel_readiness_call};

#[cfg(test)]
#[path = "tests/authorization.rs"]
mod authorization_tests;
#[cfg(test)]
#[path = "tests/cache.rs"]
mod cache_tests;
#[cfg(test)]
#[path = "tests/metadata.rs"]
mod metadata_tests;
#[cfg(test)]
#[path = "tests/paths.rs"]
mod paths_tests;
#[cfg(test)]
#[path = "tests/procfs.rs"]
mod procfs_tests;
#[cfg(test)]
#[path = "tests/snapshot.rs"]
mod snapshot_tests;
#[cfg(test)]
#[path = "tests/strict.rs"]
mod strict_tests;
#[cfg(test)]
#[path = "tests/support.rs"]
mod support;
