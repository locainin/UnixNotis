//! Trusted executable path, metadata, fingerprint, and startup snapshot policy

mod fingerprint;
mod metadata;
pub(in crate::daemon::auth) mod paths;
mod snapshots;

#[cfg(not(target_os = "linux"))]
pub(super) use paths::is_trusted_control_executable_path;
#[cfg(target_os = "linux")]
pub(super) use paths::is_trusted_control_executable_from_fd;

#[cfg(test)]
mod tests;
