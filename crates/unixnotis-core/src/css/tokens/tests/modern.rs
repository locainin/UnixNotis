use super::super::build_modern_theme_custom_properties;
use crate::{gtk_css_features_for_version, ThemeConfig};

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

    for expected in [
        ":root {",
        "--unixnotis-border-width: 2px;",
        "--unixnotis-card-radius: 12px;",
        "--unixnotis-notification-card-radius: 12px;",
        "--unixnotis-panel-card-padding-y: 9px;",
        "--unixnotis-popup-card-padding-y: 10px;",
        "--unixnotis-popup-reveal-duration: 200ms;",
        "--unixnotis-media-card-radius: 18px;",
        "--unixnotis-media-title-font-size: 13px;",
        "--unixnotis-ui-font-family: \"Inter\", \"SF Pro Text\",",
        "--unixnotis-monospace-font-family: \"CaskaydiaCove Nerd Font Mono\",",
        "--unixnotis-accent-color: @unixnotis-accent;",
        "--unixnotis-surface-alpha: 0.88;",
        "--unixnotis-card-alpha: 0.94;",
    ] {
        assert!(
            overrides.contains(expected),
            "modern token output should contain {expected}"
        );
    }
}

#[test]
fn modern_theme_custom_properties_stay_off_on_older_gtk() {
    let overrides = build_modern_theme_custom_properties(
        &ThemeConfig::default(),
        gtk_css_features_for_version(4, 15),
    );
    assert!(
        overrides.is_empty(),
        "GTK versions without custom properties should receive no modern block"
    );
}

#[test]
fn modern_theme_tokens_trim_float_values_without_losing_fraction() {
    let overrides = build_modern_theme_custom_properties(
        &ThemeConfig {
            border_width: 3,
            card_radius: 10,
            surface_alpha: 0.5,
            surface_strong_alpha: 1.0,
            card_alpha: 0.125,
            ..ThemeConfig::default()
        },
        gtk_css_features_for_version(4, 16),
    );

    for expected in [
        "--unixnotis-border-width: 3px;",
        "--unixnotis-surface-alpha: 0.5;",
        "--unixnotis-surface-strong-alpha: 1;",
        "--unixnotis-card-alpha: 0.125;",
    ] {
        assert!(
            overrides.contains(expected),
            "trimmed modern output should contain {expected}"
        );
    }
}
