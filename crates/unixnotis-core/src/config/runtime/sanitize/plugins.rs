use tracing::warn;

use super::{
    super::super::{Config, SliderWidgetConfig, WidgetPluginConfig},
    MAX_CARD_HEIGHT,
};

pub(super) const MIN_PLUGIN_TIMEOUT_MS: u64 = 100;
pub(super) const MAX_PLUGIN_TIMEOUT_MS: u64 = 30_000;
pub(super) const MIN_PLUGIN_OUTPUT_BYTES: usize = 128;
pub(super) const MAX_PLUGIN_OUTPUT_BYTES: usize = 128 * 1024;
const MAX_SLIDER_SEGMENTS: usize = 64;
const MAX_SLIDER_SUBLABEL_CHARS: usize = 32;
const SAFE_SLIDER_MIN: f64 = 0.0;
const SAFE_SLIDER_MAX: f64 = 100.0;
const SAFE_SLIDER_STEP: f64 = 1.0;
const MAX_CARD_CAROUSEL_DOTS: usize = 12;
// These limits keep one compact config from allocating an excessive GTK tree
// The total cap also prevents a preset from exhausting every per-type allowance at once
pub const MAX_TOGGLE_WIDGETS: usize = 16;
pub const MAX_STAT_WIDGETS: usize = 24;
pub const MAX_CARD_WIDGETS: usize = 16;
pub const MAX_TOTAL_WIDGETS: usize = 48;

pub(super) fn sanitize_widget_configs(config: &mut Config) {
    limit_widget_counts(config);
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

fn limit_widget_counts(config: &mut Config) {
    truncate_widgets(&mut config.widgets.toggles, MAX_TOGGLE_WIDGETS, "toggle");
    truncate_widgets(&mut config.widgets.stats, MAX_STAT_WIDGETS, "stat");
    truncate_widgets(&mut config.widgets.cards, MAX_CARD_WIDGETS, "card");

    // Keep earlier groups first because toggles and compact stats are the primary controls
    let used = config.widgets.toggles.len() + config.widgets.stats.len();
    let card_budget = MAX_TOTAL_WIDGETS.saturating_sub(used);
    truncate_widgets(&mut config.widgets.cards, card_budget, "card");
}

fn truncate_widgets<T>(widgets: &mut Vec<T>, limit: usize, widget_type: &str) {
    let requested = widgets.len();
    if requested <= limit {
        return;
    }
    widgets.truncate(limit);
    warn!(
        widget_type,
        requested,
        kept = limit,
        ignored = requested - limit,
        "widget count exceeded the safe runtime limit"
    );
}

fn sanitize_slider_widget(slider: &mut SliderWidgetConfig) {
    // Segment widgets are decorative, so cap them tightly to avoid large GTK trees
    slider.segments = slider.segments.min(MAX_SLIDER_SEGMENTS);

    // GTK ranges and f64 clamping both require finite ordered bounds
    if !slider.min.is_finite() || !slider.max.is_finite() || slider.min >= slider.max {
        slider.min = SAFE_SLIDER_MIN;
        slider.max = SAFE_SLIDER_MAX;
    }

    let range = slider.max - slider.min;
    if !slider.step.is_finite() || slider.step <= 0.0 || slider.step > range {
        // Narrow custom ranges receive the largest valid fallback no greater than one
        slider.step = SAFE_SLIDER_STEP.min(range);
    }

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

    if plugin_cfg.command.is_empty() {
        // Empty commands only look configured but can never run
        warn!(
            widget_type,
            widget_label, "empty widget plugin command; disabling plugin"
        );
        *plugin = None;
        return;
    }
    if plugin_cfg.command.uses_shell_command_string() {
        // Shell syntax is not allowed in the plugin command field
        warn!(
            widget_type,
            widget_label, "widget plugin command must be a simple command; disabling plugin"
        );
        *plugin = None;
        return;
    }
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
#[path = "tests/plugins.rs"]
mod tests;
