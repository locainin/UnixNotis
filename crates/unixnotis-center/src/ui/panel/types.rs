//! GTK widget handles for the center panel
//!
//! Keeping the widget bundle here lets `mod.rs` stay as module wiring only

/// GTK widgets backing the notification center panel window.
pub struct PanelWidgets {
    pub window: gtk::ApplicationWindow,
    pub root: gtk::Box,
    pub widget_revealer: gtk::Revealer,
    pub quick_controls: gtk::Box,
    pub toggle_container: gtk::Box,
    pub stat_container: gtk::Box,
    pub card_container: gtk::Box,
    pub scroller: gtk::ScrolledWindow,
    pub media_container: gtk::Box,
    pub search_revealer: gtk::Revealer,
    pub search_entry: gtk::SearchEntry,
    pub search_toggle: gtk::ToggleButton,
    pub header_count: gtk::Label,
    pub focus_toggle: gtk::ToggleButton,
    pub dnd_toggle: gtk::ToggleButton,
    pub clear_button: gtk::Button,
    pub close_button: gtk::Button,
}
