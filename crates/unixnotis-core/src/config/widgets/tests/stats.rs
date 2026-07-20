use crate::{CommandSpec, StatWidgetConfig, WidgetPluginConfig, WidgetsConfig};

#[test]
fn default_stat_widgets_keep_builtin_commands() {
    let widgets = WidgetsConfig::default();
    let expected = [
        (
            "CPU",
            "utilities-system-monitor-symbolic",
            "cpu",
            "builtin:cpu",
        ),
        ("RAM", "drive-harddisk-symbolic", "ram", "builtin:memory"),
        (
            "Battery",
            "battery-full-symbolic",
            "battery",
            "builtin:battery",
        ),
    ];

    for (stat, (label, icon, kind, command)) in widgets.stats.iter().zip(expected) {
        assert!(stat.enabled);
        assert_eq!(stat.label, label);
        assert_eq!(stat.icon.as_deref(), Some(icon));
        assert_eq!(stat.icon_asset, None);
        assert_eq!(stat.kind.as_deref(), Some(kind));
        assert_eq!(
            stat.cmd,
            Some(CommandSpec::direct(command, [] as [&str; 0]))
        );
        assert_eq!(stat.min_height, 72);
    }
}

#[test]
fn blank_stat_default_is_disabled_placeholder() {
    let stat = StatWidgetConfig::default();

    assert!(!stat.enabled);
    assert_eq!(stat.label, "Stat");
    assert_eq!(stat.icon, None);
    assert_eq!(stat.icon_asset, None);
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
        icon_asset = "assets/gpu.svg"
        kind = "gpu"
        cmd = { mode = "direct", program = "scripts/gpu-fallback" }
        min_height = 96

        [plugin]
        api_version = 1
        command = { mode = "direct", program = "scripts/gpu-plugin" }
        timeout_ms = 1500
        max_output_bytes = 2048
        "#,
    )
    .expect("stat should parse");

    assert!(stat.enabled);
    assert_eq!(stat.label, "GPU");
    assert_eq!(stat.icon.as_deref(), Some("video-display-symbolic"));
    assert_eq!(stat.icon_asset.as_deref(), Some("assets/gpu.svg"));
    assert_eq!(stat.kind.as_deref(), Some("gpu"));
    assert_eq!(
        stat.cmd,
        Some(CommandSpec::direct("scripts/gpu-fallback", [] as [&str; 0]))
    );
    assert_eq!(stat.min_height, 96);
    assert_eq!(
        stat.plugin,
        Some(WidgetPluginConfig {
            api_version: 1,
            command: CommandSpec::direct("scripts/gpu-plugin", [] as [&str; 0]),
            timeout_ms: 1500,
            max_output_bytes: 2048,
        })
    );
}
