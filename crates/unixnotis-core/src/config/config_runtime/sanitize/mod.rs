//! Runtime sanitization and validation for configuration values

use super::super::{
    Config, PanelConfig, PopupConfig, ThemeConfig, DEFAULT_MEDIA_ART_SIZE_PX,
    DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX, PANEL_HEIGHT_PERCENT_DEFAULT,
};

mod media;
mod plugins;
mod shell;

const MIN_REFRESH_MS: u64 = 100;
const MAX_REFRESH_MS: u64 = 60_000;
const MAX_REFRESH_SLOW_MS: u64 = 120_000;
const MAX_PANEL_WIDTH: i32 = 4096;
const MAX_PANEL_HEIGHT_PERCENT: i32 = 100;
const MAX_PANEL_HEIGHT: i32 = 4096;
const MAX_POPUP_WIDTH: i32 = 2048;
const MAX_SPACING: i32 = 256;
const MAX_MARGIN: i32 = 512;
const MAX_CARD_HEIGHT: i32 = 2048;
const MAX_MEDIA_ART_SIZE: i32 = 512;
const MIN_MEDIA_TITLE_CHAR_LIMIT: usize = 1;
const MAX_MEDIA_TITLE_CHAR_LIMIT: usize = 256;
const MIN_MEDIA_TEXT_WIDTH_FLOOR: i32 = 48;
const MAX_MEDIA_TEXT_WIDTH_FLOOR: i32 = 2048;
const MAX_HISTORY_ENTRIES: usize = 5_000;
// Match the daemon-side active cap so config, docs, and runtime all agree
const MAX_HISTORY_ACTIVE: usize = 12;
// Theme guard rails keep layout values within reasonable bounds
const MAX_BORDER_WIDTH: u8 = 16;
const MAX_CARD_RADIUS: u8 = 64;

pub(in super::super) fn sanitize_config(config: &mut Config) {
    // Clamp refresh intervals before any runtime worker reads them
    let fast = clamp_refresh_interval(
        config.widgets.refresh_interval_ms,
        MIN_REFRESH_MS,
        MAX_REFRESH_MS,
    );
    let mut slow = clamp_refresh_interval(
        config.widgets.refresh_interval_slow_ms,
        MIN_REFRESH_MS,
        MAX_REFRESH_SLOW_MS,
    );
    // Slow polling should never outrun the faster lane when both are enabled
    if fast > 0 && slow > 0 && slow < fast {
        slow = fast;
    }
    config.widgets.refresh_interval_ms = fast;
    config.widgets.refresh_interval_slow_ms = slow;

    // Restore sane panel width and height hints before the UI reads them
    if config.panel.width <= 0 {
        config.panel.width = PanelConfig::default().width;
    }
    config.panel.width = config.panel.width.clamp(1, MAX_PANEL_WIDTH);
    if config.panel.height <= 0 {
        config.panel.height = PANEL_HEIGHT_PERCENT_DEFAULT;
    }
    config.panel.height = config.panel.height.clamp(1, MAX_PANEL_HEIGHT_PERCENT);
    if let Some(height_override) = config.panel.height_override {
        if height_override <= 0 {
            config.panel.height_override = None;
        } else {
            config.panel.height_override = Some(height_override.clamp(1, MAX_PANEL_HEIGHT));
        }
    }

    // Popup width and spacing feed the live geometry path, so clamp them early
    if config.popups.width <= 0 {
        config.popups.width = PopupConfig::default().width;
    }
    config.popups.width = config.popups.width.clamp(1, MAX_POPUP_WIDTH);
    config.popups.spacing = config.popups.spacing.clamp(0, MAX_SPACING);

    // Negative margins flip widget placement, so fold them back into bounds
    config.popups.margin.top = config.popups.margin.top.clamp(0, MAX_MARGIN);
    config.popups.margin.right = config.popups.margin.right.clamp(0, MAX_MARGIN);
    config.popups.margin.bottom = config.popups.margin.bottom.clamp(0, MAX_MARGIN);
    config.popups.margin.left = config.popups.margin.left.clamp(0, MAX_MARGIN);
    config.panel.margin.top = config.panel.margin.top.clamp(0, MAX_MARGIN);
    config.panel.margin.right = config.panel.margin.right.clamp(0, MAX_MARGIN);
    config.panel.margin.bottom = config.panel.margin.bottom.clamp(0, MAX_MARGIN);
    config.panel.margin.left = config.panel.margin.left.clamp(0, MAX_MARGIN);
    config.panel.empty_offset_top = config.panel.empty_offset_top.clamp(0, MAX_MARGIN);

    // Media cleanup is large enough to keep in its own helper
    media::sanitize_media_config(config);

    // History bounds need one final invariant, not just two separate caps
    config.history.max_active = config.history.max_active.min(MAX_HISTORY_ACTIVE);
    config.history.max_entries = config.history.max_entries.min(MAX_HISTORY_ENTRIES);
    config.history.max_active = config.history.max_active.min(config.history.max_entries);

    // Widget cards and stats share the same height and plugin guard rails
    plugins::sanitize_widget_configs(config);

    sanitize_theme_config(config);
    shell::warn_missing_shell(config);
}

fn sanitize_theme_config(config: &mut Config) {
    let theme = &mut config.theme;
    let needs_theme_defaults = !theme.surface_alpha.is_finite()
        || !theme.surface_strong_alpha.is_finite()
        || !theme.card_alpha.is_finite()
        || !theme.shadow_soft_alpha.is_finite()
        || !theme.shadow_strong_alpha.is_finite();

    // Only allocate theme defaults when a fallback for bad alpha values is needed
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

    // Border styling feeds generated CSS, so keep it inside sane guard rails
    config.theme.border_width = config.theme.border_width.min(MAX_BORDER_WIDTH);
    config.theme.card_radius = config.theme.card_radius.min(MAX_CARD_RADIUS);
}

fn clamp_alpha(value: &mut f32, fallback: f32) {
    // Bad alpha falls back to the shipped default
    if !value.is_finite() {
        *value = fallback;
        return;
    }
    *value = value.clamp(0.0, 1.0);
}

fn clamp_alpha_finite(value: &mut f32) {
    // Finite alpha values still need range checks
    *value = value.clamp(0.0, 1.0);
}

fn clamp_refresh_interval(value: u64, min: u64, max: u64) -> u64 {
    // Zero keeps the interval disabled instead of forcing polling back on
    if value == 0 {
        return 0;
    }
    value.clamp(min, max)
}

#[cfg(test)]
mod tests;
