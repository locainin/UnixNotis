use super::*;
use crate::{CommandSpec, PanelSection, PanelWidgetSection, WidgetDensity};

const LEGACY_FIXTURE: &str = include_str!("fixtures/config-v0.toml");
const V2_PARTIAL_FIXTURE: &str = include_str!("fixtures/config-v2-partial.toml");

fn deserialize_config(contents: &str) -> Result<(Config, Vec<String>), String> {
    let (config, ignored_keys, _migrated_paths) = deserialize_config_with_migrations(contents)?;
    Ok((config, ignored_keys))
}

#[test]
fn unversioned_fixture_migrates_to_the_legacy_layout() {
    let (config, ignored) = deserialize_config(LEGACY_FIXTURE).expect("migrate legacy config");

    assert!(ignored.is_empty());
    assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
    assert!(config.panel.quick_actions_label.is_empty());
    assert_eq!(config.panel.empty_offset_top, 120);
    assert_eq!(
        config.panel.section_order,
        vec![PanelSection::Widgets, PanelSection::Notifications]
    );
    assert_eq!(
        config.panel.widget_order,
        vec![
            PanelWidgetSection::Sliders,
            PanelWidgetSection::Media,
            PanelWidgetSection::Toggles,
            PanelWidgetSection::Stats,
            PanelWidgetSection::Cards,
        ]
    );
    assert_eq!(config.widgets.toggle_columns, 4);
    assert_eq!(config.widgets.volume.segments, 0);
    assert!(!config.widgets.volume.show_sublabels);
    assert!(config.widgets.cards.iter().all(|card| card.enabled));
    assert_eq!(config.media.art_size_px, 50);
}

#[test]
fn version_two_partial_fixture_migrates_and_uses_current_defaults() {
    let (config, ignored) =
        deserialize_config(V2_PARTIAL_FIXTURE).expect("parse version two config");

    assert!(ignored.is_empty());
    assert_eq!(config.panel.quick_actions_label, "Quick settings");
    assert_eq!(config.panel.empty_offset_top, 24);
    assert_eq!(config.widgets.toggle_columns, 2);
    assert_eq!(config.widgets.volume.segments, 10);
    assert_eq!(config.media.art_size_px, 48);
}

#[test]
fn version_two_commands_migrate_quoted_punctuation_to_direct_and_operators_to_shell() {
    let input = r#"
        config_version = 2

        [widgets.volume]
        get_cmd = "printf '%s\\n' 'battery|charging'"
        set_cmd = "producer | parser"
    "#;

    let (config, ignored) = deserialize_config(input).expect("migrate version two commands");

    assert!(ignored.is_empty());
    assert_eq!(
        config.widgets.volume.get_cmd,
        CommandSpec::direct("printf", ["%s\\n", "battery|charging"])
    );
    assert_eq!(
        config.widgets.volume.set_cmd,
        CommandSpec::shell("producer | parser")
    );
}

#[test]
fn version_three_requires_explicit_command_mode() {
    let legacy = r#"
        config_version = 3

        [widgets.volume]
        get_cmd = "printf ready"
    "#;
    let error = deserialize_config(legacy).expect_err("reject a string command in version three");

    assert!(error.contains("expected internally tagged enum CommandSpec"));
}

#[test]
fn version_three_accepts_structured_direct_commands_without_inference() {
    let input = r#"
        config_version = 3

        [widgets.volume.get_cmd]
        mode = "direct"
        program = "printf"
        args = ["battery|charging"]
    "#;

    let (config, ignored) = deserialize_config(input).expect("parse version three command");

    assert!(ignored.is_empty());
    assert_eq!(
        config.widgets.volume.get_cmd,
        CommandSpec::direct("printf", ["battery|charging"])
    );
}

#[test]
fn missing_local_art_policy_uses_the_current_default() {
    let (config, _) = deserialize_config("config_version = 4\n[media]\n").expect("parse media");
    assert_eq!(
        config.media.local_art_policy,
        crate::MediaLocalArtPolicy::AllAdmitted
    );
}

#[test]
fn old_explicit_empty_exact_policy_is_preserved() {
    let (config, _) = deserialize_config(
        "config_version = 3\n[media]\nlocal_art_policy = \"exact_executable_only\"\n",
    )
    .expect("old media config should migrate");
    assert_eq!(
        config.media.local_art_policy,
        crate::MediaLocalArtPolicy::ExactExecutableOnly
    );
    assert!(config.media.local_art_executable_allowlist.is_empty());
}

#[test]
fn old_explicit_allowlist_remains_exact() {
    let (config, _) = deserialize_config(
        "config_version = 3\n[media]\nlocal_art_policy = \"exact_executable_only\"\nlocal_art_executable_allowlist = [\"/usr/bin/player\"]\n",
    )
    .expect("old explicit media config should migrate");
    assert_eq!(
        config.media.local_art_policy,
        crate::MediaLocalArtPolicy::ExactExecutableOnly
    );
    assert_eq!(
        config.media.local_art_executable_allowlist,
        ["/usr/bin/player"]
    );
}

#[test]
fn current_explicit_empty_exact_policy_is_preserved() {
    let (config, _) = deserialize_config(
        "config_version = 4\n[media]\nlocal_art_policy = \"exact_executable_only\"\n",
    )
    .expect("current media config should preserve explicit policy");
    assert_eq!(
        config.media.local_art_policy,
        crate::MediaLocalArtPolicy::ExactExecutableOnly
    );
}

#[test]
fn future_schema_is_rejected_instead_of_guessed() {
    let error = deserialize_config("config_version = 999\n").expect_err("reject future config");

    assert!(error.contains("newer than supported"));
}

#[test]
fn negative_schema_version_is_rejected_instead_of_wrapping() {
    let error = deserialize_config("config_version = -1\n").expect_err("reject negative version");

    assert!(error.contains("non-negative integer"));
}

#[test]
fn explicit_legacy_values_remain_authoritative_during_migration() {
    let text = "[panel]\nquick_actions_label = 'Custom'\nempty_offset_top = 77\n";
    let (config, _) = deserialize_config(text).expect("migrate explicit values");

    assert_eq!(config.panel.quick_actions_label, "Custom");
    assert_eq!(config.panel.empty_offset_top, 77);
}

#[test]
fn root_scalar_changes_do_not_report_an_empty_migration_path() {
    let before = toml::Value::Integer(1);
    let after = toml::Value::Integer(2);
    let mut paths = Vec::new();

    collect_changed_paths("", Some(&before), Some(&after), &mut paths);

    assert!(paths.is_empty());
}

#[test]
fn empty_unversioned_config_receives_complete_legacy_defaults() {
    let (config, ignored) = deserialize_config("").expect("migrate empty legacy config");

    assert!(ignored.is_empty());
    assert!(config.panel.quick_actions_label.is_empty());
    assert!(config.panel.system_status_label.is_empty());
    assert_eq!(config.panel.empty_offset_top, 120);
    assert_eq!(config.widgets.density, WidgetDensity::Comfortable);
    assert_eq!(config.widgets.toggle_columns, 4);
    assert_eq!(config.widgets.stat_columns, 2);
    assert_eq!(config.widgets.card_columns, 2);
    assert_eq!(config.widgets.volume.segments, 0);
    assert_eq!(config.widgets.brightness.segments, 0);
    assert!(config.widgets.cards.iter().all(|card| card.enabled));
    assert_eq!(config.media.art_size_px, 50);
    assert_eq!(config.media.text_width_floor_px, 140);
    assert_eq!(config.media.content_spacing_px, 10);
    assert_eq!(config.media.control_spacing_px, 6);
    assert_eq!(config.media.navigation_spacing_px, 6);
}

#[test]
fn legacy_widgets_without_slider_tables_receive_slider_compatibility() {
    let (config, _) = deserialize_config("[widgets]\ntoggle_columns = 3\n")
        .expect("migrate legacy widgets without sliders");

    // Explicit layout remains authoritative while omitted slider visuals stay historic
    assert_eq!(config.widgets.toggle_columns, 3);
    assert_eq!(config.widgets.volume.segments, 0);
    assert!(!config.widgets.volume.show_sublabels);
    assert!(config.widgets.volume.sublabel_min.is_empty());
    assert!(config.widgets.volume.sublabel_max.is_empty());
    assert_eq!(config.widgets.brightness.segments, 0);
    assert!(!config.widgets.brightness.show_sublabels);
    assert!(config.widgets.brightness.sublabel_min.is_empty());
    assert!(config.widgets.brightness.sublabel_max.is_empty());
}

#[test]
fn legacy_config_without_panel_table_receives_panel_compatibility() {
    let (config, _) = deserialize_config("[general]\ndnd_default = true\n")
        .expect("migrate legacy config without panel");

    assert!(config.panel.quick_actions_label.is_empty());
    assert!(config.panel.system_status_label.is_empty());
    assert_eq!(config.panel.empty_offset_top, 120);
    assert_eq!(
        config.panel.section_order,
        vec![PanelSection::Widgets, PanelSection::Notifications]
    );
}

#[test]
fn legacy_config_without_media_table_receives_media_compatibility() {
    let (config, _) =
        deserialize_config("[panel]\nwidth = 480\n").expect("migrate legacy config without media");

    assert_eq!(config.media.art_size_px, 50);
    assert_eq!(config.media.text_width_floor_px, 140);
    assert_eq!(config.media.content_spacing_px, 10);
    assert_eq!(config.media.control_spacing_px, 6);
    assert_eq!(config.media.navigation_spacing_px, 6);
}

#[test]
fn legacy_config_without_widgets_table_receives_widget_compatibility() {
    let (config, _) = deserialize_config("[panel]\nwidth = 480\n")
        .expect("migrate legacy config without widgets");

    assert_eq!(config.widgets.density, WidgetDensity::Comfortable);
    assert_eq!(config.widgets.toggle_columns, 4);
    assert_eq!(config.widgets.stat_columns, 2);
    assert_eq!(config.widgets.card_columns, 2);
    assert_eq!(config.widgets.volume.segments, 0);
    assert_eq!(config.widgets.brightness.segments, 0);
    assert!(config.widgets.cards.iter().all(|card| card.enabled));
}

#[test]
fn malformed_legacy_table_is_reported_instead_of_replaced() {
    let error = deserialize_config("panel = 'not a table'\n")
        .expect_err("invalid legacy table should remain a type error");

    assert!(error.contains("invalid type"));
}
