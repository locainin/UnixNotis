//! Theme configuration values and defaults

use serde::{Deserialize, Serialize};

use super::corners::CutCorners;

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Stock,
    Custom,
}

impl ThemeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stock => "stock",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Embedded stock or explicitly enabled versioned custom CSS
    pub mode: ThemeMode,
    #[serde(alias = "style_css")]
    pub base_css: String,
    pub popup_css: String,
    pub panel_css: String,
    pub widgets_css: String,
    /// Media widget theme layer loaded above widgets.css for layout-specific ricing.
    pub media_css: String,
    /// Border thickness for cards and controls (pixels).
    pub border_width: u8,
    /// Corner radius for notification cards (pixels).
    pub card_radius: u8,
    /// True diagonal cuts applied to panel and popup notification cards
    pub notification_corners: CutCorners,
    /// Base alpha for panel surfaces (0.0 - 1.0).
    pub surface_alpha: f32,
    /// Stronger alpha for panel surfaces (0.0 - 1.0).
    pub surface_strong_alpha: f32,
    /// Global alpha for card surfaces (0.0 - 1.0).
    pub card_alpha: f32,
    /// Alpha for softer drop shadows (0.0 - 1.0).
    pub shadow_soft_alpha: f32,
    /// Alpha for stronger drop shadows (0.0 - 1.0).
    pub shadow_strong_alpha: f32,
}

#[cfg(test)]
#[path = "tests/theme.rs"]
mod tests;

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Stock,
            base_css: "base.css".to_string(),
            popup_css: "popup.css".to_string(),
            panel_css: "panel.css".to_string(),
            widgets_css: "widgets.css".to_string(),
            media_css: "media.css".to_string(),
            border_width: 1,
            // Matches the default card radius used by the bundled theme.
            card_radius: 22,
            // Square clipping preserves the existing rounded CSS presentation
            notification_corners: CutCorners::default(),
            surface_alpha: 0.88,
            surface_strong_alpha: 0.96,
            card_alpha: 0.94,
            shadow_soft_alpha: 0.30,
            // Slightly stronger to preserve depth on dark backgrounds.
            shadow_strong_alpha: 0.64,
        }
    }
}
