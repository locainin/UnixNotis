use super::super::super::super::widget_config::WidgetPluginConfig;
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
