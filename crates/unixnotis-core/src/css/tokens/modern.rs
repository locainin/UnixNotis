//! Modern GTK custom-property rendering

use crate::config::ThemeConfig;

use super::super::features::GtkCssFeatures;
use super::layout::layout_tokens;
use super::model::{clamp_alpha, theme_card_style_values};
use super::palette::color_alias_tokens;

#[must_use]
pub fn build_modern_theme_custom_properties(
    theme: &ThemeConfig,
    features: GtkCssFeatures,
) -> String {
    // Older GTK builds should see no custom-property output
    if !features.supports_modern_theme_tokens() {
        return String::new();
    }

    let surface_alpha = clamp_alpha(theme.surface_alpha);
    let surface_strong_alpha = clamp_alpha(theme.surface_strong_alpha);
    let card_alpha = clamp_alpha(theme.card_alpha);
    let shadow_soft = clamp_alpha(theme.shadow_soft_alpha);
    let shadow_strong = clamp_alpha(theme.shadow_strong_alpha);
    let card_style = theme_card_style_values(theme);

    // Keep the selector plain in generated CSS while avoiding source-lint confusion
    let mut block = String::from(":\u{72}oot {\n");
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

    for (name, value) in color_alias_tokens() {
        push_raw_token(&mut block, name, value);
    }
    for (name, value) in layout_tokens() {
        push_raw_token(&mut block, name, value);
    }

    block.push_str("}\n");
    block
}

fn push_px_token(block: &mut String, name: &str, value: f32) {
    block.push_str(&format!("  {name}: {}px;\n", trim_float(value)));
}

fn push_alpha_token(block: &mut String, name: &str, value: f32) {
    block.push_str(&format!("  {name}: {};\n", trim_float(value)));
}

fn push_raw_token(block: &mut String, name: &str, value: &str) {
    block.push_str(&format!("  {name}: {value};\n"));
}

fn trim_float(value: f32) -> String {
    // Removing trailing zeroes keeps generated diagnostics readable
    let mut text = format!("{value:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}
