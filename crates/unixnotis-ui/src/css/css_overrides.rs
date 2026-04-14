//! Theme-driven CSS overrides used by the UI CSS manager.

use gtk::{major_version, minor_version};
use unixnotis_core::{
    build_legacy_theme_color_overrides, build_modern_theme_custom_properties,
    gtk_css_features_for_version, theme_card_style_values, GtkCssFeatures, ThemeConfig,
};

pub(crate) fn build_base_overrides(theme: &ThemeConfig) -> String {
    // Runtime gating keeps older GTK builds on the legacy-safe token path
    build_base_overrides_for_runtime(theme, current_gtk_css_features())
}

fn build_base_overrides_for_runtime(theme: &ThemeConfig, features: GtkCssFeatures) -> String {
    // Legacy colors stay first so older GTK still has the same theme path
    let mut overrides = build_legacy_theme_color_overrides(theme);
    // Modern tokens are additive and only show up on runtimes that can parse them
    overrides.push_str(&build_modern_theme_custom_properties(theme, features));
    overrides
}

pub(crate) fn build_panel_overrides(theme: &ThemeConfig) -> String {
    // Panel and widgets share the same card shell values
    let card_style = theme_card_style_values(theme);
    format!(
        r#"
.unixnotis-panel-card {{
  border-width: {}px;
  border-style: solid;
  border-radius: {}px;
  background: @unixnotis-card;
}}
"#,
        card_style.border_width_px, card_style.card_radius_px,
    )
}

pub(crate) fn build_widgets_overrides(theme: &ThemeConfig) -> String {
    // Media cards use the same base shell so one theme knob moves both
    let card_style = theme_card_style_values(theme);
    format!(
        r#"
.unixnotis-media-card {{
  border-width: {}px;
  border-style: solid;
  border-radius: {}px;
  background: @unixnotis-card;
}}
"#,
        card_style.border_width_px, card_style.card_radius_px,
    )
}

pub(crate) fn build_popup_overrides(theme: &ThemeConfig) -> String {
    // Popups keep the same border and radius contract as the panel cards
    let card_style = theme_card_style_values(theme);
    format!(
        r#"
.unixnotis-popup-card {{
  border-width: {}px;
  border-style: solid;
  border-radius: {}px;
}}
"#,
        card_style.border_width_px, card_style.card_radius_px,
    )
}

fn current_gtk_css_features() -> GtkCssFeatures {
    // Runtime GTK version decides whether custom properties can be emitted safely
    gtk_css_features_for_version(major_version(), minor_version())
}

#[cfg(test)]
#[path = "css_overrides_tests.rs"]
mod tests;
