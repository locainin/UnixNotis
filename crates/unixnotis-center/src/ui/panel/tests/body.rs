use gtk::prelude::*;
use unixnotis_core::{hooks, WidgetDensity};
use unixnotis_core::{PanelClearButtonPlacement, PanelConfig, PanelSection};

use super::{
    apply_panel_body_section_order, build_panel_sections, notification_header_row_visible,
};

#[test]
fn notification_header_row_stays_visible_for_header_clear_button() {
    let mut config = PanelConfig {
        notification_section_visible: false,
        clear_button_placement: PanelClearButtonPlacement::NotificationHeader,
        ..PanelConfig::default()
    };
    assert!(notification_header_row_visible(&config));

    config.clear_button_placement = PanelClearButtonPlacement::ActionRow;
    assert!(!notification_header_row_visible(&config));
}

#[gtk::test]
fn compact_widget_density_updates_spacing_and_state_class() {
    let sections = build_panel_sections(&PanelConfig::default(), WidgetDensity::Compact);

    assert_eq!(sections.widget_stack.spacing(), 6);
    assert_eq!(sections.quick_controls.spacing(), 6);
    assert_eq!(sections.media_container.spacing(), 6);
    assert!(sections
        .widget_stack
        .has_css_class(hooks::panel_shell::WIDGET_DENSITY_COMPACT));
    assert!(!sections
        .widget_stack
        .has_css_class(hooks::panel_shell::WIDGET_DENSITY_COMFORTABLE));
}

#[test]
fn notification_header_row_uses_section_label_when_section_is_visible() {
    let config = PanelConfig {
        notification_section_visible: true,
        recent_notifications_label: "Recent".to_string(),
        clear_button_placement: PanelClearButtonPlacement::Hidden,
        ..PanelConfig::default()
    };

    assert!(notification_header_row_visible(&config));
}

#[gtk::test]
fn build_panel_sections_can_place_notifications_before_widgets() {
    let config = PanelConfig {
        section_order: vec![PanelSection::Notifications, PanelSection::Widgets],
        ..PanelConfig::default()
    };

    let sections = build_panel_sections(&config, unixnotis_core::WidgetDensity::Comfortable);
    let first = sections
        .body_stack
        .first_child()
        .expect("body stack should contain notification section");
    let expected: gtk::Widget = sections.notification_container.upcast();

    assert_eq!(first, expected);
}

#[gtk::test]
fn apply_panel_body_section_order_can_move_notifications_before_widgets() {
    let config = PanelConfig::default();
    let sections = build_panel_sections(&config, unixnotis_core::WidgetDensity::Comfortable);

    apply_panel_body_section_order(
        &sections.body_stack,
        &sections.widget_revealer,
        &sections.notification_container,
        &[PanelSection::Notifications, PanelSection::Widgets],
    );
    let first = sections
        .body_stack
        .first_child()
        .expect("body stack should contain notification section");
    let expected: gtk::Widget = sections.notification_container.upcast();

    assert_eq!(first, expected);
}
