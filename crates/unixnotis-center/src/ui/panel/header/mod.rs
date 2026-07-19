//! Panel header component wiring

pub(in crate::ui) mod actions;
mod build;
pub(in crate::ui) mod dnd;
pub(in crate::ui) mod search;
mod widgets;

pub(super) use build::build_panel_header;
pub(in crate::ui) use widgets::PanelHeaderWidgets;
