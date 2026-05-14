use std::collections::HashSet;

use crate::{ToggleLayout, ToggleWidgetConfig, WidgetsConfig};

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
