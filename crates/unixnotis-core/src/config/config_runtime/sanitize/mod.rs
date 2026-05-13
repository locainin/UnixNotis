//! Runtime sanitization and validation for configuration values

use super::super::{
    default_panel_widget_order, Config, PanelConfig, PanelWidgetSection, PopupConfig, ThemeConfig,
    DEFAULT_MEDIA_ART_SIZE_PX, DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX, PANEL_HEIGHT_PERCENT_DEFAULT,
};

mod media;
mod panel;
mod plugins;
mod refresh;
mod shell;
mod theme;

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
const MIN_WIDGET_COLUMNS: usize = 1;
const MAX_WIDGET_COLUMNS: usize = 8;

pub(in super::super) fn sanitize_config(config: &mut Config) {
    // Clamp refresh intervals before any runtime worker reads them
    let fast = refresh::clamp_refresh_interval(
        config.widgets.refresh_interval_ms,
        MIN_REFRESH_MS,
        MAX_REFRESH_MS,
    );
    let mut slow = refresh::clamp_refresh_interval(
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
    panel::sanitize_panel_text(&mut config.panel);
    panel::sanitize_panel_widget_order(&mut config.panel.widget_order);
    panel::sanitize_widget_columns(config);

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

    theme::sanitize_theme_config(config);
    shell::warn_missing_shell(config);
}

#[cfg(test)]
mod tests;
