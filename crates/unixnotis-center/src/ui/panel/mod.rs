//! Panel layout and widget construction for the center window
//!
//! The folder root stays focused on module wiring and the public panel surface

mod actions;
mod build;
mod header;
mod layout;
mod monitor;
mod search;
mod sections;
mod types;

pub use self::build::build_panel_widgets;
pub use self::layout::{apply_panel_config, live_panel_width};
pub use self::types::PanelWidgets;
