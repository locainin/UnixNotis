use super::{MAX_BORDER_WIDTH, MAX_CARD_RADIUS};
use crate::{Config, ThemeConfig};

pub(super) fn sanitize_theme_config(config: &mut Config) {
    let theme = &mut config.theme;
    let needs_theme_defaults = !theme.surface_alpha.is_finite()
        || !theme.surface_strong_alpha.is_finite()
        || !theme.card_alpha.is_finite()
        || !theme.shadow_soft_alpha.is_finite()
        || !theme.shadow_strong_alpha.is_finite();

    if needs_theme_defaults {
        let theme_defaults = ThemeConfig::default();
        clamp_alpha(&mut theme.surface_alpha, theme_defaults.surface_alpha);
        clamp_alpha(
            &mut theme.surface_strong_alpha,
            theme_defaults.surface_strong_alpha,
        );
        clamp_alpha(&mut theme.card_alpha, theme_defaults.card_alpha);
        clamp_alpha(
            &mut theme.shadow_soft_alpha,
            theme_defaults.shadow_soft_alpha,
        );
        clamp_alpha(
            &mut theme.shadow_strong_alpha,
            theme_defaults.shadow_strong_alpha,
        );
    } else {
        clamp_alpha_finite(&mut theme.surface_alpha);
        clamp_alpha_finite(&mut theme.surface_strong_alpha);
        clamp_alpha_finite(&mut theme.card_alpha);
        clamp_alpha_finite(&mut theme.shadow_soft_alpha);
        clamp_alpha_finite(&mut theme.shadow_strong_alpha);
    }

    // CSS generation reads these directly, so keep values inside simple visual bounds
    config.theme.border_width = config.theme.border_width.min(MAX_BORDER_WIDTH);
    config.theme.card_radius = config.theme.card_radius.min(MAX_CARD_RADIUS);
}

fn clamp_alpha(value: &mut f32, fallback: f32) {
    if !value.is_finite() {
        *value = fallback;
        return;
    }
    *value = value.clamp(0.0, 1.0);
}

fn clamp_alpha_finite(value: &mut f32) {
    *value = value.clamp(0.0, 1.0);
}

#[cfg(test)]
#[path = "../../tests/runtime/theme.rs"]
mod tests;
