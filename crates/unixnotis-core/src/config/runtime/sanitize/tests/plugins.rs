#![allow(
    clippy::float_cmp,
    reason = "sanitization assigns exact finite constants and test inputs"
)]

use super::super::super::super::widgets::WidgetPluginConfig;
use super::super::*;
use crate::{CommandSpec, Config};

#[test]
fn sanitize_widget_plugin_clamps_bounds_and_preserves_literal_arguments() {
    let mut config = Config::default();
    config.widgets.stats[0].plugin = Some(WidgetPluginConfig {
        command: CommandSpec::direct("script", ["  literal arg  "]),
        timeout_ms: super::super::plugins::MAX_PLUGIN_TIMEOUT_MS + 1,
        max_output_bytes: super::super::plugins::MAX_PLUGIN_OUTPUT_BYTES + 10,
        ..WidgetPluginConfig::default()
    });
    sanitize_config(&mut config);

    let plugin = config.widgets.stats[0]
        .plugin
        .as_ref()
        .expect("plugin should remain enabled");
    assert_eq!(
        plugin.command,
        CommandSpec::direct("script", ["  literal arg  "])
    );
    assert_eq!(
        plugin.timeout_ms,
        super::super::plugins::MAX_PLUGIN_TIMEOUT_MS
    );
    assert_eq!(
        plugin.max_output_bytes,
        super::super::plugins::MAX_PLUGIN_OUTPUT_BYTES
    );
}

#[test]
fn sanitize_widget_plugin_rejects_shell_meta_commands() {
    // Shell syntax is not allowed in the simple plugin command field
    let mut config = Config::default();
    config.widgets.cards[0].plugin = Some(WidgetPluginConfig {
        command: CommandSpec::shell("echo pwned | cat"),
        ..WidgetPluginConfig::default()
    });
    sanitize_config(&mut config);
    assert!(config.widgets.cards[0].plugin.is_none());
}

#[test]
fn sanitize_widget_plugin_rejects_direct_shell_interpreters() {
    let mut config = Config::default();
    config.widgets.cards[0].plugin = Some(WidgetPluginConfig {
        command: CommandSpec::direct("sh", ["-c", "printf unsafe"]),
        ..WidgetPluginConfig::default()
    });

    sanitize_config(&mut config);

    assert!(config.widgets.cards[0].plugin.is_none());
}

#[test]
fn sanitize_widget_plugin_keeps_direct_shell_scripts_with_long_options() {
    for (shell, option, script) in [
        ("bash", "--norc", "script.sh"),
        ("fish", "--no-config", "script.fish"),
    ] {
        let mut config = Config::default();
        config.widgets.cards[0].plugin = Some(WidgetPluginConfig {
            command: CommandSpec::direct(shell, [option, script]),
            ..WidgetPluginConfig::default()
        });

        sanitize_config(&mut config);

        assert!(
            config.widgets.cards[0].plugin.is_some(),
            "{shell} script plugins must remain enabled"
        );
    }
}

#[test]
fn sanitize_widget_options_caps_decorative_layout_counts() {
    let mut config = Config::default();
    config.widgets.volume.segments = 999;
    config.widgets.volume.sublabel_min = "  abcdefghijklmnopqrstuvwxyz0123456789  ".to_string();
    config.widgets.cards[0].carousel_dots = 999;

    sanitize_config(&mut config);

    assert_eq!(config.widgets.volume.segments, 64);
    assert_eq!(
        config.widgets.volume.sublabel_min,
        "abcdefghijklmnopqrstuvwxyz012345"
    );
    assert_eq!(config.widgets.cards[0].carousel_dots, 12);
}

#[test]
fn sanitize_slider_bounds_reset_reversed_equal_and_non_finite_ranges() {
    for (min, max) in [
        (100.0, 0.0),
        (10.0, 10.0),
        (f64::NAN, 100.0),
        (0.0, f64::NAN),
        (f64::NEG_INFINITY, 100.0),
        (0.0, f64::INFINITY),
    ] {
        let mut config = Config::default();
        config.widgets.volume.min = min;
        config.widgets.volume.max = max;

        sanitize_config(&mut config);

        assert_eq!(config.widgets.volume.min, 0.0, "min={min}, max={max}");
        assert_eq!(config.widgets.volume.max, 100.0, "min={min}, max={max}");
    }
}

#[test]
fn sanitize_slider_step_resets_non_finite_non_positive_and_oversized_values() {
    for step in [0.0, -1.0, 25.1, f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
        let mut config = Config::default();
        config.widgets.volume.min = -12.5;
        config.widgets.volume.max = 12.5;
        config.widgets.volume.step = step;

        sanitize_config(&mut config);

        assert_eq!(config.widgets.volume.step, 1.0, "step={step}");
    }
}

#[test]
fn sanitize_slider_keeps_valid_signed_and_narrow_ranges() {
    let mut config = Config::default();
    config.widgets.volume.min = -12.5;
    config.widgets.volume.max = 12.5;
    config.widgets.volume.step = 0.5;
    config.widgets.brightness.min = 0.0;
    config.widgets.brightness.max = 0.25;
    config.widgets.brightness.step = 1.0;

    sanitize_config(&mut config);

    assert_eq!(config.widgets.volume.min, -12.5);
    assert_eq!(config.widgets.volume.max, 12.5);
    assert_eq!(config.widgets.volume.step, 0.5);
    assert_eq!(config.widgets.brightness.step, 0.25);
}

#[test]
fn sanitize_slider_step_keeps_a_step_equal_to_the_full_range() {
    let mut config = Config::default();
    config.widgets.volume.min = -12.5;
    config.widgets.volume.max = 12.5;
    config.widgets.volume.step = 25.0;

    sanitize_config(&mut config);

    assert_eq!(config.widgets.volume.step, 25.0);
}

#[test]
fn plugin_contract_limits_stay_at_expected_byte_and_time_caps() {
    // These limits are part of the runtime safety contract for external widget commands
    assert_eq!(super::super::plugins::MIN_PLUGIN_TIMEOUT_MS, 100);
    assert_eq!(super::super::plugins::MAX_PLUGIN_TIMEOUT_MS, 30_000);
    assert_eq!(super::super::plugins::MIN_PLUGIN_OUTPUT_BYTES, 128);
    assert_eq!(super::super::plugins::MAX_PLUGIN_OUTPUT_BYTES, 128 * 1024);
}

#[test]
fn sanitize_widget_counts_bounds_each_group_and_the_combined_tree() {
    let defaults = Config::default();
    let mut config = Config::default();
    config.widgets.toggles = vec![defaults.widgets.toggles[0].clone(); MAX_TOGGLE_WIDGETS + 5];
    config.widgets.stats = vec![defaults.widgets.stats[0].clone(); MAX_STAT_WIDGETS + 5];
    config.widgets.cards = vec![defaults.widgets.cards[0].clone(); MAX_CARD_WIDGETS + 5];

    sanitize_config(&mut config);

    // Group priority keeps all controls and stats while cards use the remaining budget
    assert_eq!(config.widgets.toggles.len(), MAX_TOGGLE_WIDGETS);
    assert_eq!(config.widgets.stats.len(), MAX_STAT_WIDGETS);
    assert_eq!(
        config.widgets.cards.len(),
        MAX_TOTAL_WIDGETS - MAX_TOGGLE_WIDGETS - MAX_STAT_WIDGETS
    );
    assert_eq!(
        config.widgets.toggles.len() + config.widgets.stats.len() + config.widgets.cards.len(),
        MAX_TOTAL_WIDGETS
    );
}

#[test]
fn sanitize_widget_counts_keeps_normal_configs_unchanged() {
    let mut config = Config::default();
    let expected = config.widgets.clone();

    sanitize_config(&mut config);

    assert_eq!(config.widgets, expected);
}
