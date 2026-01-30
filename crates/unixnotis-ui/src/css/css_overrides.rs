//! Theme-driven CSS overrides used by the UI CSS manager.

use unixnotis_core::ThemeConfig;

pub(crate) fn build_base_overrides(theme: &ThemeConfig) -> String {
    // Clamp alpha values to avoid invalid CSS and keep overrides predictable.
    let surface_alpha = theme.surface_alpha.clamp(0.0, 1.0);
    let surface_strong_alpha = theme.surface_strong_alpha.clamp(0.0, 1.0);
    let shadow_soft = theme.shadow_soft_alpha.clamp(0.0, 1.0);
    let shadow_strong = theme.shadow_strong_alpha.clamp(0.0, 1.0);
    format!(
        r#"
@define-color unixnotis-surface alpha(@unixnotis-surface-base, {surface_alpha});
@define-color unixnotis-surface-strong alpha(@unixnotis-surface-strong-base, {surface_strong_alpha});
@define-color unixnotis-shadow-soft alpha(#000000, {shadow_soft});
@define-color unixnotis-shadow-strong alpha(#000000, {shadow_strong});
"#
    )
}

pub(crate) fn build_panel_overrides(theme: &ThemeConfig) -> String {
    let border_width = theme.border_width as f32;
    let card_radius = theme.card_radius as f32;
    let card_alpha = theme.card_alpha.clamp(0.0, 1.0);
    format!(
        r#"
.unixnotis-panel-card {{
  border-width: {border_width}px;
  border-style: solid;
  border-radius: {card_radius}px;
  background: alpha(@unixnotis-card-base, {card_alpha});
}}
"#
    )
}

pub(crate) fn build_widgets_overrides(theme: &ThemeConfig) -> String {
    let border_width = theme.border_width as f32;
    let card_radius = theme.card_radius as f32;
    let card_alpha = theme.card_alpha.clamp(0.0, 1.0);
    format!(
        r#"
.unixnotis-media-card {{
  border-width: {border_width}px;
  border-style: solid;
  border-radius: {card_radius}px;
  background: alpha(@unixnotis-card-base, {card_alpha});
}}
"#
    )
}

pub(crate) fn build_popup_overrides(theme: &ThemeConfig) -> String {
    let border_width = theme.border_width as f32;
    let card_radius = theme.card_radius as f32;
    format!(
        r#"
.unixnotis-popup-card {{
  border-width: {border_width}px;
  border-style: solid;
  border-radius: {card_radius}px;
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_base_overrides, build_panel_overrides, build_popup_overrides, build_widgets_overrides,
    };
    use unixnotis_core::ThemeConfig;

    #[test]
    fn base_overrides_clamp_alpha_values() {
        // Confirms alpha values are clamped into the CSS-friendly [0.0, 1.0] range.
        let theme = ThemeConfig {
            surface_alpha: 1.5,
            surface_strong_alpha: -0.25,
            shadow_soft_alpha: 2.0,
            shadow_strong_alpha: -1.0,
            ..ThemeConfig::default()
        };

        let overrides = build_base_overrides(&theme);
        let surface = format!(
            "alpha(@unixnotis-surface-base, {})",
            1.0_f32.clamp(0.0, 1.0)
        );
        let surface_strong = format!(
            "alpha(@unixnotis-surface-strong-base, {})",
            (-0.25_f32).clamp(0.0, 1.0)
        );
        let shadow_soft = format!("alpha(#000000, {})", 2.0_f32.clamp(0.0, 1.0));
        let shadow_strong = format!("alpha(#000000, {})", (-1.0_f32).clamp(0.0, 1.0));

        assert!(overrides.contains(&surface));
        assert!(overrides.contains(&surface_strong));
        assert!(overrides.contains(&shadow_soft));
        assert!(overrides.contains(&shadow_strong));
    }

    #[test]
    fn panel_overrides_use_theme_values() {
        // Ensures panel overrides reflect the configured card styling values.
        let theme = ThemeConfig {
            border_width: 3,
            card_radius: 12,
            card_alpha: 0.42,
            ..ThemeConfig::default()
        };

        let overrides = build_panel_overrides(&theme);
        assert!(overrides.contains("border-width: 3px;"));
        assert!(overrides.contains("border-radius: 12px;"));
        assert!(overrides.contains("alpha(@unixnotis-card-base, 0.42"));
    }

    #[test]
    fn widgets_overrides_use_theme_values() {
        // Ensures widget card styling uses the configured theme values.
        let theme = ThemeConfig {
            border_width: 2,
            card_radius: 8,
            card_alpha: 0.77,
            ..ThemeConfig::default()
        };

        let overrides = build_widgets_overrides(&theme);
        assert!(overrides.contains("border-width: 2px;"));
        assert!(overrides.contains("border-radius: 8px;"));
        assert!(overrides.contains("alpha(@unixnotis-card-base, 0.77"));
    }

    #[test]
    fn popup_overrides_use_theme_values() {
        // Ensures popup card styling uses the configured theme values.
        let theme = ThemeConfig {
            border_width: 5,
            card_radius: 24,
            ..ThemeConfig::default()
        };

        let overrides = build_popup_overrides(&theme);
        assert!(overrides.contains("border-width: 5px;"));
        assert!(overrides.contains("border-radius: 24px;"));
    }
}
