use super::ThemeConfig;

#[test]
fn default_theme_opacity_values_stay_within_css_alpha_bounds() {
    let theme = ThemeConfig::default();

    for alpha in [
        theme.surface_alpha,
        theme.surface_strong_alpha,
        theme.card_alpha,
        theme.shadow_soft_alpha,
        theme.shadow_strong_alpha,
    ] {
        assert!((0.0..=1.0).contains(&alpha));
    }
}
