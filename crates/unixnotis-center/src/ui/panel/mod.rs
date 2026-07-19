//! Panel layout and widget construction for the center window
//!
//! The folder root stays focused on module wiring and the public panel surface

mod apply;
pub(in crate::ui) mod behavior;
mod body;
mod build;
mod geometry;
mod header;
mod notice;
mod state;
mod widgets;

pub use self::apply::{apply_reloaded_body_order, apply_reloaded_panel_chrome};
pub use self::body::apply_widget_density;
pub use self::body::{notification_header_row_visible, WIDGET_REVEAL_TRANSITION_MS};
pub use self::build::build_panel_widgets;
pub use self::geometry::{apply_panel_config, requested_panel_width};
pub use self::widgets::PanelWidgets;
pub(in crate::ui) use behavior::input;
pub(in crate::ui) use behavior::{connect_auto_close, connect_keyboard_shortcuts};
pub(in crate::ui) use header::actions::{
    connect_clear_button, connect_close_button, connect_dnd_toggle,
};
pub(in crate::ui) use header::dnd::{connect_dnd_menu, DndCountdown, DndDurationMenu};
pub(in crate::ui) use header::search::{
    connect_filter_entry, connect_search_toggle, connect_widget_collapse_toggle, set_search_open,
};
