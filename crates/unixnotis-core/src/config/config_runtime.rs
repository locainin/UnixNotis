//! Runtime adjustments for configuration defaults.
//!
//! Selects backend commands based on runtime availability.

use super::SliderWidgetConfig;
use crate::program_in_path;

const LEGACY_WPCTL_WATCH: &str = "wpctl subscribe";

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
