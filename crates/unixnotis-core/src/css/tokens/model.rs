//! Config-backed values shared by CSS token renderers

use crate::config::ThemeConfig;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeCardStyleValues {
    // These values are reused by several override builders
    pub border_width_px: f32,
    pub card_radius_px: f32,
    pub card_alpha: f32,
}

#[must_use]
pub fn theme_card_style_values(theme: &ThemeConfig) -> ThemeCardStyleValues {
    ThemeCardStyleValues {
        border_width_px: f32::from(theme.border_width),
        card_radius_px: f32::from(theme.card_radius),
        card_alpha: clamp_alpha(theme.card_alpha),
    }
}

pub(super) const fn clamp_alpha(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}
