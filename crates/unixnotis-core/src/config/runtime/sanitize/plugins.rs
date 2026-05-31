use tracing::warn;

use super::{
    super::super::{Config, SliderWidgetConfig, WidgetPluginConfig},
    MAX_CARD_HEIGHT,
};
use crate::util;

pub(super) const MIN_PLUGIN_TIMEOUT_MS: u64 = 100;
pub(super) const MAX_PLUGIN_TIMEOUT_MS: u64 = 30_000;
pub(super) const MIN_PLUGIN_OUTPUT_BYTES: usize = 128;
pub(super) const MAX_PLUGIN_OUTPUT_BYTES: usize = 128 * 1024;
const MAX_SLIDER_SEGMENTS: usize = 64;
const MAX_SLIDER_SUBLABEL_CHARS: usize = 32;
const MAX_CARD_CAROUSEL_DOTS: usize = 12;

pub(super) fn sanitize_widget_configs(config: &mut Config) {
    sanitize_slider_widget(&mut config.widgets.volume);
    sanitize_slider_widget(&mut config.widgets.brightness);
    // Stats and cards share the same geometry and plugin contract
    for stat in &mut config.widgets.stats {
        stat.min_height = stat.min_height.clamp(0, MAX_CARD_HEIGHT);
        sanitize_widget_plugin(&mut stat.plugin, "stat", &stat.label);
    }
    for card in &mut config.widgets.cards {
        card.min_height = card.min_height.clamp(0, MAX_CARD_HEIGHT);
        card.carousel_dots = card.carousel_dots.min(MAX_CARD_CAROUSEL_DOTS);
        sanitize_widget_plugin(&mut card.plugin, "card", &card.title);
    }
}

fn sanitize_slider_widget(slider: &mut SliderWidgetConfig) {
    // Segment widgets are decorative, so cap them tightly to avoid large GTK trees
    slider.segments = slider.segments.min(MAX_SLIDER_SEGMENTS);
    trim_slider_label(&mut slider.sublabel_min);
    trim_slider_label(&mut slider.sublabel_max);
}

fn trim_slider_label(label: &mut String) {
    *label = label
        .trim()
        .chars()
        .take(MAX_SLIDER_SUBLABEL_CHARS)
        .collect();
}

fn sanitize_widget_plugin(
    plugin: &mut Option<WidgetPluginConfig>,
    widget_type: &str,
    widget_label: &str,
) {
    let Some(plugin_cfg) = plugin.as_mut() else {
        return;
    };

    // Unknown plugin versions are disabled instead of being guessed at runtime
    if plugin_cfg.api_version != WidgetPluginConfig::API_VERSION_V1 {
        warn!(
            widget_type,
            widget_label,
            version = plugin_cfg.api_version,
            "unsupported widget plugin api_version; disabling plugin"
        );
        *plugin = None;
        return;
    }

    let command = plugin_cfg.command.trim();
    if command.is_empty() {
        // Empty commands only look configured but can never run
        warn!(
            widget_type,
            widget_label, "empty widget plugin command; disabling plugin"
        );
        *plugin = None;
        return;
    }
    if !util::is_simple_command(command) {
        // Shell syntax is not allowed in the plugin command field
        warn!(
            widget_type,
            widget_label, "widget plugin command must be a simple command; disabling plugin"
        );
        *plugin = None;
        return;
    }
    plugin_cfg.command = command.to_string();

    if plugin_cfg.timeout_ms == 0 {
        // Zero timeout falls back to the canonical plugin default
        plugin_cfg.timeout_ms = WidgetPluginConfig::default().timeout_ms;
    }
    plugin_cfg.timeout_ms = plugin_cfg
        .timeout_ms
        .clamp(MIN_PLUGIN_TIMEOUT_MS, MAX_PLUGIN_TIMEOUT_MS);

    if plugin_cfg.max_output_bytes == 0 {
        // Zero output budget falls back to the canonical plugin default
        plugin_cfg.max_output_bytes = WidgetPluginConfig::default().max_output_bytes;
    }
    plugin_cfg.max_output_bytes = plugin_cfg
        .max_output_bytes
        .clamp(MIN_PLUGIN_OUTPUT_BYTES, MAX_PLUGIN_OUTPUT_BYTES);
}

#[cfg(test)]
#[path = "../../tests/runtime/plugins.rs"]
mod tests;
