use super::*;
use crate::{
    Anchor, EmptyStateAlignment, Margins, PanelKeyboardInteractivity, PANEL_HEIGHT_PERCENT_DEFAULT,
};

#[test]
fn default_panel_config_keeps_expected_layout_and_text_contract() {
    let panel = PanelConfig::default();

    assert!(matches!(panel.anchor, Anchor::Right));
    assert_eq!(
        panel.margin,
        Margins {
            top: 16,
            right: 10,
            bottom: 14,
            left: 10,
        }
    );
    assert_eq!(panel.width, 420);
    assert_eq!(panel.height, PANEL_HEIGHT_PERCENT_DEFAULT);
    assert_eq!(panel.height_override, None);
    assert!(matches!(
        panel.keyboard_interactivity,
        PanelKeyboardInteractivity::OnDemand
    ));
    assert_eq!(panel.title, "Notifications");
    assert_eq!(panel.empty_text, "NO NOTIFICATIONS");
    assert_eq!(panel.empty_offset_top, 24);
    assert_eq!(panel.empty_alignment, EmptyStateAlignment::Auto);
    assert_eq!(panel.quick_actions_label, "Quick settings");
    assert_eq!(panel.system_status_label, "System health");
    assert_eq!(panel.search_placeholder, "Search app, title, or message");
    assert!(panel.action_row_visible);
    assert!(panel.notification_list_expand);
    assert!(panel.close_on_click_outside);
    assert!(panel.respect_work_area);
}

#[test]
fn partial_panel_values_use_current_presentation_defaults() {
    let panel: PanelConfig =
        toml::from_str("width = 420").expect("partial panel config should parse");

    assert_eq!(panel.quick_actions_label, "Quick settings");
    assert_eq!(panel.system_status_label, "System health");
    assert_eq!(panel.empty_offset_top, 24);
}

#[test]
fn panel_config_parses_custom_ordering_and_action_blocks() {
    let panel: PanelConfig = toml::from_str(
        r#"
        width = 512
        height = 75
        height_override = 640
        section_order = ["notifications", "widgets"]
        widget_order = ["stats", "cards", "media", "toggles", "sliders"]
        action_order = ["close", "search", "widgets", "dnd", "clear"]
        clear_button_placement = "notification-header"
        empty_alignment = "end"

        [search_action]
        label = "Find"
        icon = "system-search-symbolic"
        tooltip = "Find notifications"
        icon_only = false
        "#,
    )
    .expect("panel config should parse");

    assert_eq!(panel.width, 512);
    assert_eq!(panel.height, 75);
    assert_eq!(panel.height_override, Some(640));
    assert_eq!(panel.empty_alignment, EmptyStateAlignment::End);
    assert_eq!(
        panel.section_order,
        vec![PanelSection::Notifications, PanelSection::Widgets]
    );
    assert_eq!(
        panel.widget_order,
        vec![
            PanelWidgetSection::Stats,
            PanelWidgetSection::Cards,
            PanelWidgetSection::Media,
            PanelWidgetSection::Toggles,
            PanelWidgetSection::Sliders,
        ]
    );
    assert_eq!(
        panel.action_order,
        vec![
            PanelActionId::Close,
            PanelActionId::Search,
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
        ]
    );
    assert_eq!(
        panel.clear_button_placement,
        PanelClearButtonPlacement::NotificationHeader
    );
    assert_eq!(panel.search_action.label, "Find");
    assert_eq!(panel.search_action.tooltip, "Find notifications");
    assert!(!panel.search_action.icon_only);
}

#[test]
fn panel_config_defaults_are_filled_for_partial_toml() {
    let panel: PanelConfig = toml::from_str(
        r#"
        title = "Ops"
        action_row_visible = false
        "#,
    )
    .expect("partial panel config should parse");

    assert_eq!(panel.title, "Ops");
    assert!(!panel.action_row_visible);
    assert_eq!(panel.width, PanelConfig::default().width);
    assert_eq!(panel.action_order, default_panel_action_order());
    assert_eq!(panel.focus_action, PanelActionConfig::widgets());
}
