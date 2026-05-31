//! Shared layout primitives used by panel and popup config

use serde::{Deserialize, Serialize};

// Center runtime keeps side panels at or above this width
pub const PANEL_RUNTIME_WIDTH_MIN: i32 = 260;
// Panel height is configured as a percent of usable monitor height by default
pub const PANEL_HEIGHT_PERCENT_DEFAULT: i32 = 84;

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
