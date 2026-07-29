use super::super::build_legacy_theme_color_overrides;
use crate::ThemeConfig;

#[test]
fn legacy_theme_color_overrides_include_card_alpha() {
    let overrides = build_legacy_theme_color_overrides(&ThemeConfig {
        card_alpha: 0.42,
        ..ThemeConfig::default()
    });

    assert!(
        overrides.contains("@define-color unixnotis-card alpha(@unixnotis-card-base, 0.42);"),
        "legacy output should preserve the configured card alpha"
    );
}
