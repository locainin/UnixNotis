//! Runtime adjustments for configuration defaults.
//!
//! Selects backend commands based on runtime availability.

use super::{Config, SliderWidgetConfig};
use crate::program_in_path;

const LEGACY_WPCTL_WATCH: &str = "wpctl subscribe";
const MIN_REFRESH_MS: u64 = 100;
const MAX_REFRESH_MS: u64 = 60_000;
const MAX_REFRESH_SLOW_MS: u64 = 120_000;
const MAX_PANEL_WIDTH: i32 = 4096;
const MAX_PANEL_HEIGHT: i32 = 4096;
const MAX_POPUP_WIDTH: i32 = 2048;
const MAX_SPACING: i32 = 256;
const MAX_MARGIN: i32 = 512;
const MAX_CARD_HEIGHT: i32 = 2048;

pub(super) fn apply_volume_backend(volume: &mut SliderWidgetConfig) {
    if !volume.enabled {
        return;
    }
    let is_wpctl_default = volume.get_cmd == SliderWidgetConfig::WPCTL_GET
        && volume.set_cmd == SliderWidgetConfig::WPCTL_SET
        && volume
            .toggle_cmd
            .as_deref()
            .map(|cmd| cmd == SliderWidgetConfig::WPCTL_TOGGLE)
            .unwrap_or(false);
    let watch_is_legacy = volume.watch_cmd.as_deref() == Some(LEGACY_WPCTL_WATCH);
    let pactl_available = program_in_path("pactl");
    let wpctl_available = program_in_path("wpctl");

    if volume.watch_cmd.is_none() || watch_is_legacy {
        if pactl_available {
            // Prefer the documented long-running `pactl subscribe` watcher when available.
            volume.watch_cmd = Some(SliderWidgetConfig::PACTL_WATCH.to_string());
        } else if watch_is_legacy {
            // Avoid spawning the legacy wpctl watcher that is not part of `wpctl` CLI.
            volume.watch_cmd = None;
        }
    }

    if !is_wpctl_default {
        return;
    }
    if wpctl_available {
        return;
    }
    if pactl_available {
        volume.get_cmd = SliderWidgetConfig::PACTL_GET.to_string();
        volume.set_cmd = SliderWidgetConfig::PACTL_SET.to_string();
        volume.toggle_cmd = Some(SliderWidgetConfig::PACTL_TOGGLE.to_string());
        if volume.watch_cmd.is_none() {
            volume.watch_cmd = Some(SliderWidgetConfig::PACTL_WATCH.to_string());
        }
    } else {
        volume.enabled = false;
    }
}

pub(super) fn apply_brightness_backend(brightness: &mut SliderWidgetConfig) {
    if !brightness.enabled {
        return;
    }
    if brightness.watch_cmd.as_deref() == Some("brightnessctl -w") {
        // Remove the legacy watch flag because brightnessctl has no watch mode.
        brightness.watch_cmd = None;
    }
}

pub(super) fn sanitize_config(config: &mut Config) {
    // Clamp refresh intervals to avoid busy loops or runaway timers.
    let fast = config
        .widgets
        .refresh_interval_ms
        .clamp(MIN_REFRESH_MS, MAX_REFRESH_MS);
    let slow = config
        .widgets
        .refresh_interval_slow_ms
        .clamp(fast, MAX_REFRESH_SLOW_MS);
    config.widgets.refresh_interval_ms = fast;
    config.widgets.refresh_interval_slow_ms = slow;

    // Normalize panel sizing; keep height 0 as "auto".
    if config.panel.width <= 0 {
        config.panel.width = super::PanelConfig::default().width;
    }
    config.panel.width = config.panel.width.clamp(1, MAX_PANEL_WIDTH);
    if config.panel.height < 0 {
        config.panel.height = 0;
    }
    if config.panel.height > 0 {
        config.panel.height = config.panel.height.clamp(1, MAX_PANEL_HEIGHT);
    }

    // Normalize popup sizing and spacing.
    if config.popups.width <= 0 {
        config.popups.width = super::PopupConfig::default().width;
    }
    config.popups.width = config.popups.width.clamp(1, MAX_POPUP_WIDTH);
    if config.popups.spacing < 0 {
        config.popups.spacing = 0;
    }
    config.popups.spacing = config.popups.spacing.clamp(0, MAX_SPACING);

    // Clamp margins to non-negative values to avoid inverted geometry.
    config.popups.margin.top = config.popups.margin.top.clamp(0, MAX_MARGIN);
    config.popups.margin.right = config.popups.margin.right.clamp(0, MAX_MARGIN);
    config.popups.margin.bottom = config.popups.margin.bottom.clamp(0, MAX_MARGIN);
    config.popups.margin.left = config.popups.margin.left.clamp(0, MAX_MARGIN);
    config.panel.margin.top = config.panel.margin.top.clamp(0, MAX_MARGIN);
    config.panel.margin.right = config.panel.margin.right.clamp(0, MAX_MARGIN);
    config.panel.margin.bottom = config.panel.margin.bottom.clamp(0, MAX_MARGIN);
    config.panel.margin.left = config.panel.margin.left.clamp(0, MAX_MARGIN);

    for stat in &mut config.widgets.stats {
        if stat.min_height < 0 {
            stat.min_height = 0;
        }
        stat.min_height = stat.min_height.clamp(0, MAX_CARD_HEIGHT);
    }
    for card in &mut config.widgets.cards {
        if card.min_height < 0 {
            card.min_height = 0;
        }
        card.min_height = card.min_height.clamp(0, MAX_CARD_HEIGHT);
    }
}
