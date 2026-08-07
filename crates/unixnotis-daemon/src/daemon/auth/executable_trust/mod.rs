//! Trusted executable path, metadata, fingerprint, and startup snapshot policy

use std::collections::HashMap;

mod fingerprint;
mod metadata;
pub(in crate::daemon::auth) mod paths;
mod snapshots;

#[cfg(target_os = "linux")]
pub(super) use paths::is_trusted_control_executable_from_fd;
#[cfg(not(target_os = "linux"))]
pub(super) use paths::is_trusted_control_executable_path;

pub(in crate::daemon) fn build_trusted_control_snapshots_for_current_executable(
) -> HashMap<String, super::policy::TrustedExecutableSnapshot> {
    // Resolve the sibling directory before the daemon publishes any D-Bus service
    paths::trusted_control_directory().map_or_else(HashMap::new, |trusted_dir| {
        snapshots::build_trusted_control_snapshots(&trusted_dir)
    })
}

#[cfg(test)]
mod tests;
