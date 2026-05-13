//! Layout-related configuration types for the panel and popups.
//!
//! Keeps positioning and sizing settings grouped for easier maintenance.

use serde::{Deserialize, Serialize};

// Center runtime keeps side panels at or above this width
pub const PANEL_RUNTIME_WIDTH_MIN: i32 = 260;
// Panel height is configured as a percent of usable monitor height by default
pub const PANEL_HEIGHT_PERCENT_DEFAULT: i32 = 84;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PopupConfig {
    pub anchor: Anchor,
    pub margin: Margins,
    pub width: i32,
    pub spacing: i32,
    pub max_visible: usize,
    pub default_timeout_ms: u64,
    pub critical_timeout_ms: Option<u64>,
    pub allow_click_through: bool,
    pub output: Option<String>,
}

impl Default for PopupConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopRight,
            margin: Margins::default(),
            width: 360,
            spacing: 12,
            max_visible: 4,
            default_timeout_ms: 5000,
            critical_timeout_ms: None,
            allow_click_through: false,
            output: None,
        }
    }
}

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
    /// Top-to-bottom widget section order
    pub widget_order: Vec<PanelWidgetSection>,
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
                // Tuned for the default control-center layout shipped with UnixNotis.
                // Keeps the panel clear of edges and compositor bars without feeling cramped.
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
            clear_button_placement: PanelClearButtonPlacement::ActionRow,
            quick_actions_label: "Quick Actions".to_string(),
            system_status_label: "System Status".to_string(),
            recent_notifications_label: "Notifications".to_string(),
            clear_label: "Clear".to_string(),
            footer_label: String::new(),
            widget_order: default_panel_widget_order(),
            empty_offset_top: 120,
            close_on_blur: false,
            close_on_click_outside: true,
            respect_work_area: true,
        }
    }
}

/// Location for the panel-wide clear action
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PanelClearButtonPlacement {
    #[default]
    ActionRow,
    NotificationHeader,
    Hidden,
}

/// Top-level panel widget sections that can be ordered by config
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PanelWidgetSection {
    Media,
    Toggles,
    Sliders,
    Stats,
    Cards,
}

/// Return the stock top-to-bottom panel section order
pub fn default_panel_widget_order() -> Vec<PanelWidgetSection> {
    vec![
        PanelWidgetSection::Sliders,
        PanelWidgetSection::Media,
        PanelWidgetSection::Toggles,
        PanelWidgetSection::Stats,
        PanelWidgetSection::Cards,
    ]
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    /// Default anchor for panels when no explicit config value is set
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PanelKeyboardInteractivity {
    // Do not request keyboard focus; panel is purely pointer-driven.
    None,
    // Only request keyboard focus when an interaction requires it (search entry, text input, etc.).
    // Default keeps focus optional to avoid persistent grabs.
    #[default]
    OnDemand,
    // Always grab exclusive keyboard focus while the panel is open.
    Exclusive,
}

// Equality is derived to support work-area change detection without custom comparisons.
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Margins {
    // Pixel margins applied around the panel/control-center surface.
    // These are logical pixels (before output scaling), and are used both for aesthetics and
    // to keep the surface off the screen edge / away from reserved work area.
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Default for Margins {
    fn default() -> Self {
        // Default padding around the panel. Keeping it symmetric produces a balanced look by default.
        // Users can override individual edges in config for tighter or asymmetric layouts.
        Self {
            // Matches the default popup stack spacing for a cohesive baseline layout.
            top: 14,
            right: 14,
            bottom: 14,
            left: 14,
        }
    }
}
