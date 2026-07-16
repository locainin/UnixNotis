use std::sync::atomic::{AtomicUsize, Ordering};

use gtk::prelude::*;
use unixnotis_core::{
    css::hooks, PanelActionId, PanelClearButtonPlacement, PanelConfig, PanelSection,
};

use super::super::header::build_panel_header;
use super::super::notice::build_reload_notice;
use super::super::sections::build_panel_sections;
use super::super::types::PanelWidgets;
use super::{apply_reloaded_body_order, apply_reloaded_panel_chrome};

static APP_ID: AtomicUsize = AtomicUsize::new(0);

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

fn panel_widgets(config: &PanelConfig) -> PanelWidgets {
    let serial = APP_ID.fetch_add(1, Ordering::Relaxed);
    let app = gtk::Application::builder()
        .application_id(format!("dev.unixnotis.panel.reload.test{serial}"))
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("test application should register");
    let header = build_panel_header(config);
    let sections = build_panel_sections(config, unixnotis_core::WidgetDensity::Comfortable);
    let notice = build_reload_notice();

    PanelWidgets {
        window: gtk::ApplicationWindow::new(&app),
        surface: gtk::Overlay::new(),
        root: gtk::Box::new(gtk::Orientation::Vertical, 0),
        body_stack: sections.body_stack,
        widget_revealer: sections.widget_revealer,
        widget_stack: sections.widget_stack,
        quick_controls: sections.quick_controls,
        toggle_container: sections.toggle_container,
        stat_container: sections.stat_container,
        card_container: sections.card_container,
        scroller: sections.scroller,
        media_container: sections.media_container,
        search_revealer: header.search.revealer,
        search_entry: header.search.entry,
        search_toggle: header.actions.search_toggle,
        header_title: header.title,
        header_subtitle: header.subtitle,
        header_count: header.count,
        header_top: header.top,
        header_action_row: header.action_row,
        header_action_group: header.actions.group,
        notification_container: sections.notification_container,
        notification_header_row: sections.notification_header_row,
        notification_header: sections.notification_header,
        toggle_section_header: sections.toggle_section_header,
        stat_section_header: sections.stat_section_header,
        footer_label: sections.footer,
        focus_toggle: header.actions.focus_toggle,
        dnd_toggle: header.actions.dnd_toggle,
        clear_action_button: header.actions.clear_button,
        clear_header_button: sections.clear_header_button,
        close_button: header.actions.close_button,
        reload_notice_revealer: notice.revealer,
        reload_notice_shell: notice.shell,
        reload_notice_label: notice.label,
    }
}

#[gtk::test]
fn apply_reloaded_panel_chrome_updates_clear_buttons_and_close_placement() {
    let panel = panel_widgets(&PanelConfig::default());
    let config = PanelConfig {
        clear_button_placement: PanelClearButtonPlacement::NotificationHeader,
        action_order: vec![
            PanelActionId::Close,
            PanelActionId::Widgets,
            PanelActionId::Dnd,
            PanelActionId::Clear,
            PanelActionId::Search,
        ],
        ..PanelConfig::default()
    };

    apply_reloaded_panel_chrome(&panel, &config);

    assert!(!panel.clear_action_button.get_visible());
    assert!(panel.clear_header_button.get_visible());
    assert!(child_with_class(&panel.header_top, hooks::panel_action::CLOSE).is_none());
    assert!(child_with_class(&panel.header_action_group, hooks::panel_action::CLOSE).is_some());
}

#[gtk::test]
fn apply_reloaded_body_order_moves_notifications_before_widgets() {
    let panel = panel_widgets(&PanelConfig::default());

    apply_reloaded_body_order(
        &panel,
        &[PanelSection::Notifications, PanelSection::Widgets],
    );

    let first = panel
        .body_stack
        .first_child()
        .expect("body stack should retain both sections");
    assert_eq!(first, panel.notification_container);
    let second = first
        .next_sibling()
        .expect("widget section should follow notifications");
    assert_eq!(second, panel.widget_revealer);
}
