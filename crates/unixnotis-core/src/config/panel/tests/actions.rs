use super::*;

#[derive(serde::Deserialize)]
struct ActionOrderFixture {
    action_order: Vec<PanelActionId>,
}

#[test]
fn default_panel_actions_keep_expected_labels_icons_and_modes() {
    let actions = [
        PanelActionConfig::widgets(),
        PanelActionConfig::dnd(),
        PanelActionConfig::clear(),
        PanelActionConfig::search(),
        PanelActionConfig::close(),
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
fn default_panel_action_order_keeps_close_out_of_action_row() {
    assert_eq!(
        default_panel_action_order(),
        vec![
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
            PanelActionId::Search,
        ]
    );
}

#[test]
fn panel_action_ids_parse_from_kebab_case_config_values() {
    let fixture: ActionOrderFixture =
        toml::from_str(r#"action_order = ["widgets", "dnd", "clear", "search", "close"]"#)
            .expect("panel action ids should parse");

    assert_eq!(
        fixture.action_order,
        vec![
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
            PanelActionId::Search,
            PanelActionId::Close,
        ]
    );
}

#[test]
fn clear_button_placement_defaults_to_action_row() {
    assert_eq!(
        PanelClearButtonPlacement::default(),
        PanelClearButtonPlacement::ActionRow
    );
}
