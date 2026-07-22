//! Trusted executable path, metadata, fingerprint, and startup snapshot policy

mod fingerprint;
mod metadata;
mod paths;
mod snapshots;

#[cfg(test)]
pub(super) use paths::canonicalize_best_effort;
pub(super) use paths::is_trusted_control_executable_path;

#[cfg(test)]
mod tests;
