//! Optional local build acceleration helpers

// Detection stays separate so read-only status checks do not carry write helpers along
mod detect;
// Shared status types live in one place so UI and write paths use the same shape
mod model;
// Wrapper content and file permissions stay separate from config update flow
mod wrapper;
// Config writes live apart from detection so the side effects are easy to audit
mod write;

pub use detect::{detect_build_accel, detect_build_accel_without_repo};
pub use model::{BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome};
pub use write::write_build_accel_config;

#[cfg(test)]
mod tests;
