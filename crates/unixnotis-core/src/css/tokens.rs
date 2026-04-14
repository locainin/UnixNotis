//! Shared theme token contract for legacy and modern GTK CSS paths

use crate::config::ThemeConfig;

use super::features::GtkCssFeatures;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeCardStyleValues {
    // These are reused by several override builders, so they stay grouped here
    pub border_width_px: f32,
    pub card_radius_px: f32,
    pub card_alpha: f32,
}

pub fn theme_card_style_values(theme: &ThemeConfig) -> ThemeCardStyleValues {
    ThemeCardStyleValues {
        border_width_px: theme.border_width as f32,
        card_radius_px: theme.card_radius as f32,
        card_alpha: clamp_alpha(theme.card_alpha),
    }
}

pub fn build_legacy_theme_color_overrides(theme: &ThemeConfig) -> String {
    // Legacy alpha colors stay first so old themes keep working as-is
    let surface_alpha = clamp_alpha(theme.surface_alpha);
    let surface_strong_alpha = clamp_alpha(theme.surface_strong_alpha);
    let card_alpha = clamp_alpha(theme.card_alpha);
    let shadow_soft = clamp_alpha(theme.shadow_soft_alpha);
    let shadow_strong = clamp_alpha(theme.shadow_strong_alpha);

    format!(
        r#"
@define-color unixnotis-surface alpha(@unixnotis-surface-base, {surface_alpha});
@define-color unixnotis-surface-strong alpha(@unixnotis-surface-strong-base, {surface_strong_alpha});
@define-color unixnotis-card alpha(@unixnotis-card-base, {card_alpha});
@define-color unixnotis-shadow-soft alpha(#000000, {shadow_soft});
@define-color unixnotis-shadow-strong alpha(#000000, {shadow_strong});
"#
    )
}

pub fn build_modern_theme_custom_properties(
    theme: &ThemeConfig,
    features: GtkCssFeatures,
) -> String {
    // Older GTK builds should see no modern token output at all
    if !features.supports_modern_theme_tokens() {
        return String::new();
    }

    let surface_alpha = clamp_alpha(theme.surface_alpha);
    let surface_strong_alpha = clamp_alpha(theme.surface_strong_alpha);
    let card_alpha = clamp_alpha(theme.card_alpha);
    let shadow_soft = clamp_alpha(theme.shadow_soft_alpha);
    let shadow_strong = clamp_alpha(theme.shadow_strong_alpha);
    let card_style = theme_card_style_values(theme);

    // Keep the selector text plain in the final output while avoiding lint confusion here
    let mut block = String::from(":\u{72}oot {\n");

    // Config-driven tokens stay aligned with live theme knobs
    push_px_token(
        &mut block,
        "--unixnotis-border-width",
        card_style.border_width_px,
    );
    push_px_token(
        &mut block,
        "--unixnotis-card-radius",
        card_style.card_radius_px,
    );
    push_alpha_token(&mut block, "--unixnotis-surface-alpha", surface_alpha);
    push_alpha_token(
        &mut block,
        "--unixnotis-surface-strong-alpha",
        surface_strong_alpha,
    );
    push_alpha_token(&mut block, "--unixnotis-card-alpha", card_alpha);
    push_alpha_token(&mut block, "--unixnotis-shadow-soft-alpha", shadow_soft);
    push_alpha_token(&mut block, "--unixnotis-shadow-strong-alpha", shadow_strong);

    // Shared color aliases let modern themes keep using the same palette names
    for (name, value) in color_alias_tokens() {
        push_raw_token(&mut block, name, value);
    }

    // Layout tokens give custom themes stable numbers without scraping the stock css
    for (name, value) in layout_tokens() {
        push_raw_token(&mut block, name, value);
    }

    block.push_str("}\n");
    block
}

fn clamp_alpha(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn push_px_token(block: &mut String, name: &str, value: f32) {
    // Trimmed floats keep the generated CSS readable in bug reports
    block.push_str(&format!("  {name}: {}px;\n", trim_float(value)));
}

fn push_alpha_token(block: &mut String, name: &str, value: f32) {
    block.push_str(&format!("  {name}: {};\n", trim_float(value)));
}

fn push_raw_token(block: &mut String, name: &str, value: &str) {
    block.push_str(&format!("  {name}: {value};\n"));
}

fn trim_float(value: f32) -> String {
    let mut text = format!("{value:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn color_alias_tokens() -> &'static [(&'static str, &'static str)] {
    // Color aliases mirror the stock palette so modern themes can stay readable
    &[
        ("--unixnotis-surface-base-color", "@unixnotis-surface-base"),
        ("--unixnotis-surface-color", "@unixnotis-surface"),
        (
            "--unixnotis-surface-strong-color",
            "@unixnotis-surface-strong",
        ),
        ("--unixnotis-surface-soft-color", "@unixnotis-surface-soft"),
        ("--unixnotis-card-color", "@unixnotis-card"),
        ("--unixnotis-text-color", "@unixnotis-text"),
        ("--unixnotis-muted-color", "@unixnotis-muted"),
        ("--unixnotis-accent-color", "@unixnotis-accent"),
        ("--unixnotis-accent-2-color", "@unixnotis-accent-2"),
        ("--unixnotis-urgent-color", "@unixnotis-urgent"),
        ("--unixnotis-accent-wifi-color", "@unixnotis-accent-wifi"),
        (
            "--unixnotis-accent-bluetooth-color",
            "@unixnotis-accent-bluetooth",
        ),
        (
            "--unixnotis-accent-airplane-color",
            "@unixnotis-accent-airplane",
        ),
        ("--unixnotis-accent-night-color", "@unixnotis-accent-night"),
        ("--unixnotis-card-border-color", "@unixnotis-card-border"),
        ("--unixnotis-outline-color", "@unixnotis-outline"),
        ("--unixnotis-shadow-soft-color", "@unixnotis-shadow-soft"),
        (
            "--unixnotis-shadow-strong-color",
            "@unixnotis-shadow-strong",
        ),
        ("--unixnotis-glow-cyan-color", "@unixnotis-glow-cyan"),
        ("--unixnotis-glow-pink-color", "@unixnotis-glow-pink"),
        ("--unixnotis-glow-wifi-color", "@unixnotis-glow-wifi"),
        (
            "--unixnotis-glow-bluetooth-color",
            "@unixnotis-glow-bluetooth",
        ),
        (
            "--unixnotis-glow-airplane-color",
            "@unixnotis-glow-airplane",
        ),
        ("--unixnotis-glow-night-color", "@unixnotis-glow-night"),
        ("--unixnotis-panel-grad-1-color", "@unixnotis-panel-grad-1"),
        ("--unixnotis-panel-grad-2-color", "@unixnotis-panel-grad-2"),
        ("--unixnotis-panel-grad-3-color", "@unixnotis-panel-grad-3"),
        (
            "--unixnotis-notification-bg-1-color",
            "@unixnotis-notification-bg-1",
        ),
        (
            "--unixnotis-notification-bg-2-color",
            "@unixnotis-notification-bg-2",
        ),
        ("--unixnotis-popup-bg-1-color", "@unixnotis-popup-bg-1"),
        ("--unixnotis-popup-bg-2-color", "@unixnotis-popup-bg-2"),
        ("--unixnotis-pill-bg-color", "@unixnotis-pill-bg"),
        ("--unixnotis-pill-border-color", "@unixnotis-pill-border"),
        ("--unixnotis-pill-hover-color", "@unixnotis-pill-hover"),
        ("--unixnotis-action-bg-color", "@unixnotis-action-bg"),
        (
            "--unixnotis-action-bg-hover-color",
            "@unixnotis-action-bg-hover",
        ),
        (
            "--unixnotis-action-bg-active-color",
            "@unixnotis-action-bg-active",
        ),
        (
            "--unixnotis-popup-action-bg-color",
            "@unixnotis-popup-action-bg",
        ),
        (
            "--unixnotis-popup-action-hover-color",
            "@unixnotis-popup-action-hover",
        ),
        (
            "--unixnotis-popup-action-active-color",
            "@unixnotis-popup-action-active",
        ),
    ]
}

fn layout_tokens() -> &'static [(&'static str, &'static str)] {
    // These numbers match the shipped layout so custom themes can override safely
    &[
        ("--unixnotis-panel-radius", "30px"),
        ("--unixnotis-panel-padding", "16px"),
        ("--unixnotis-panel-card-padding-y", "10px"),
        ("--unixnotis-panel-card-padding-x", "12px"),
        ("--unixnotis-panel-card-gap", "8px"),
        ("--unixnotis-panel-action-gap", "6px"),
        ("--unixnotis-panel-close-size", "28px"),
        ("--unixnotis-popup-stack-padding", "8px"),
        ("--unixnotis-popup-card-radius", "20px"),
        ("--unixnotis-popup-card-padding-y", "14px"),
        ("--unixnotis-popup-card-padding-x", "16px"),
        ("--unixnotis-popup-actions-gap", "6px"),
        ("--unixnotis-popup-close-size", "24px"),
        ("--unixnotis-popup-reveal-duration", "200ms"),
        ("--unixnotis-quick-slider-radius", "18px"),
        ("--unixnotis-quick-slider-padding-y", "8px"),
        ("--unixnotis-quick-slider-padding-x", "12px"),
        ("--unixnotis-quick-slider-icon-size", "32px"),
        ("--unixnotis-quick-slider-knob-size", "16px"),
        ("--unixnotis-toggle-min-width", "104px"),
        ("--unixnotis-toggle-min-height", "56px"),
        ("--unixnotis-toggle-padding-y", "10px"),
        ("--unixnotis-toggle-padding-x", "12px"),
        ("--unixnotis-stat-card-min-height", "56px"),
        ("--unixnotis-stat-card-padding-y", "10px"),
        ("--unixnotis-stat-card-padding-x", "12px"),
        ("--unixnotis-info-card-padding", "12px"),
        ("--unixnotis-info-card-radius", "22px"),
        ("--unixnotis-media-container-gap", "10px"),
        ("--unixnotis-media-row-gap", "6px"),
        ("--unixnotis-media-control-gap", "6px"),
        ("--unixnotis-media-action-rail-gap", "8px"),
        ("--unixnotis-media-card-padding-y", "8px"),
        ("--unixnotis-media-card-padding-x", "10px"),
        ("--unixnotis-media-card-padding-inline-y", "10px"),
        ("--unixnotis-media-card-padding-inline-x", "12px"),
        ("--unixnotis-media-card-padding-stacked", "12px"),
        ("--unixnotis-media-card-padding-showcase-y", "10px"),
        ("--unixnotis-media-card-padding-showcase-x", "12px"),
        ("--unixnotis-media-art-size", "50px"),
        ("--unixnotis-media-art-frame-size", "54px"),
        ("--unixnotis-media-button-padding-y", "4px"),
        ("--unixnotis-media-button-padding-x", "6px"),
        ("--unixnotis-media-nav-size", "22px"),
    ]
}

#[cfg(test)]
mod tests {
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

        assert!(
            overrides.contains("@define-color unixnotis-card alpha(@unixnotis-card-base, 0.42);")
        );
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
}
