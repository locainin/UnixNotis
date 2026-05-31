use super::super::super::{Config, PanelConfig, PopupConfig, PANEL_HEIGHT_PERCENT_DEFAULT};
use super::{media, panel, plugins, refresh, shell, theme};

pub(in super::super) const MIN_REFRESH_MS: u64 = 100;
pub(in super::super) const MAX_REFRESH_MS: u64 = 60_000;
pub(in super::super) const MAX_REFRESH_SLOW_MS: u64 = 120_000;
pub(in super::super) const MAX_PANEL_WIDTH: i32 = 4096;
pub(in super::super) const MAX_PANEL_HEIGHT_PERCENT: i32 = 100;
pub(in super::super) const MAX_PANEL_HEIGHT: i32 = 4096;
pub(in super::super) const MAX_POPUP_WIDTH: i32 = 2048;
pub(in super::super) const MAX_SPACING: i32 = 256;
pub(in super::super) const MAX_MARGIN: i32 = 512;
pub(in super::super) const MAX_CARD_HEIGHT: i32 = 2048;
pub(in super::super) const MAX_MEDIA_ART_SIZE: i32 = 512;
pub(in super::super) const MIN_MEDIA_TITLE_CHAR_LIMIT: usize = 1;
pub(in super::super) const MAX_MEDIA_TITLE_CHAR_LIMIT: usize = 256;
pub(in super::super) const MIN_MEDIA_TEXT_WIDTH_FLOOR: i32 = 48;
pub(in super::super) const MAX_MEDIA_TEXT_WIDTH_FLOOR: i32 = 2048;
pub(in super::super) const MAX_HISTORY_ENTRIES: usize = 5_000;
pub(in super::super) const MAX_HISTORY_ACTIVE: usize = 12;
pub(in super::super) const MAX_BORDER_WIDTH: u8 = 16;
pub(in super::super) const MAX_CARD_RADIUS: u8 = 64;
pub(in super::super) const MIN_WIDGET_COLUMNS: usize = 1;
pub(in super::super) const MAX_WIDGET_COLUMNS: usize = 8;

pub(in super::super::super) fn sanitize_config(config: &mut Config) {
    sanitize_refresh_intervals(config);
    sanitize_panel_geometry(config);
    sanitize_popup_geometry(config);

    // Media, plugin, and theme rules live in their own files because each has
    // enough edge cases to test directly
    media::sanitize_media_config(config);
    sanitize_history(config);
    plugins::sanitize_widget_configs(config);
    theme::sanitize_theme_config(config);
    shell::warn_missing_shell(config);
}

fn sanitize_refresh_intervals(config: &mut Config) {
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
}

fn sanitize_panel_geometry(config: &mut Config) {
    if config.panel.width <= 0 {
        // Negative or zero width cannot map to a usable layer-shell surface
        config.panel.width = PanelConfig::default().width;
    }
    config.panel.width = config.panel.width.clamp(1, MAX_PANEL_WIDTH);

    if config.panel.height <= 0 {
        // Height is a percentage unless height_override is set
        config.panel.height = PANEL_HEIGHT_PERCENT_DEFAULT;
    }
    config.panel.height = config.panel.height.clamp(1, MAX_PANEL_HEIGHT_PERCENT);

    if let Some(height_override) = config.panel.height_override {
        if height_override <= 0 {
            // Invalid overrides are removed so percentage height can take over again
            config.panel.height_override = None;
        } else {
            config.panel.height_override = Some(height_override.clamp(1, MAX_PANEL_HEIGHT));
        }
    }

    panel::sanitize_panel_text(&mut config.panel);
    panel::sanitize_panel_section_order(&mut config.panel.section_order);
    panel::sanitize_panel_widget_order(&mut config.panel.widget_order);
    panel::sanitize_panel_action_order(&mut config.panel.action_order);
    panel::sanitize_widget_columns(config);

    config.panel.margin.top = config.panel.margin.top.clamp(0, MAX_MARGIN);
    config.panel.margin.right = config.panel.margin.right.clamp(0, MAX_MARGIN);
    config.panel.margin.bottom = config.panel.margin.bottom.clamp(0, MAX_MARGIN);
    config.panel.margin.left = config.panel.margin.left.clamp(0, MAX_MARGIN);
    config.panel.empty_offset_top = config.panel.empty_offset_top.clamp(0, MAX_MARGIN);
}

fn sanitize_popup_geometry(config: &mut Config) {
    if config.popups.width <= 0 {
        config.popups.width = PopupConfig::default().width;
    }
    config.popups.width = config.popups.width.clamp(1, MAX_POPUP_WIDTH);
    config.popups.spacing = config.popups.spacing.clamp(0, MAX_SPACING);

    config.popups.margin.top = config.popups.margin.top.clamp(0, MAX_MARGIN);
    config.popups.margin.right = config.popups.margin.right.clamp(0, MAX_MARGIN);
    config.popups.margin.bottom = config.popups.margin.bottom.clamp(0, MAX_MARGIN);
    config.popups.margin.left = config.popups.margin.left.clamp(0, MAX_MARGIN);
}

fn sanitize_history(config: &mut Config) {
    // Active notifications are bounded tighter than history to protect panel layout and memory
    config.history.max_active = config.history.max_active.min(MAX_HISTORY_ACTIVE);
    config.history.max_entries = config.history.max_entries.min(MAX_HISTORY_ENTRIES);
    config.history.max_active = config.history.max_active.min(config.history.max_entries);
}

#[cfg(test)]
#[path = "../../tests/runtime/layout.rs"]
mod tests;
