use gtk::prelude::*;
use unixnotis_core::{css::hooks, PanelActionConfig, PanelActionId, PanelConfig};

use super::{
    action_order_contains_close, apply_panel_action_config, build_panel_actions,
    resolved_clear_action,
};

fn child_with_class(parent: &gtk::Box, class_name: &str) -> Option<gtk::Widget> {
    let mut child = parent.first_child();
    while let Some(widget) = child {
        if widget.has_css_class(class_name) {
            return Some(widget);
        }
        child = widget.next_sibling();
    }
    None
}

#[test]
fn legacy_clear_label_updates_stock_clear_action() {
    let config = PanelConfig {
        clear_label: "Wipe".to_string(),
        ..PanelConfig::default()
    };

    assert_eq!(resolved_clear_action(&config).label, "Wipe");
}

#[test]
fn custom_clear_action_with_clear_label_is_not_rewritten() {
    let mut custom = PanelActionConfig::clear();
    custom.icon = "edit-delete-symbolic".to_string();
    let config = PanelConfig {
        clear_label: "Wipe".to_string(),
        clear_action: custom.clone(),
        ..PanelConfig::default()
    };

    assert_eq!(resolved_clear_action(&config), custom);
}

#[test]
fn action_order_contains_close_only_when_close_is_configured() {
    assert!(!action_order_contains_close(&[
        PanelActionId::Widgets,
        PanelActionId::Dnd,
        PanelActionId::Clear,
        PanelActionId::Search,
    ]));
    assert!(action_order_contains_close(&[
        PanelActionId::Close,
        PanelActionId::Search,
    ]));
}

#[gtk::test]
fn build_panel_actions_places_explicit_close_inside_action_group() {
    let config = PanelConfig {
        action_order: vec![PanelActionId::Close, PanelActionId::Search],
        ..PanelConfig::default()
    };

    let actions = build_panel_actions(&config);

    assert!(child_with_class(&actions.widgets.group, hooks::panel_action::CLOSE).is_some());
}

#[gtk::test]
fn apply_panel_action_config_moves_close_between_header_and_action_group() {
    let header_top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let initial = PanelConfig::default();
    let actions = build_panel_actions(&initial);
    header_top.append(&actions.widgets.close_button);

    let row_config = PanelConfig {
        action_order: vec![
            PanelActionId::Close,
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
            PanelActionId::Search,
        ],
        ..PanelConfig::default()
    };
    apply_panel_action_config(&header_top, &actions.widgets, &row_config);
    assert!(child_with_class(&actions.widgets.group, hooks::panel_action::CLOSE).is_some());
    assert!(child_with_class(&header_top, hooks::panel_action::CLOSE).is_none());

    apply_panel_action_config(&header_top, &actions.widgets, &PanelConfig::default());
    assert!(child_with_class(&actions.widgets.group, hooks::panel_action::CLOSE).is_none());
    assert!(child_with_class(&header_top, hooks::panel_action::CLOSE).is_some());
}

#[gtk::test]
fn dnd_duration_menu_does_not_add_a_standalone_arrow_button() {
    let actions = build_panel_actions(&PanelConfig::default());
    let toggle = actions
        .widgets
        .dnd_group
        .first_child()
        .expect("DND group should contain its toggle");
    let status = toggle
        .next_sibling()
        .expect("DND group should contain its countdown label");

    assert_eq!(toggle, actions.widgets.dnd_toggle);
    assert_eq!(status, actions.widgets.dnd_status);
    assert!(status.next_sibling().is_none());
    assert!(actions
        .widgets
        .dnd_toggle
        .tooltip_text()
        .is_some_and(|text| text.contains("Right-click")));
}
