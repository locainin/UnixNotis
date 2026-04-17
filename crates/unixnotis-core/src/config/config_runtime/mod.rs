//! Runtime config adjustments and sanitization
//!
//! Keeps runtime-only config shaping in one tree so defaults, backend
//! selection, and tests do not drift apart

mod sanitize;
mod toggles;
mod widgets;

// Keep the public surface narrow so config callers still go through config_io
pub(super) use self::sanitize::sanitize_config;
// Toggle backend shaping stays separate from widget sliders because the fallback rules differ a lot
pub(super) use self::toggles::apply_toggle_backends;
// Slider backends share one runtime layer because both widgets follow the same command contract
pub(super) use self::widgets::{apply_brightness_backend, apply_volume_backend};
