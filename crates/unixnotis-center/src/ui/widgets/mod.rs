//! Widget module wiring and shared exports for the center panel.

pub mod brightness;
pub mod cards;
pub mod stats;
pub mod toggles;
pub mod volume;

mod stats_builtin;
mod util;

pub use util::CommandSlider;
