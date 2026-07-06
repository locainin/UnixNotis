use crate::{StatWidgetConfig, WidgetPluginConfig, WidgetsConfig};

#[test]
fn default_stat_widgets_keep_builtin_commands() {
    let widgets = WidgetsConfig::default();
    let expected = [
        ("CPU", "utilities-system-monitor-symbolic", "builtin:cpu"),
        ("RAM", "drive-harddisk-symbolic", "builtin:memory"),
        ("Battery", "battery-full-symbolic", "builtin:battery"),
    ];

    for (stat, (label, icon, command)) in widgets.stats.iter().zip(expected) {
        assert!(stat.enabled);
        assert_eq!(stat.label, label);
        assert_eq!(stat.icon.as_deref(), Some(icon));
        assert_eq!(stat.cmd.as_deref(), Some(command));
        assert_eq!(stat.min_height, 72);
    }
}

#[test]
fn blank_stat_default_is_disabled_placeholder() {
    let stat = StatWidgetConfig::default();

    assert!(!stat.enabled);
    assert_eq!(stat.label, "Stat");
    assert_eq!(stat.icon, None);
    assert_eq!(stat.kind, None);
    assert_eq!(stat.cmd, None);
    assert_eq!(stat.plugin, None);
    assert_eq!(stat.min_height, 72);
}

#[test]
fn custom_stat_plugin_config_parses_with_command_fallback() {
    let stat: StatWidgetConfig = toml::from_str(
        r#"
        enabled = true
        label = "GPU"
        icon = "video-display-symbolic"
        kind = "gpu"
        cmd = "scripts/gpu-fallback"
        min_height = 96

        [plugin]
        api_version = 1
        command = "scripts/gpu-plugin"
        timeout_ms = 1500
        max_output_bytes = 2048
        "#,
    )
    .expect("stat should parse");

    assert!(stat.enabled);
    assert_eq!(stat.label, "GPU");
    assert_eq!(stat.icon.as_deref(), Some("video-display-symbolic"));
    assert_eq!(stat.kind.as_deref(), Some("gpu"));
    assert_eq!(stat.cmd.as_deref(), Some("scripts/gpu-fallback"));
    assert_eq!(stat.min_height, 96);
    assert_eq!(
        stat.plugin,
        Some(WidgetPluginConfig {
            api_version: 1,
            command: "scripts/gpu-plugin".to_string(),
            timeout_ms: 1500,
            max_output_bytes: 2048,
        })
    );
}
