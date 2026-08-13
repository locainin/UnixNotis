//! Panel geometry module wiring

mod layout;
pub(super) mod monitor;

pub(super) use layout::{apply_anchor, map_keyboard_mode, resolve_panel_size};
pub use layout::{apply_panel_config, requested_panel_width};
