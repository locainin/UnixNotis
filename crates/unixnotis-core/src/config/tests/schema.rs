use super::*;
use crate::{PanelSection, PanelWidgetSection};

const LEGACY_FIXTURE: &str = include_str!("fixtures/config-v0.toml");
const CURRENT_PARTIAL_FIXTURE: &str = include_str!("fixtures/config-v2-partial.toml");

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
fn current_partial_fixture_uses_current_defaults() {
    let (config, ignored) =
        deserialize_config(CURRENT_PARTIAL_FIXTURE).expect("parse current config");

    assert!(ignored.is_empty());
    assert_eq!(config.panel.quick_actions_label, "Quick settings");
    assert_eq!(config.panel.empty_offset_top, 24);
    assert_eq!(config.widgets.toggle_columns, 2);
    assert_eq!(config.widgets.volume.segments, 10);
    assert_eq!(config.media.art_size_px, 48);
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
