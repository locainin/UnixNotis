use std::collections::HashSet;

use crate::{
    CardWidgetConfig, SliderWidgetConfig, StatWidgetConfig, ToggleLayout, ToggleWidgetConfig,
    WidgetPluginConfig, WidgetsConfig,
};

#[test]
fn default_widgets_keep_expected_grid_shape() {
    let widgets = WidgetsConfig::default();

    // These counts define the visible stock control-center sections
    assert_eq!(widgets.toggle_layout, ToggleLayout::Horizontal);
    assert_eq!(widgets.toggle_columns, 4);
    assert_eq!(widgets.stat_columns, 2);
    assert_eq!(widgets.card_columns, 2);
    assert_eq!(widgets.toggles.len(), 4);
    assert_eq!(widgets.stats.len(), 3);
    assert_eq!(widgets.cards.len(), 2);
}

#[test]
fn default_toggles_have_unique_stable_kinds() {
    let widgets = WidgetsConfig::default();
    let mut seen = HashSet::new();

    for toggle in &widgets.toggles {
        let kind = toggle.kind.as_deref().expect("default toggle kind");
        assert!(
            seen.insert(kind.to_string()),
            "duplicate toggle kind: {kind}"
        );
    }
}

#[test]
fn default_night_toggle_uses_shipped_relative_scripts() {
    let night = WidgetsConfig::default()
        .toggles
        .into_iter()
        .find(|toggle| toggle.kind.as_deref() == Some("night"))
        .expect("night toggle");

    // The commands stay config-owned while core startup guarantees the files exist
    assert_eq!(
        night.state_cmd.as_deref(),
        Some("scripts/unixnotis-blue-light-state")
    );
    assert_eq!(
        night.on_cmd.as_deref(),
        Some("scripts/unixnotis-blue-light-on")
    );
    assert_eq!(
        night.off_cmd.as_deref(),
        Some("scripts/unixnotis-blue-light-off")
    );
    assert_eq!(night.toggle_cmd, None);
    assert_eq!(night.watch_cmd, None);
}

#[test]
fn default_toggles_keep_commands_config_owned() {
    let widgets = WidgetsConfig::default();

    for toggle in widgets.toggles {
        for command in [
            toggle.state_cmd.as_deref(),
            toggle.toggle_cmd.as_deref(),
            toggle.on_cmd.as_deref(),
            toggle.off_cmd.as_deref(),
            toggle.watch_cmd.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            // Stock commands should stay relative or PATH based so config files remain portable
            assert!(
                !command.starts_with('/'),
                "absolute command leaked: {command}"
            );
        }
    }
}

#[test]
fn custom_toggles_round_trip_arbitrary_user_commands() {
    let widgets: WidgetsConfig = toml::from_str(
        r#"
        [[toggles]]
        enabled = true
        label = "Build"
        icon = "applications-development-symbolic"
        state_cmd = "scripts/build-state"
        toggle_cmd = "sh -c 'make test && notify-send done'"
        on_cmd = "scripts/build-on"
        off_cmd = "scripts/build-off"
        watch_cmd = "scripts/build-watch"
        "#,
    )
    .expect("widgets config should parse");

    let toggle = widgets.toggles.first().expect("custom toggle");
    assert_eq!(toggle.label, "Build");
    assert_eq!(toggle.state_cmd.as_deref(), Some("scripts/build-state"));
    assert_eq!(
        toggle.toggle_cmd.as_deref(),
        Some("sh -c 'make test && notify-send done'")
    );
    assert_eq!(toggle.on_cmd.as_deref(), Some("scripts/build-on"));
    assert_eq!(toggle.off_cmd.as_deref(), Some("scripts/build-off"));
    assert_eq!(toggle.watch_cmd.as_deref(), Some("scripts/build-watch"));
}

#[test]
fn blank_toggle_default_is_disabled_and_action_free() {
    let toggle = ToggleWidgetConfig::default();

    assert!(!toggle.enabled);
    assert_eq!(toggle.kind, None);
    assert_eq!(toggle.state_cmd, None);
    assert_eq!(toggle.toggle_cmd, None);
    assert_eq!(toggle.on_cmd, None);
    assert_eq!(toggle.off_cmd, None);
    assert_eq!(toggle.watch_cmd, None);
}

#[test]
fn default_panel_actions_keep_expected_labels_icons_and_modes() {
    let actions = [
        crate::PanelActionConfig::widgets(),
        crate::PanelActionConfig::dnd(),
        crate::PanelActionConfig::clear(),
        crate::PanelActionConfig::search(),
        crate::PanelActionConfig::close(),
    ];

    assert_eq!(actions[0].label, "Widgets");
    assert_eq!(actions[0].icon, "applications-system-symbolic");
    assert_eq!(actions[1].label, "DND");
    assert_eq!(actions[1].tooltip, "Silence incoming notifications");
    assert_eq!(actions[2].icon, "user-trash-symbolic");
    assert_eq!(actions[3].label, "Search");
    assert!(actions[3].icon_only);
    assert_eq!(actions[4].label, "Close");
    assert!(actions[4].icon_only);
}

#[test]
fn default_card_widgets_keep_builtin_identity_and_layout() {
    let widgets = WidgetsConfig::default();
    let calendar = &widgets.cards[0];
    let weather = &widgets.cards[1];

    assert_eq!(calendar.kind.as_deref(), Some("calendar"));
    assert_eq!(calendar.title, "Calendar");
    assert_eq!(calendar.icon.as_deref(), Some("x-office-calendar-symbolic"));
    assert_eq!(calendar.min_height, 180);
    assert_eq!(calendar.cmd, None);

    assert_eq!(weather.kind.as_deref(), Some("weather"));
    assert_eq!(weather.title, "Weather");
    assert_eq!(weather.subtitle.as_deref(), Some("No data"));
    assert_eq!(weather.icon.as_deref(), Some("weather-clear-symbolic"));
    assert_eq!(weather.min_height, 160);
}

#[test]
fn default_slider_widgets_keep_stock_commands() {
    let widgets = WidgetsConfig::default();

    assert!(widgets.volume.enabled);
    assert_eq!(widgets.volume.label, "Volume");
    assert_eq!(widgets.volume.get_cmd, SliderWidgetConfig::WPCTL_GET);
    assert_eq!(widgets.volume.set_cmd, SliderWidgetConfig::WPCTL_SET);
    assert_eq!(
        widgets.volume.toggle_cmd.as_deref(),
        Some(SliderWidgetConfig::WPCTL_TOGGLE)
    );
    assert_eq!(widgets.volume.watch_cmd, None);

    assert!(widgets.brightness.enabled);
    assert_eq!(widgets.brightness.label, "Brightness");
    assert_eq!(widgets.brightness.get_cmd, "brightnessctl -m");
    assert_eq!(widgets.brightness.set_cmd, "brightnessctl s {value}%");
    assert_eq!(widgets.brightness.watch_cmd, None);
}

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
fn blank_card_and_stat_defaults_are_disabled_placeholders() {
    let card = CardWidgetConfig::default();
    let stat = StatWidgetConfig::default();

    assert!(!card.enabled);
    assert_eq!(card.title, "Card");
    assert_eq!(card.min_height, 120);
    assert!(!stat.enabled);
    assert_eq!(stat.label, "Stat");
    assert_eq!(stat.min_height, 72);
}

#[test]
fn widget_plugin_defaults_keep_contract_limits() {
    let plugin = WidgetPluginConfig::default();

    assert_eq!(plugin.api_version, WidgetPluginConfig::API_VERSION_V1);
    assert_eq!(plugin.command, "");
    assert_eq!(plugin.timeout_ms, 2_000);
    assert_eq!(plugin.max_output_bytes, 16 * 1024);
}
