use gtk::prelude::*;
use unixnotis_core::{css::hooks, PanelActionId, PanelConfig};

use super::build_panel_header;

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

#[gtk::test]
fn build_panel_header_keeps_default_close_in_header_top() {
    let header = build_panel_header(&PanelConfig::default());

    assert!(child_with_class(&header.top, hooks::panel_action::CLOSE).is_some());
    assert!(child_with_class(&header.actions.group, hooks::panel_action::CLOSE).is_none());
}

#[gtk::test]
fn build_panel_header_places_explicit_close_inside_action_group() {
    let config = PanelConfig {
        action_order: vec![
            PanelActionId::Close,
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
            PanelActionId::Search,
        ],
        ..PanelConfig::default()
    };

    let header = build_panel_header(&config);

    assert!(child_with_class(&header.top, hooks::panel_action::CLOSE).is_none());
    assert!(child_with_class(&header.actions.group, hooks::panel_action::CLOSE).is_some());
}

#[gtk::test]
fn visible_search_configuration_activates_toggle_and_revealer_together() {
    let config = PanelConfig {
        search_visible: true,
        ..PanelConfig::default()
    };

    let header = build_panel_header(&config);

    assert!(header.actions.search_toggle.is_active());
    assert!(header.search.revealer.reveals_child());
}
