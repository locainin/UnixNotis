//! Popup layout configuration

use serde::{Deserialize, Serialize};

use super::{Anchor, Margins};

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
