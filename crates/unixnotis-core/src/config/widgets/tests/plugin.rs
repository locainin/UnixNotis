use crate::{CommandSpec, WidgetPluginConfig};

#[test]
fn widget_plugin_defaults_keep_contract_limits() {
    let plugin = WidgetPluginConfig::default();

    assert_eq!(plugin.api_version, WidgetPluginConfig::API_VERSION_V1);
    assert!(plugin.command.is_empty());
    assert_eq!(plugin.timeout_ms, 2_000);
    assert_eq!(plugin.max_output_bytes, 16 * 1024);
}

#[test]
fn widget_plugin_partial_toml_uses_default_limits() {
    let plugin: WidgetPluginConfig = toml::from_str(
        r#"
        command = { mode = "direct", program = "scripts/widget" }
        "#,
    )
    .expect("plugin should parse");

    assert_eq!(plugin.api_version, WidgetPluginConfig::API_VERSION_V1);
    assert_eq!(
        plugin.command,
        CommandSpec::direct("scripts/widget", std::iter::empty::<&str>())
    );
    assert_eq!(plugin.timeout_ms, WidgetPluginConfig::default().timeout_ms);
    assert_eq!(
        plugin.max_output_bytes,
        WidgetPluginConfig::default().max_output_bytes
    );
}
