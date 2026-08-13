use std::sync::atomic::{AtomicUsize, Ordering};

use gtk::prelude::*;
use unixnotis_core::{
    css::hooks, PanelActionId, PanelClearButtonPlacement, PanelConfig, PanelSection,
};

use super::super::body::build_panel_sections;
use super::super::header::build_panel_header;
use super::super::notice::build_reload_notice;
use super::super::widgets::PanelWidgets;
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
        header,
        sections,
        reload_notice: notice,
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

    assert!(!panel.header.actions.clear_button.get_visible());
    assert!(panel.sections.clear_header_button.get_visible());
    assert!(child_with_class(&panel.header.top, hooks::panel_action::CLOSE).is_none());
    assert!(child_with_class(&panel.header.actions.group, hooks::panel_action::CLOSE).is_some());
}

#[gtk::test]
fn apply_reloaded_body_order_moves_notifications_before_widgets() {
    let panel = panel_widgets(&PanelConfig::default());

    apply_reloaded_body_order(
        &panel,
        &[PanelSection::Notifications, PanelSection::Widgets],
    );

    let first = panel
        .sections
        .body_stack
        .first_child()
        .expect("body stack should retain both sections");
    assert_eq!(first, panel.sections.notification_container);
    let second = first
        .next_sibling()
        .expect("widget section should follow notifications");
    assert_eq!(second, panel.sections.widget_revealer);
}
