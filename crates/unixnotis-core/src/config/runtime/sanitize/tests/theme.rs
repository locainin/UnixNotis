#![allow(
    clippy::float_cmp,
    reason = "theme sanitization assigns exact clamp boundaries and explicit fallback constants"
)]

use super::super::*;
use crate::{Config, ThemeConfig};

type ThemeMutation = fn(&mut crate::ThemeConfig);
type ThemeField = fn(&crate::ThemeConfig) -> f32;

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
    config.theme.notification_corners.top_left = u16::MAX;
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
    assert_eq!(config.theme.notification_corners.top_left, MAX_CORNER_CUT);
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

#[test]
fn sanitize_theme_defaults_when_any_alpha_field_is_non_finite() {
    let cases: [(ThemeMutation, ThemeField); 5] = [
        (
            |theme| theme.surface_alpha = f32::NAN,
            |theme| theme.surface_alpha,
        ),
        (
            |theme| theme.surface_strong_alpha = f32::INFINITY,
            |theme| theme.surface_strong_alpha,
        ),
        (
            |theme| theme.card_alpha = f32::NEG_INFINITY,
            |theme| theme.card_alpha,
        ),
        (
            |theme| theme.shadow_soft_alpha = f32::NAN,
            |theme| theme.shadow_soft_alpha,
        ),
        (
            |theme| theme.shadow_strong_alpha = f32::INFINITY,
            |theme| theme.shadow_strong_alpha,
        ),
    ];

    for (make_non_finite, changed_field) in cases {
        let mut config = Config::default();
        let defaults = ThemeConfig::default();
        config.theme.surface_alpha = 0.12;
        config.theme.surface_strong_alpha = 0.23;
        config.theme.card_alpha = 0.34;
        config.theme.shadow_soft_alpha = 0.45;
        config.theme.shadow_strong_alpha = 0.56;
        make_non_finite(&mut config.theme);

        sanitize_config(&mut config);

        assert_eq!(changed_field(&config.theme), changed_field(&defaults));
        assert!(config.theme.surface_alpha.is_finite());
        assert!(config.theme.surface_strong_alpha.is_finite());
        assert!(config.theme.card_alpha.is_finite());
        assert!(config.theme.shadow_soft_alpha.is_finite());
        assert!(config.theme.shadow_strong_alpha.is_finite());
    }
}
