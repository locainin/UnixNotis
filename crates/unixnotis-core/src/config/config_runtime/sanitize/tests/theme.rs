use super::super::*;
use crate::{Config, ThemeConfig};

#[test]
fn sanitize_clamps_alpha_and_theme_limits() {
    // Non-finite alpha values should fall back while finite ones still clamp
    let mut config = Config::default();
    let theme_defaults = ThemeConfig::default();
    config.theme.surface_alpha = -0.25;
    config.theme.surface_strong_alpha = 1.25;
    config.theme.card_alpha = f32::NAN;
    config.theme.shadow_soft_alpha = f32::INFINITY;
    config.theme.shadow_strong_alpha = -0.5;
    config.theme.border_width = MAX_BORDER_WIDTH + 2;
    config.theme.card_radius = MAX_CARD_RADIUS + 3;
    sanitize_config(&mut config);

    assert_eq!(config.theme.surface_alpha, 0.0);
    assert_eq!(config.theme.surface_strong_alpha, 1.0);
    assert!(
        (config.theme.card_alpha - theme_defaults.card_alpha).abs() < f32::EPSILON,
        "card alpha fallback should match theme default"
    );
    assert!(
        (config.theme.shadow_soft_alpha - theme_defaults.shadow_soft_alpha).abs() < f32::EPSILON,
        "shadow soft alpha fallback should match theme default"
    );
    assert_eq!(config.theme.shadow_strong_alpha, 0.0);
    assert_eq!(config.theme.border_width, MAX_BORDER_WIDTH);
    assert_eq!(config.theme.card_radius, MAX_CARD_RADIUS);
}

#[test]
fn sanitize_clamps_alpha_without_defaults() {
    // Finite alpha values should clamp without forcing a full theme reset
    let mut config = Config::default();
    config.theme.surface_alpha = 1.5;
    config.theme.surface_strong_alpha = -0.2;
    config.theme.card_alpha = 0.2;
    config.theme.shadow_soft_alpha = 2.0;
    config.theme.shadow_strong_alpha = -1.0;
    sanitize_config(&mut config);

    assert_eq!(config.theme.surface_alpha, 1.0);
    assert_eq!(config.theme.surface_strong_alpha, 0.0);
    assert_eq!(config.theme.card_alpha, 0.2);
    assert_eq!(config.theme.shadow_soft_alpha, 1.0);
    assert_eq!(config.theme.shadow_strong_alpha, 0.0);
}
