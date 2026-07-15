//! Runtime config adjustments and sanitization
//!
//! Keeps runtime-only config shaping in one tree so loaded config is safe
//! before the UI and workers read it

mod sanitize;
mod widgets;

// Keep config loading as the only caller of runtime shaping
pub(super) use self::sanitize::sanitize_config;
pub use self::sanitize::{
    MAX_CARD_WIDGETS, MAX_STAT_WIDGETS, MAX_TOGGLE_WIDGETS, MAX_TOTAL_WIDGETS,
};
pub(super) use self::widgets::{apply_brightness_backend, apply_volume_backend};
