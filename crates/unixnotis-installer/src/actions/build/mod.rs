//! Build actions and optional acceleration helpers

// Build acceleration is separate from the plain compile path so normal builds stay easy to trace
mod accel;
// Release compilation stays small and focused on invoking cargo with the right package list
mod compile;

pub use accel::{
    detect_build_accel, detect_build_accel_without_repo, write_build_accel_config,
    BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome,
};
pub(crate) use compile::run_build;
