//! Panel layout and widget construction for the center window
//!
//! The folder root stays focused on module wiring and the public panel surface

mod action_widgets;
mod actions;
mod autoclose;
mod build;
mod dnd;
mod header;
pub(in crate::ui) mod input;
mod keyboard;
mod layout;
mod monitor;
mod notice;
mod reload;
mod search;
mod search_widgets;
mod sections;
mod timing;
mod types;
mod visibility;

pub use self::build::build_panel_widgets;
pub use self::layout::{apply_panel_config, requested_panel_width};
pub use self::reload::{apply_reloaded_body_order, apply_reloaded_panel_chrome};
pub use self::search_widgets::SEARCH_REVEAL_TRANSITION_MS;
pub use self::sections::apply_widget_density;
pub use self::sections::{notification_header_row_visible, WIDGET_REVEAL_TRANSITION_MS};
pub use self::types::PanelWidgets;
pub(in crate::ui) use actions::{connect_clear_button, connect_close_button, connect_dnd_toggle};
pub(in crate::ui) use autoclose::connect_auto_close;
pub(in crate::ui) use dnd::connect_dnd_menu;
pub(in crate::ui) use keyboard::connect_keyboard_shortcuts;
pub(in crate::ui) use search::{
    connect_filter_entry, connect_search_toggle, connect_widget_collapse_toggle,
};
