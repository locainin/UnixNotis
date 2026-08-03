//! Theme-driven CSS overrides used by the UI CSS manager

use unixnotis_core::{
    build_legacy_theme_color_overrides, build_modern_theme_custom_properties,
    theme_card_style_values, ThemeConfig,
};

pub fn build_base_overrides(theme: &ThemeConfig) -> String {
    // Legacy color aliases remain first so every generated token has a stable source
    let mut overrides = build_legacy_theme_color_overrides(theme);
    // GTK 4.18 is the supported baseline for the custom-property theme contract
    overrides.push_str(&build_modern_theme_custom_properties(theme));
    overrides
}

pub fn build_panel_overrides(theme: &ThemeConfig) -> String {
    // Panel and widgets share the same card shell values
    let card_style = theme_card_style_values(theme);
    format!(
        r"
.unixnotis-panel-card {{
  border-width: {}px;
  border-style: solid;
  border-radius: {}px;
  background: @unixnotis-card;
}}
",
        card_style.border_width_px, card_style.card_radius_px,
    )
}

pub fn build_widgets_overrides(theme: &ThemeConfig) -> String {
    // Media cards use the same base shell so one theme knob moves both
    let card_style = theme_card_style_values(theme);
    format!(
        r"
.unixnotis-media-card {{
  border-width: {}px;
  border-style: solid;
  border-radius: {}px;
  background: @unixnotis-card;
}}
",
        card_style.border_width_px, card_style.card_radius_px,
    )
}

pub fn build_popup_overrides(theme: &ThemeConfig) -> String {
    // Popups keep the same border and radius contract as the panel cards
    let card_style = theme_card_style_values(theme);
    format!(
        r"
.unixnotis-popup-card {{
  border-width: {}px;
  border-style: solid;
  border-radius: {}px;
}}
",
        card_style.border_width_px, card_style.card_radius_px,
    )
}

#[cfg(test)]
#[path = "tests/overrides.rs"]
mod tests;
