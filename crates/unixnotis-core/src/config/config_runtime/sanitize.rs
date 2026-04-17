//! Runtime sanitization and validation for configuration values.

use std::collections::{BTreeMap, HashSet};

use super::super::{
    Config, PanelConfig, PopupConfig, ThemeConfig, WidgetPluginConfig, DEFAULT_MEDIA_ART_SIZE_PX,
    DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX, PANEL_HEIGHT_PERCENT_DEFAULT,
};
use crate::{program_in_path, util};
use tracing::warn;

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
// Theme guard rails keep layout values within reasonable bounds.
const MAX_BORDER_WIDTH: u8 = 16;
const MAX_CARD_RADIUS: u8 = 64;
const MIN_PLUGIN_TIMEOUT_MS: u64 = 100;
const MAX_PLUGIN_TIMEOUT_MS: u64 = 30_000;
const MIN_PLUGIN_OUTPUT_BYTES: usize = 128;
const MAX_PLUGIN_OUTPUT_BYTES: usize = 128 * 1024;

pub(in super::super) fn sanitize_config(config: &mut Config) {
    // Clean config before use
    // Clamp refresh intervals to avoid busy loops or runaway timers.
    // A value of 0 disables polling and must be preserved for UI correctness.
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
    // Only enforce slow >= fast when both intervals are enabled.
    if fast > 0 && slow > 0 && slow < fast {
        slow = fast;
    }
    config.widgets.refresh_interval_ms = fast;
    config.widgets.refresh_interval_slow_ms = slow;

    // Normalize panel sizing
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

    // Normalize popup sizing and spacing.
    if config.popups.width <= 0 {
        config.popups.width = PopupConfig::default().width;
    }
    config.popups.width = config.popups.width.clamp(1, MAX_POPUP_WIDTH);
    // Clamp spacing directly; negative values fold to the lower bound.
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
    config.panel.empty_offset_top = config.panel.empty_offset_top.clamp(0, MAX_MARGIN);

    // Normalize media identifiers once so later runtime checks see one stable shape
    config.media.allowlist = normalize_media_tokens(&config.media.allowlist);
    config.media.denylist = normalize_media_tokens(&config.media.denylist);
    config.media.browser_tokens = normalize_media_tokens(&config.media.browser_tokens);
    config.media.source_aliases = normalize_media_aliases(&config.media.source_aliases);
    config.media.title_char_limit = config
        .media
        .title_char_limit
        .clamp(MIN_MEDIA_TITLE_CHAR_LIMIT, MAX_MEDIA_TITLE_CHAR_LIMIT);
    if config.media.art_size_px <= 0 {
        // Pull the scalar default directly instead of building a full MediaConfig
        config.media.art_size_px = DEFAULT_MEDIA_ART_SIZE_PX;
    }
    config.media.art_size_px = config.media.art_size_px.clamp(1, MAX_MEDIA_ART_SIZE);
    if config.media.text_width_floor_px <= 0 {
        // Same rule here so sanitize stays cheap and explicit
        config.media.text_width_floor_px = DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX;
    }
    config.media.text_width_floor_px = config
        .media
        .text_width_floor_px
        .clamp(MIN_MEDIA_TEXT_WIDTH_FLOOR, MAX_MEDIA_TEXT_WIDTH_FLOOR);
    config.media.content_spacing_px = config.media.content_spacing_px.clamp(0, MAX_SPACING);
    config.media.control_spacing_px = config.media.control_spacing_px.clamp(0, MAX_SPACING);
    config.media.navigation_spacing_px = config.media.navigation_spacing_px.clamp(0, MAX_SPACING);
    if let Some(card_height_px) = config.media.card_height_px {
        if card_height_px <= 0 {
            config.media.card_height_px = None;
        } else {
            config.media.card_height_px = Some(card_height_px.clamp(1, MAX_CARD_HEIGHT));
        }
    }

    // Match the daemon cap
    config.history.max_active = config.history.max_active.min(MAX_HISTORY_ACTIVE);
    // Keep history bounded too
    config.history.max_entries = config.history.max_entries.min(MAX_HISTORY_ENTRIES);
    // Active notifications live inside the same history pool
    config.history.max_active = config.history.max_active.min(config.history.max_entries);

    // Clamp min-height values directly; clamp covers negative inputs.
    for stat in &mut config.widgets.stats {
        // Keep widget size in range
        stat.min_height = stat.min_height.clamp(0, MAX_CARD_HEIGHT);
        // Check plugin config here
        sanitize_widget_plugin(&mut stat.plugin, "stat", &stat.label);
    }
    for card in &mut config.widgets.cards {
        // Same size rules for cards
        card.min_height = card.min_height.clamp(0, MAX_CARD_HEIGHT);
        sanitize_widget_plugin(&mut card.plugin, "card", &card.title);
    }

    let theme = &mut config.theme;
    let needs_theme_defaults = !theme.surface_alpha.is_finite()
        || !theme.surface_strong_alpha.is_finite()
        || !theme.card_alpha.is_finite()
        || !theme.shadow_soft_alpha.is_finite()
        || !theme.shadow_strong_alpha.is_finite();
    // Only allocate theme defaults when a fallback for non-finite values is required.
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
    // Clamp border styling to keep generated CSS within sensible bounds.
    config.theme.border_width = config.theme.border_width.min(MAX_BORDER_WIDTH);
    config.theme.card_radius = config.theme.card_radius.min(MAX_CARD_RADIUS);
    warn_missing_shell(config);
}

fn normalize_media_tokens(tokens: &[String]) -> Vec<String> {
    let mut normalized = Vec::with_capacity(tokens.len());
    let mut seen = HashSet::with_capacity(tokens.len());

    for token in tokens {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            // Empty tokens cannot match anything useful later
            warn!(raw_token = token, "discarding empty media token");
            continue;
        }

        let normalized_token = trimmed.to_lowercase();
        if !seen.insert(normalized_token.clone()) {
            // Duplicate tokens only repeat the same later comparison
            warn!(token = normalized_token, "discarding duplicate media token");
            continue;
        }

        normalized.push(normalized_token);
    }

    normalized
}

fn normalize_media_aliases(aliases: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut normalized = BTreeMap::new();
    let mut original_keys: BTreeMap<String, String> = BTreeMap::new();

    for (token, label) in aliases {
        let trimmed_token = token.trim();
        let trimmed_label = label.trim();

        if trimmed_token.is_empty() {
            // Empty alias keys never match a player name or identity
            warn!(
                raw_key = token,
                raw_label = label,
                "discarding media alias with empty key"
            );
            continue;
        }
        if trimmed_label.is_empty() {
            // Empty labels render like missing metadata and only confuse the result
            warn!(
                raw_key = token,
                raw_label = label,
                "discarding media alias with empty label"
            );
            continue;
        }

        let normalized_token = trimmed_token.to_lowercase();
        if let Some(existing_key) = original_keys.get(&normalized_token) {
            // Lowercasing can collapse two user keys into the same runtime key
            warn!(
                normalized_key = normalized_token,
                kept_key = existing_key.as_str(),
                dropped_key = token.as_str(),
                "discarding colliding media alias after normalization"
            );
            continue;
        }

        original_keys.insert(normalized_token.clone(), token.clone());
        normalized.insert(normalized_token, trimmed_label.to_string());
    }

    normalized
}

fn clamp_alpha(value: &mut f32, fallback: f32) {
    // Bad alpha falls back
    if !value.is_finite() {
        *value = fallback;
        return;
    }
    *value = value.clamp(0.0, 1.0);
}

fn clamp_alpha_finite(value: &mut f32) {
    // Clamp to the valid range
    *value = value.clamp(0.0, 1.0);
}

fn clamp_refresh_interval(value: u64, min: u64, max: u64) -> u64 {
    // Zero means off
    if value == 0 {
        return 0;
    }
    value.clamp(min, max)
}

fn sanitize_widget_plugin(
    plugin: &mut Option<WidgetPluginConfig>,
    widget_type: &str,
    widget_label: &str,
) {
    let Some(plugin_cfg) = plugin.as_mut() else {
        return;
    };
    // Unknown plugin version means off
    if plugin_cfg.api_version != WidgetPluginConfig::API_VERSION_V1 {
        // Unknown contract versions are disabled rather than best-effort parsed.
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
    // Empty command means off
    if command.is_empty() {
        warn!(
            widget_type,
            widget_label, "empty widget plugin command; disabling plugin"
        );
        *plugin = None;
        return;
    }
    if !util::is_simple_command(command) {
        // Shell syntax is not allowed here
        warn!(
            widget_type,
            widget_label, "widget plugin command must be a simple command; disabling plugin"
        );
        *plugin = None;
        return;
    }
    plugin_cfg.command = command.to_string();

    if plugin_cfg.timeout_ms == 0 {
        // Zero timeout falls back to the canonical plugin default.
        plugin_cfg.timeout_ms = WidgetPluginConfig::default().timeout_ms;
    }
    // Clamp timeout to bounded runtime to protect the widget worker queue.
    plugin_cfg.timeout_ms = plugin_cfg
        .timeout_ms
        .clamp(MIN_PLUGIN_TIMEOUT_MS, MAX_PLUGIN_TIMEOUT_MS);

    if plugin_cfg.max_output_bytes == 0 {
        // Zero output budget falls back to the canonical plugin default.
        plugin_cfg.max_output_bytes = WidgetPluginConfig::default().max_output_bytes;
    }
    // Clamp output budget to prevent unbounded buffering in command capture paths.
    plugin_cfg.max_output_bytes = plugin_cfg
        .max_output_bytes
        .clamp(MIN_PLUGIN_OUTPUT_BYTES, MAX_PLUGIN_OUTPUT_BYTES);
}

fn warn_missing_shell(config: &Config) {
    // Only warn when needed
    if program_in_path("sh") {
        return;
    }
    if !config_requires_shell(config) {
        return;
    }
    // Shell-backed commands depend on sh being present for pipes, redirects, and control flow.
    warn!("POSIX shell (sh) not found in PATH; widget commands using pipes or redirects will fail");
}

fn config_requires_shell(config: &Config) -> bool {
    // Walk all configured commands to detect whether shell metacharacters are present.
    let volume = &config.widgets.volume;
    if command_requires_shell(&volume.get_cmd)
        || command_requires_shell(&volume.set_cmd)
        || command_requires_shell_opt(&volume.toggle_cmd)
        || command_requires_shell_opt(&volume.watch_cmd)
    {
        return true;
    }

    let brightness = &config.widgets.brightness;
    if command_requires_shell(&brightness.get_cmd)
        || command_requires_shell(&brightness.set_cmd)
        || command_requires_shell_opt(&brightness.toggle_cmd)
        || command_requires_shell_opt(&brightness.watch_cmd)
    {
        return true;
    }

    if config.widgets.toggles.iter().any(|toggle| {
        command_requires_shell_opt(&toggle.state_cmd)
            || command_requires_shell_opt(&toggle.on_cmd)
            || command_requires_shell_opt(&toggle.off_cmd)
            || command_requires_shell_opt(&toggle.watch_cmd)
    }) {
        return true;
    }

    if config.widgets.stats.iter().any(|stat| {
        command_requires_shell_opt(&stat.cmd)
            || stat
                .plugin
                .as_ref()
                .is_some_and(|plugin| command_requires_shell(&plugin.command))
    }) {
        return true;
    }

    config.widgets.cards.iter().any(|card| {
        command_requires_shell_opt(&card.cmd)
            || card
                .plugin
                .as_ref()
                .is_some_and(|plugin| command_requires_shell(&plugin.command))
    })
}

fn command_requires_shell_opt(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(command_requires_shell)
        .unwrap_or(false)
}

fn command_requires_shell(cmd: &str) -> bool {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return false;
    }
    // Strip known runtime placeholders so braces do not trigger false positives.
    let cmd = cmd.replace("{value}", "0");
    !util::is_simple_command(&cmd)
}

#[cfg(test)]
#[path = "tests/sanitize.rs"]
mod tests;
