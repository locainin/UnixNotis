//! Widget module wiring and shared exports for the center panel

pub mod brightness;
pub mod cards;
mod command_runtime;
mod command_slider;
mod icon_image;
mod kind_css;
// Plugin schema and JSON parsing helpers for widget-backed commands
mod plugin;
pub mod stats;
pub mod toggles;
pub mod volume;

pub use command_slider::CommandSlider;

pub fn configure_command_config_dir(config_dir: std::path::PathBuf) {
    command_runtime::command::configure_command_config_dir(config_dir);
}
