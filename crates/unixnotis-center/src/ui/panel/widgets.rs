//! Grouped GTK widget handles for the center panel
//!
//! Keeping the widget bundle here lets `mod.rs` stay as module wiring only

use super::body::PanelSectionWidgets;
use super::header::PanelHeaderWidgets;
use super::notice::ReloadNoticeWidgets;

/// GTK widgets backing the notification center panel window
pub struct PanelWidgets {
    pub window: gtk::ApplicationWindow,
    pub surface: gtk::Overlay,
    pub root: gtk::Box,
    pub(in crate::ui) header: PanelHeaderWidgets,
    pub(in crate::ui) sections: PanelSectionWidgets,
    pub(in crate::ui) reload_notice: ReloadNoticeWidgets,
}
