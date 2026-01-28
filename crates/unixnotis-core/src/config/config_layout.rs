//! Layout-related configuration types for the panel and popups.
//!
//! Keeps positioning and sizing settings grouped for easier maintenance.

use serde::{Deserialize, Serialize};

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
    pub height: i32,
    pub keyboard_interactivity: PanelKeyboardInteractivity,
    pub output: Option<String>,
    /// Text shown when the notification list is empty.
    pub empty_text: String,
    /// Top offset in logical pixels for the empty-state label.
    pub empty_offset_top: i32,
    /// Hide the panel when focus leaves the window.
    pub close_on_blur: bool,
    /// Close the panel when a different window becomes active (Hyprland only).
    pub close_on_click_outside: bool,
    /// Respect compositor reserved work area when computing height (Hyprland only).
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
            height: 0,
            keyboard_interactivity: PanelKeyboardInteractivity::OnDemand,
            output: None,
            empty_text: "NO NOTIFICATIONS".to_string(),
            empty_offset_top: 120,
            close_on_blur: false,
            close_on_click_outside: true,
            respect_work_area: true,
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    /// Default anchor for panels when no explicit config value is set.
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

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
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
