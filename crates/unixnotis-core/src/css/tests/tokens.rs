use super::{
    build_legacy_theme_color_overrides, build_modern_theme_custom_properties,
    theme_card_style_values,
};
use crate::{gtk_css_features_for_version, ThemeConfig};

#[test]
fn theme_card_style_values_clamp_alpha_and_keep_lengths() {
    let values = theme_card_style_values(&ThemeConfig {
        border_width: 3,
        card_radius: 18,
        card_alpha: 1.5,
        ..ThemeConfig::default()
    });

    assert_eq!(values.border_width_px, 3.0);
    assert_eq!(values.card_radius_px, 18.0);
    assert_eq!(values.card_alpha, 1.0);
}

#[test]
fn legacy_theme_color_overrides_include_card_alpha() {
    let overrides = build_legacy_theme_color_overrides(&ThemeConfig {
        card_alpha: 0.42,
        ..ThemeConfig::default()
    });

    assert!(overrides.contains("@define-color unixnotis-card alpha(@unixnotis-card-base, 0.42);"));
}

#[test]
fn modern_theme_custom_properties_stay_additive() {
    let overrides = build_modern_theme_custom_properties(
        &ThemeConfig {
            border_width: 2,
            card_radius: 12,
            surface_alpha: 0.88,
            ..ThemeConfig::default()
        },
        gtk_css_features_for_version(4, 16),
    );

    assert!(overrides.contains(":root {"));
    assert!(overrides.contains("--unixnotis-border-width: 2px;"));
    assert!(overrides.contains("--unixnotis-card-radius: 12px;"));
    assert!(overrides.contains("--unixnotis-panel-card-padding-y: 10px;"));
    assert!(overrides.contains("--unixnotis-popup-reveal-duration: 200ms;"));
    assert!(overrides.contains("--unixnotis-accent-color: @unixnotis-accent;"));
}

#[test]
fn modern_theme_custom_properties_stay_off_on_older_gtk() {
    let overrides = build_modern_theme_custom_properties(
        &ThemeConfig::default(),
        gtk_css_features_for_version(4, 15),
    );
    assert!(overrides.is_empty());
}
