//! Widget module wiring and shared exports for the center panel.

pub mod brightness;
pub mod cards;
// Plugin schema and JSON parsing helpers for widget-backed commands
mod plugin;
pub mod stats;
pub mod toggles;
// Shared helpers are kept in a dedicated module to prevent single-file sprawl
mod utils;
pub mod volume;

// Re-export keeps existing call sites stable while internals stay modular
pub use utils::CommandSlider;

pub(crate) fn configure_command_config_dir(config_dir: std::path::PathBuf) {
    utils::configure_command_config_dir(config_dir);
}
