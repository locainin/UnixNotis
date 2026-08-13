//! Runtime adjustments for slider widget backends

use super::super::{NumericParseMode, SliderWidgetConfig};
use crate::{program_in_path, CommandSpec};
use tracing::warn;

pub(in super::super) fn apply_volume_backend(volume: &mut SliderWidgetConfig) {
    if !volume.enabled {
        return;
    }
    let is_wpctl_default = volume.get_cmd == SliderWidgetConfig::wpctl_get()
        && volume.set_cmd == SliderWidgetConfig::wpctl_set()
        && volume
            .toggle_cmd
            .as_ref()
            .is_some_and(|cmd| *cmd == SliderWidgetConfig::wpctl_toggle());
    let watch_is_legacy = volume
        .watch_cmd
        .as_ref()
        .is_some_and(|command| *command == CommandSpec::direct("wpctl", ["subscribe"]));
    let pactl_available = program_in_path("pactl");
    let wpctl_available = program_in_path("wpctl");

    // Only stock volume commands are migrated; custom slider commands remain config-owned
    let watch_needs_stock_backfill = is_wpctl_default && volume.watch_cmd.is_none();
    if watch_needs_stock_backfill || watch_is_legacy {
        if pactl_available {
            // Prefer the documented long-running `pactl subscribe` watcher when available
            volume.watch_cmd = Some(SliderWidgetConfig::pactl_watch());
        } else if watch_is_legacy {
            // Avoid spawning the legacy wpctl watcher that is not part of `wpctl` CLI
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
        // pactl is the compatible fallback when wpctl is not installed
        volume.get_cmd = SliderWidgetConfig::pactl_get();
        volume.set_cmd = SliderWidgetConfig::pactl_set();
        volume.toggle_cmd = Some(SliderWidgetConfig::pactl_toggle());
        // Fall back to auto parsing because pactl output differs from wpctl ratios
        volume.parse_mode = NumericParseMode::Auto;
        if volume.watch_cmd.is_none() {
            volume.watch_cmd = Some(SliderWidgetConfig::pactl_watch());
        }
    } else {
        // Disable the widget explicitly when no supported backend is present
        warn!("volume widget disabled: missing wpctl and pactl backends");
        volume.enabled = false;
    }
}

pub(in super::super) fn apply_brightness_backend(brightness: &mut SliderWidgetConfig) {
    if !brightness.enabled {
        return;
    }
    if brightness
        .watch_cmd
        .as_ref()
        .is_some_and(|command| *command == CommandSpec::direct("brightnessctl", ["-w"]))
    {
        // Remove the legacy watch flag because brightnessctl has no watch mode
        brightness.watch_cmd = None;
    }
}

#[cfg(test)]
#[path = "tests/widgets.rs"]
mod tests;
