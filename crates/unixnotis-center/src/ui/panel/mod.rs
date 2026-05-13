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
pub(crate) use self::search::SEARCH_REVEAL_TRANSITION_MS;
pub(crate) use self::sections::{notification_header_row_visible, WIDGET_REVEAL_TRANSITION_MS};
pub use self::types::PanelWidgets;
