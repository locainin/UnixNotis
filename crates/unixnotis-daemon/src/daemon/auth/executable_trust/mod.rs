//! Trusted executable path, metadata, fingerprint, and startup snapshot policy

mod fingerprint;
mod metadata;
pub(in crate::daemon::auth) mod paths;
mod snapshots;

pub(super) use paths::is_trusted_control_executable_path;

#[cfg(test)]
mod tests;
