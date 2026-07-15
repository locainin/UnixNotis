use super::super::super::super::widgets::WidgetPluginConfig;
use super::super::*;
use crate::Config;

#[test]
fn sanitize_widget_plugin_clamps_bounds_and_trim_command() {
    // Plugin commands should be trimmed and bounded before any worker runs them
    let mut config = Config::default();
    config.widgets.stats[0].plugin = Some(WidgetPluginConfig {
        command: "  script arg  ".to_string(),
        timeout_ms: super::super::plugins::MAX_PLUGIN_TIMEOUT_MS + 1,
        max_output_bytes: super::super::plugins::MAX_PLUGIN_OUTPUT_BYTES + 10,
        ..WidgetPluginConfig::default()
    });
    sanitize_config(&mut config);

    let plugin = config.widgets.stats[0]
        .plugin
        .as_ref()
        .expect("plugin should remain enabled");
    assert_eq!(plugin.command, "script arg");
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
        command: "sh -c 'echo pwned | cat'".to_string(),
        ..WidgetPluginConfig::default()
    });
    sanitize_config(&mut config);
    assert!(config.widgets.cards[0].plugin.is_none());
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
