//! GTK widget handles for the center panel
//!
//! Keeping the widget bundle here lets `mod.rs` stay as module wiring only

/// GTK widgets backing the notification center panel window.
pub struct PanelWidgets {
    pub window: gtk::ApplicationWindow,
    pub surface: gtk::Overlay,
    pub root: gtk::Box,
    pub body_stack: gtk::Box,
    pub widget_revealer: gtk::Revealer,
    pub widget_stack: gtk::Box,
    pub quick_controls: gtk::Box,
    pub toggle_container: gtk::Box,
    pub stat_container: gtk::Box,
    pub card_container: gtk::Box,
    pub scroller: gtk::ScrolledWindow,
    pub media_container: gtk::Box,
    pub search_revealer: gtk::Revealer,
    pub search_entry: gtk::SearchEntry,
    pub search_toggle: gtk::ToggleButton,
    pub header_title: gtk::Label,
    pub header_subtitle: gtk::Label,
    pub header_count: gtk::Label,
    pub header_action_row: gtk::Box,
    pub header_action_group: gtk::Box,
    pub notification_container: gtk::Box,
    pub notification_header_row: gtk::Box,
    pub notification_header: gtk::Label,
    pub toggle_section_header: gtk::Label,
    pub stat_section_header: gtk::Label,
    pub footer_label: gtk::Label,
    pub focus_toggle: gtk::ToggleButton,
    pub dnd_toggle: gtk::ToggleButton,
    pub clear_action_button: gtk::Button,
    pub clear_header_button: gtk::Button,
    pub close_button: gtk::Button,
}
