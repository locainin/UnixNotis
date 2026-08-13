//! Popup layout configuration

use serde::{Deserialize, Serialize};

use super::{Anchor, Margins};

/// Longest automatic popup timer accepted by the freedesktop millisecond domain
pub const MAX_POPUP_TIMEOUT_MS: u64 = 2_147_483_647;

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
            margin: Margins {
                top: 14,
                right: 18,
                bottom: 14,
                left: 18,
            },
            width: 360,
            spacing: 12,
            max_visible: 3,
            default_timeout_ms: 5000,
            critical_timeout_ms: None,
            allow_click_through: false,
            output: None,
        }
    }
}
