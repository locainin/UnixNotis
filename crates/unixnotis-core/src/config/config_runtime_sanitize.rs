//! Runtime sanitization and validation for configuration values.

use super::{Config, PanelConfig, PopupConfig, ThemeConfig};
use crate::{program_in_path, util};
use tracing::warn;

const MIN_REFRESH_MS: u64 = 100;
const MAX_REFRESH_MS: u64 = 60_000;
const MAX_REFRESH_SLOW_MS: u64 = 120_000;
const MAX_PANEL_WIDTH: i32 = 4096;
const MAX_PANEL_HEIGHT: i32 = 4096;
const MAX_POPUP_WIDTH: i32 = 2048;
const MAX_SPACING: i32 = 256;
const MAX_MARGIN: i32 = 512;
const MAX_CARD_HEIGHT: i32 = 2048;
// Theme guard rails keep layout values within reasonable bounds.
const MAX_BORDER_WIDTH: u8 = 16;
const MAX_CARD_RADIUS: u8 = 64;

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
        config.panel.width = PanelConfig::default().width;
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

    // Normalize media identifiers to lowercase for consistent substring matching.
    config.media.allowlist = normalize_media_tokens(&config.media.allowlist);
    config.media.denylist = normalize_media_tokens(&config.media.denylist);
    config.media.browser_tokens = normalize_media_tokens(&config.media.browser_tokens);

    // Clamp min-height values directly; clamp covers negative inputs.
    for stat in &mut config.widgets.stats {
        stat.min_height = stat.min_height.clamp(0, MAX_CARD_HEIGHT);
    }
    for card in &mut config.widgets.cards {
        card.min_height = card.min_height.clamp(0, MAX_CARD_HEIGHT);
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
    // Drop empty entries and enforce lowercase so comparisons stay case-insensitive.
    tokens
        .iter()
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn clamp_alpha(value: &mut f32, fallback: f32) {
    if !value.is_finite() {
        *value = fallback;
        return;
    }
    *value = value.clamp(0.0, 1.0);
}

fn clamp_alpha_finite(value: &mut f32) {
    *value = value.clamp(0.0, 1.0);
}

fn warn_missing_shell(config: &Config) {
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

    if config
        .widgets
        .stats
        .iter()
        .any(|stat| command_requires_shell_opt(&stat.cmd))
    {
        return true;
    }

    config
        .widgets
        .cards
        .iter()
        .any(|card| command_requires_shell_opt(&card.cmd))
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
mod tests {
    use super::super::config_widgets::{CardWidgetConfig, StatWidgetConfig};
    use super::*;

    #[test]
    fn sanitize_clamps_refresh_intervals_and_preserves_ordering() {
        // Ensure the fast interval clamps to bounds and slow interval does not undercut fast.
        let mut config = Config::default();
        config.widgets.refresh_interval_ms = 1;
        config.widgets.refresh_interval_slow_ms = 50;
        sanitize_config(&mut config);
        assert_eq!(config.widgets.refresh_interval_ms, MIN_REFRESH_MS);
        assert_eq!(config.widgets.refresh_interval_slow_ms, MIN_REFRESH_MS);

        // Ensure upper bounds are enforced for both fast and slow refresh intervals.
        let mut config = Config::default();
        config.widgets.refresh_interval_ms = MAX_REFRESH_MS + 10;
        config.widgets.refresh_interval_slow_ms = MAX_REFRESH_SLOW_MS + 10;
        sanitize_config(&mut config);
        assert_eq!(config.widgets.refresh_interval_ms, MAX_REFRESH_MS);
        assert_eq!(config.widgets.refresh_interval_slow_ms, MAX_REFRESH_SLOW_MS);
    }

    #[test]
    fn sanitize_clamps_panel_and_popup_sizes() {
        // Validate default sizing is restored when invalid or negative inputs are provided.
        let mut config = Config::default();
        config.panel.width = 0;
        config.panel.height = -8;
        config.popups.width = -10;
        config.popups.spacing = -3;
        sanitize_config(&mut config);
        assert_eq!(config.panel.width, PanelConfig::default().width);
        assert_eq!(config.panel.height, 0);
        assert_eq!(config.popups.width, PopupConfig::default().width);
        assert_eq!(config.popups.spacing, 0);

        // Validate size limits are enforced for oversized values.
        let mut config = Config::default();
        config.panel.width = MAX_PANEL_WIDTH + 25;
        config.panel.height = MAX_PANEL_HEIGHT + 40;
        config.popups.width = MAX_POPUP_WIDTH + 30;
        config.popups.spacing = MAX_SPACING + 12;
        sanitize_config(&mut config);
        assert_eq!(config.panel.width, MAX_PANEL_WIDTH);
        assert_eq!(config.panel.height, MAX_PANEL_HEIGHT);
        assert_eq!(config.popups.width, MAX_POPUP_WIDTH);
        assert_eq!(config.popups.spacing, MAX_SPACING);
    }

    #[test]
    fn sanitize_clamps_margins_and_card_heights() {
        // Ensure the config has enough widgets for min-height coverage.
        let mut config = Config::default();
        while config.widgets.stats.len() < 2 {
            config.widgets.stats.push(StatWidgetConfig::default());
        }
        while config.widgets.cards.len() < 2 {
            config.widgets.cards.push(CardWidgetConfig::default());
        }

        // Validate margins clamp to non-negative values and maximum bounds.
        config.popups.margin.top = -4;
        config.popups.margin.right = MAX_MARGIN + 3;
        config.popups.margin.bottom = -9;
        config.popups.margin.left = MAX_MARGIN + 8;
        config.panel.margin.top = MAX_MARGIN + 6;
        config.panel.margin.right = -5;
        config.panel.margin.bottom = MAX_MARGIN + 4;
        config.panel.margin.left = -7;

        // Validate stat/card min-height values clamp to the allowable range.
        config.widgets.stats[0].min_height = -1;
        config.widgets.stats[1].min_height = MAX_CARD_HEIGHT + 11;
        config.widgets.cards[0].min_height = -2;
        config.widgets.cards[1].min_height = MAX_CARD_HEIGHT + 21;
        sanitize_config(&mut config);

        assert_eq!(config.popups.margin.top, 0);
        assert_eq!(config.popups.margin.right, MAX_MARGIN);
        assert_eq!(config.popups.margin.bottom, 0);
        assert_eq!(config.popups.margin.left, MAX_MARGIN);
        assert_eq!(config.panel.margin.top, MAX_MARGIN);
        assert_eq!(config.panel.margin.right, 0);
        assert_eq!(config.panel.margin.bottom, MAX_MARGIN);
        assert_eq!(config.panel.margin.left, 0);

        assert_eq!(config.widgets.stats[0].min_height, 0);
        assert_eq!(config.widgets.stats[1].min_height, MAX_CARD_HEIGHT);
        assert_eq!(config.widgets.cards[0].min_height, 0);
        assert_eq!(config.widgets.cards[1].min_height, MAX_CARD_HEIGHT);
    }

    #[test]
    fn sanitize_normalizes_media_tokens() {
        // Normalize case and drop empty entries to keep media matching stable.
        let mut config = Config::default();
        config.media.allowlist = vec!["Spotify".to_string(), " ".to_string()];
        config.media.denylist = vec!["Playerctld".to_string()];
        config.media.browser_tokens = vec!["FireFox".to_string(), "".to_string()];
        sanitize_config(&mut config);

        assert_eq!(config.media.allowlist, vec!["spotify".to_string()]);
        assert_eq!(config.media.denylist, vec!["playerctld".to_string()]);
        assert_eq!(config.media.browser_tokens, vec!["firefox".to_string()]);
    }

    #[test]
    fn sanitize_clamps_alpha_and_theme_limits() {
        // Validate alpha values clamp to [0, 1] and fall back on non-finite inputs.
        let mut config = Config::default();
        let theme_defaults = ThemeConfig::default();
        config.theme.surface_alpha = -0.25;
        config.theme.surface_strong_alpha = 1.25;
        config.theme.card_alpha = f32::NAN;
        config.theme.shadow_soft_alpha = f32::INFINITY;
        config.theme.shadow_strong_alpha = -0.5;
        config.theme.border_width = MAX_BORDER_WIDTH + 2;
        config.theme.card_radius = MAX_CARD_RADIUS + 3;
        sanitize_config(&mut config);

        assert_eq!(config.theme.surface_alpha, 0.0);
        assert_eq!(config.theme.surface_strong_alpha, 1.0);
        assert!(
            (config.theme.card_alpha - theme_defaults.card_alpha).abs() < f32::EPSILON,
            "card alpha fallback should match theme default"
        );
        assert!(
            (config.theme.shadow_soft_alpha - theme_defaults.shadow_soft_alpha).abs()
                < f32::EPSILON,
            "shadow soft alpha fallback should match theme default"
        );
        assert_eq!(config.theme.shadow_strong_alpha, 0.0);
        assert_eq!(config.theme.border_width, MAX_BORDER_WIDTH);
        assert_eq!(config.theme.card_radius, MAX_CARD_RADIUS);
    }

    #[test]
    fn sanitize_clamps_alpha_without_defaults() {
        // Validate finite alpha values clamp without forcing theme defaults.
        let mut config = Config::default();
        config.theme.surface_alpha = 1.5;
        config.theme.surface_strong_alpha = -0.2;
        config.theme.card_alpha = 0.2;
        config.theme.shadow_soft_alpha = 2.0;
        config.theme.shadow_strong_alpha = -1.0;
        sanitize_config(&mut config);

        assert_eq!(config.theme.surface_alpha, 1.0);
        assert_eq!(config.theme.surface_strong_alpha, 0.0);
        assert_eq!(config.theme.card_alpha, 0.2);
        assert_eq!(config.theme.shadow_soft_alpha, 1.0);
        assert_eq!(config.theme.shadow_strong_alpha, 0.0);
    }
}
