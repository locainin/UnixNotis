use gtk::prelude::*;
use unixnotis_core::{PanelRequest, WidgetDensity};

use super::support::{same_widget, state};

#[gtk::test]
fn reloaded_panel_applies_copy_and_widget_density() {
    let mut state = state();
    let mut config = state.config.clone();
    config.panel.title = "Operations".to_string();
    config.panel.subtitle = "Live state".to_string();
    config.widgets.density = WidgetDensity::Compact;

    state.apply_reloaded_panel(&config);

    assert_eq!(state.panel.header.title.text(), "Operations");
    assert_eq!(state.panel.header.subtitle.text(), "Live state");
    assert!(state.panel.header.subtitle.get_visible());
    assert_eq!(state.panel.sections.widget_stack.spacing(), 6);
}

#[gtk::test]
fn reloaded_panel_applies_visibility_placement_and_widget_order_edges() {
    let new_state = state;
    let mut state = new_state();
    let mut config = state.config.clone();
    config.panel.subtitle.clear();
    config.panel.search_visible = false;
    config.panel.action_row_visible = false;
    config.panel.notification_section_visible = true;
    config.panel.recent_notifications_label.clear();
    config.panel.quick_actions_label.clear();
    config.panel.system_status_label = "Resources".to_string();
    config.panel.notification_list_expand = false;
    config.panel.footer_label.clear();
    config.panel.clear_button_placement =
        unixnotis_core::PanelClearButtonPlacement::NotificationHeader;
    config.panel.widget_order = vec![
        unixnotis_core::PanelWidgetSection::Cards,
        unixnotis_core::PanelWidgetSection::Stats,
        unixnotis_core::PanelWidgetSection::Toggles,
        unixnotis_core::PanelWidgetSection::Media,
        unixnotis_core::PanelWidgetSection::Sliders,
    ];
    state.panel.header.actions.search_toggle.set_active(true);

    state.apply_reloaded_panel(&config);

    assert!(!state.panel.header.subtitle.get_visible());
    assert!(state.panel.header.search.revealer.reveals_child());
    assert!(!state.panel.header.action_row.get_visible());
    assert!(!state.panel.sections.notification_header.get_visible());
    assert!(!state.panel.sections.toggle_section_header.get_visible());
    assert_eq!(state.panel.sections.stat_section_header.text(), "Resources");
    assert!(state.panel.sections.stat_section_header.get_visible());
    assert!(state
        .panel
        .sections
        .notification_container
        .has_css_class(unixnotis_core::hooks::panel_shell::RECENT_SECTION));
    assert!(!state.panel.sections.scroller.vexpands());
    assert!(!state.panel.sections.notification_container.vexpands());
    assert!(!state.panel.header.actions.clear_button.get_visible());
    assert!(state.panel.sections.clear_header_button.get_visible());
    assert!(!state.panel.sections.footer.get_visible());

    let first = state
        .panel
        .sections
        .widget_stack
        .first_child()
        .expect("widget stack should keep configured sections");
    assert!(same_widget(&first, &state.panel.sections.card_container));

    let mut hidden_state = new_state();
    hidden_state.apply_reloaded_panel(&config);
    assert!(!hidden_state.panel.header.actions.search_toggle.is_active());
    assert!(!hidden_state.panel.header.search.revealer.reveals_child());
}

#[gtk::test]
fn reload_enables_configured_search_in_toggle_and_revealer() {
    let mut state = state();
    let mut config = state.config.clone();
    assert!(!state.panel.header.actions.search_toggle.is_active());
    assert!(!state.panel.header.search.revealer.reveals_child());

    config.panel.search_visible = true;
    state.apply_reloaded_panel(&config);

    assert!(state.panel.header.actions.search_toggle.is_active());
    assert!(state.panel.header.search.revealer.reveals_child());
}

#[gtk::test]
fn panel_close_and_reopen_keep_transient_search_closed() {
    let mut state = state();
    state.apply_panel_request(PanelRequest::open());
    state.panel.header.actions.search_toggle.set_active(true);
    state.panel.header.search.entry.set_text("urgent");
    assert!(state.panel.header.search.revealer.reveals_child());

    state.apply_panel_request(PanelRequest::close());
    assert!(!state.panel.header.actions.search_toggle.is_active());
    assert!(!state.panel.header.search.revealer.reveals_child());
    assert!(state.panel.header.search.entry.text().is_empty());

    state.apply_panel_request(PanelRequest::open());
    assert!(!state.panel.header.actions.search_toggle.is_active());
    assert!(!state.panel.header.search.revealer.reveals_child());
    state.apply_panel_request(PanelRequest::close());
}
