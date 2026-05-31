//! Panel layout configuration

use serde::{Deserialize, Serialize};

use super::{
    default_panel_action_order, default_panel_section_order, default_panel_widget_order, Anchor,
    Margins, PanelActionConfig, PanelActionId, PanelClearButtonPlacement,
    PanelKeyboardInteractivity, PanelSection, PanelWidgetSection, PANEL_HEIGHT_PERCENT_DEFAULT,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PanelConfig {
    pub anchor: Anchor,
    pub margin: Margins,
    pub width: i32,
    /// Vertical size as a percent of usable monitor height
    pub height: i32,
    /// Exact pixel height override for advanced layouts
    pub height_override: Option<i32>,
    pub keyboard_interactivity: PanelKeyboardInteractivity,
    pub output: Option<String>,
    /// Text shown when the notification list is empty
    pub empty_text: String,
    /// Main heading shown in the panel header
    pub title: String,
    /// Secondary text shown below the main heading
    pub subtitle: String,
    /// Placeholder text shown in the panel search entry
    pub search_placeholder: String,
    /// Show the search entry without requiring the search toggle first
    pub search_visible: bool,
    /// Show the compact utility action row below the header
    pub action_row_visible: bool,
    /// Wrap the notification list in a titled section
    pub notification_section_visible: bool,
    /// Let the notification list consume remaining vertical panel space
    pub notification_list_expand: bool,
    /// Show optional notification metadata lanes
    pub notification_metadata_visible: bool,
    /// Show optional notification image thumbnails in panel rows
    pub notification_thumbnails_visible: bool,
    /// Where the "clear all" action is rendered
    pub clear_button_placement: PanelClearButtonPlacement,
    /// Heading shown above toggle-style quick actions
    pub quick_actions_label: String,
    /// Heading shown above stat cards
    pub system_status_label: String,
    /// Heading shown above the notification list
    pub recent_notifications_label: String,
    /// Text shown on the notification clear action
    pub clear_label: String,
    /// Optional passive footer label. Empty hides the footer
    pub footer_label: String,
    /// Top-to-bottom panel body section order
    pub section_order: Vec<PanelSection>,
    /// Top-to-bottom widget section order
    pub widget_order: Vec<PanelWidgetSection>,
    /// Left-to-right action order inside the panel action row
    pub action_order: Vec<PanelActionId>,
    /// Widgets collapse/expand action customization
    pub focus_action: PanelActionConfig,
    /// Do-not-disturb action customization
    pub dnd_action: PanelActionConfig,
    /// Clear-notifications action customization
    pub clear_action: PanelActionConfig,
    /// Search action customization
    pub search_action: PanelActionConfig,
    /// Close action customization
    pub close_action: PanelActionConfig,
    /// Top offset in logical pixels for the empty-state label
    pub empty_offset_top: i32,
    /// Hide the panel when focus leaves the window
    pub close_on_blur: bool,
    /// Close the panel when a different window becomes active (Hyprland only)
    pub close_on_click_outside: bool,
    /// Respect compositor reserved work area when computing height (Hyprland only)
    pub respect_work_area: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::Right,
            margin: Margins {
                // Tuned for the default control-center layout shipped with UnixNotis
                // Keeps the panel clear of edges and compositor bars without feeling cramped
                top: 16,
                right: 10,
                bottom: 14,
                left: 10,
            },
            width: 420,
            height: PANEL_HEIGHT_PERCENT_DEFAULT,
            height_override: None,
            keyboard_interactivity: PanelKeyboardInteractivity::OnDemand,
            output: None,
            empty_text: "NO NOTIFICATIONS".to_string(),
            title: "Notifications".to_string(),
            subtitle: String::new(),
            search_placeholder: "Search app, title, or message".to_string(),
            search_visible: false,
            action_row_visible: true,
            notification_section_visible: false,
            notification_list_expand: true,
            notification_metadata_visible: false,
            notification_thumbnails_visible: false,
            clear_button_placement: PanelClearButtonPlacement::ActionRow,
            quick_actions_label: String::new(),
            system_status_label: String::new(),
            recent_notifications_label: "Notifications".to_string(),
            clear_label: "Clear".to_string(),
            footer_label: String::new(),
            section_order: default_panel_section_order(),
            widget_order: default_panel_widget_order(),
            action_order: default_panel_action_order(),
            focus_action: PanelActionConfig::widgets(),
            dnd_action: PanelActionConfig::dnd(),
            clear_action: PanelActionConfig::clear(),
            search_action: PanelActionConfig::search(),
            close_action: PanelActionConfig::close(),
            empty_offset_top: 120,
            close_on_blur: false,
            close_on_click_outside: true,
            respect_work_area: true,
        }
    }
}
