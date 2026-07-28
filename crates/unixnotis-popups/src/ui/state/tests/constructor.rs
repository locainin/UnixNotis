use std::path::Path;

use gtk::prelude::*;
use unixnotis_core::{
    hooks, Config, CutCorners, NotificationImage, NotificationView, ThemePaths, Urgency,
};
use unixnotis_ui::{css::CssManager, CutCorner};

use super::super::UiState;

fn theme_paths(root: &Path) -> ThemePaths {
    let root = root.to_path_buf();
    ThemePaths {
        base_dir: root.clone(),
        base_css: root.join("base.css"),
        popup_css: root.join("popup.css"),
        panel_css: root.join("panel.css"),
        widgets_css: root.join("widgets.css"),
        media_css: root.join("media.css"),
    }
}

#[gtk::test]
fn popup_entry_uses_the_configured_cut_corner_primitive() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupCornerTest")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup corner test application");
    let mut config = Config::default();
    config.theme.notification_corners = CutCorners {
        top_left: 20,
        bottom_right: 14,
        ..CutCorners::default()
    };
    let corners = config.theme.notification_corners;
    let config_root = std::env::temp_dir().join("unixnotis-popup-corners");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let notification = NotificationView {
        id: 1,
        generation: 1,
        app_name: "Demo".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Summary".to_string(),
        body: "Body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        is_transient: false,
        image: NotificationImage::default(),
    };

    let entry = state.build_popup_entry(&notification);
    let plate = entry
        .revealer
        .and_then(|revealer| revealer.child())
        .and_downcast::<CutCorner>()
        .expect("popup revealer should contain the cut-corner primitive");
    let root = entry.root.expect("popup entry should keep its styled root");

    assert_eq!(plate.corners(), corners);
    assert_eq!(plate.child().as_ref(), Some(root.upcast_ref()));
}

#[gtk::test]
fn constructor_keeps_config_path_and_starts_with_empty_runtime_collections() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupStateTest")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup test application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-state");
    let config_path = config_root.join("config.toml");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());

    let state = UiState::new(&app, config, config_path.clone(), command_tx, css);

    assert_eq!(state.config_path, config_path);
    assert!(state.popups.is_empty());
    assert!(state.popup_order.is_empty());
    assert!(state.visible_popups.is_empty());
}

#[gtk::test]
fn critical_popup_probe_builds_the_root_class_and_badge() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupCriticalProbe")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup critical probe application");
    let config = Config::default();
    let config_root = std::env::temp_dir().join("unixnotis-popup-critical-probe");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(theme_paths(&config_root), config.theme.clone());
    let mut state = UiState::new(
        &app,
        config,
        config_root.join("config.toml"),
        command_tx,
        css,
    );
    let notification = NotificationView {
        id: 2,
        generation: 2,
        app_name: "Critical probe".to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            display_name: "Critical probe".to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: "Critical popup".to_string(),
        body: "The composed critical state must be visible".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: Urgency::Critical as u8,
        is_transient: false,
        image: NotificationImage::default(),
    };

    let root = state.build_popup_root(&notification);

    assert!(root.has_css_class(hooks::shared_state::CRITICAL));
    assert!(visible_descendant_has_class(
        root.upcast_ref(),
        hooks::urgency::BADGE
    ));
}

fn visible_descendant_has_class(widget: &gtk::Widget, class_name: &str) -> bool {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if current.get_visible() && current.has_css_class(class_name) {
            return true;
        }
        if visible_descendant_has_class(&current, class_name) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}
