//! GTK color definitions supported by every compatible GTK version

use crate::config::ThemeConfig;

use super::model::clamp_alpha;

#[must_use]
pub fn build_legacy_theme_color_overrides(theme: &ThemeConfig) -> String {
    // Legacy alpha colors stay first so existing theme palettes remain stable
    let surface_alpha = clamp_alpha(theme.surface_alpha);
    let surface_strong_alpha = clamp_alpha(theme.surface_strong_alpha);
    let card_alpha = clamp_alpha(theme.card_alpha);
    let shadow_soft = clamp_alpha(theme.shadow_soft_alpha);
    let shadow_strong = clamp_alpha(theme.shadow_strong_alpha);

    format!(
        r"
@define-color unixnotis-surface alpha(@unixnotis-surface-base, {surface_alpha});
@define-color unixnotis-surface-strong alpha(@unixnotis-surface-strong-base, {surface_strong_alpha});
@define-color unixnotis-card alpha(@unixnotis-card-base, {card_alpha});
@define-color unixnotis-shadow-soft alpha(#000000, {shadow_soft});
@define-color unixnotis-shadow-strong alpha(#000000, {shadow_strong});
"
    )
}
