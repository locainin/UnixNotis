use std::collections::HashSet;

use crate::{CommandSpec, ToggleLayout, ToggleWidgetConfig, WidgetsConfig};

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
        night.state_cmd,
        Some(CommandSpec::direct(
            "scripts/unixnotis-blue-light-state",
            [] as [&str; 0]
        ))
    );
    assert_eq!(
        night.on_cmd,
        Some(CommandSpec::direct(
            "scripts/unixnotis-blue-light-on",
            [] as [&str; 0]
        ))
    );
    assert_eq!(
        night.off_cmd,
        Some(CommandSpec::direct(
            "scripts/unixnotis-blue-light-off",
            [] as [&str; 0]
        ))
    );
    assert_eq!(night.toggle_cmd, None);
    assert_eq!(night.watch_cmd, None);
}

#[test]
fn default_toggles_keep_commands_config_owned() {
    let widgets = WidgetsConfig::default();

    for toggle in widgets.toggles {
        for command in [
            toggle.state_cmd.as_ref(),
            toggle.toggle_cmd.as_ref(),
            toggle.on_cmd.as_ref(),
            toggle.off_cmd.as_ref(),
            toggle.watch_cmd.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            // Stock commands should stay relative or PATH based so config files remain portable
            assert!(
                command.program().is_none_or(|program| !program.is_absolute()),
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
        id = "build"
        label = "Build"
        icon = "applications-development-symbolic"
        icon_asset = "assets/build.svg"
        state_cmd = { mode = "direct", program = "scripts/build-state" }
        toggle_cmd = { mode = "shell", script = "make test && notify-send done" }
        on_cmd = { mode = "direct", program = "scripts/build-on" }
        off_cmd = { mode = "direct", program = "scripts/build-off" }
        watch_cmd = { mode = "direct", program = "scripts/build-watch" }
        "#,
    )
    .expect("widgets config should parse");

    let toggle = widgets.toggles.first().expect("custom toggle");
    assert_eq!(toggle.kind.as_deref(), Some("build"));
    assert_eq!(toggle.label, "Build");
    assert_eq!(toggle.icon_asset.as_deref(), Some("assets/build.svg"));
    assert_eq!(
        toggle.state_cmd,
        Some(CommandSpec::direct("scripts/build-state", [] as [&str; 0]))
    );
    assert_eq!(
        toggle.toggle_cmd,
        Some(CommandSpec::shell("make test && notify-send done"))
    );
    assert_eq!(
        toggle.on_cmd,
        Some(CommandSpec::direct("scripts/build-on", [] as [&str; 0]))
    );
    assert_eq!(
        toggle.off_cmd,
        Some(CommandSpec::direct("scripts/build-off", [] as [&str; 0]))
    );
    assert_eq!(
        toggle.watch_cmd,
        Some(CommandSpec::direct("scripts/build-watch", [] as [&str; 0]))
    );
}

#[test]
fn blank_toggle_default_is_disabled_and_action_free() {
    let toggle = ToggleWidgetConfig::default();

    assert!(!toggle.enabled);
    assert_eq!(toggle.kind, None);
    assert_eq!(toggle.icon_asset, None);
    assert_eq!(toggle.state_cmd, None);
    assert_eq!(toggle.toggle_cmd, None);
    assert_eq!(toggle.on_cmd, None);
    assert_eq!(toggle.off_cmd, None);
    assert_eq!(toggle.watch_cmd, None);
}

#[test]
fn toggle_layout_parses_kebab_case_values() {
    #[derive(serde::Deserialize)]
    struct LayoutFixture {
        layout: ToggleLayout,
    }

    let horizontal: LayoutFixture =
        toml::from_str("layout = \"horizontal\"").expect("horizontal should parse");
    let vertical: LayoutFixture =
        toml::from_str("layout = \"vertical\"").expect("vertical should parse");

    assert_eq!(horizontal.layout, ToggleLayout::Horizontal);
    assert_eq!(vertical.layout, ToggleLayout::Vertical);
}
