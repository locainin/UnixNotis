use super::{CutCorners, ThemeConfig};

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

#[test]
fn default_theme_keeps_notification_corner_clipping_disabled() {
    let theme = ThemeConfig::default();

    assert!(!theme.notification_corners.is_active());
}

#[test]
fn every_individual_cut_corner_enables_clipping() {
    for corners in [
        CutCorners {
            top_left: 1,
            ..CutCorners::default()
        },
        CutCorners {
            top_right: 1,
            ..CutCorners::default()
        },
        CutCorners {
            bottom_right: 1,
            ..CutCorners::default()
        },
        CutCorners {
            bottom_left: 1,
            ..CutCorners::default()
        },
    ] {
        assert!(corners.is_active());
    }
}
