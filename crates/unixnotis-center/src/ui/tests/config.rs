use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{fs, path::Path};

use gtk::prelude::*;
use unixnotis_core::{Config, EmptyStateAlignment, Margins, ToggleWidgetConfig, WidgetDensity};
use unixnotis_ui::css::CssManager;

use super::{UiState, UiStateInit};
use crate::dbus::{UiCommand, UiEvent};

static APP_ID: AtomicUsize = AtomicUsize::new(0);

fn state() -> UiState {
    let serial = APP_ID.fetch_add(1, Ordering::Relaxed);
    let app = gtk::Application::builder()
        .application_id(format!("dev.unixnotis.config.reload.test{serial}"))
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("test application should register");

    let mut config = Config::default();
    // External widget processes are irrelevant to configuration application tests
    config.media.enabled = false;
    config.widgets.volume.enabled = false;
    config.widgets.brightness.enabled = false;
    config.widgets.toggles.clear();
    config.widgets.stats.clear();
    config.widgets.cards.clear();

    let config_dir = std::env::temp_dir().join(format!(
        "unixnotis-config-reload-test-{}",
        std::process::id()
    ));
    let config_path = config_dir.join("config.toml");
    fs::create_dir_all(&config_dir).expect("test config directory should exist");
    let theme_paths = config
        .resolve_theme_paths_from(&config_dir)
        .expect("test theme paths should resolve");
    let css = CssManager::new_panel(theme_paths, config.theme.clone());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel::<UiCommand>(8);
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(8);
    let runtime = Arc::new(tokio::runtime::Runtime::new().expect("test runtime should build"));

    UiState::new(UiStateInit {
        app,
        config,
        config_path,
        command_tx,
        css,
        event_tx,
        media_handle: None,
        runtime,
    })
}

fn same_widget<W: IsA<gtk::Widget>>(left: &gtk::Widget, right: &W) -> bool {
    left == right.as_ref()
}

fn write_config(path: &Path, config: &Config) {
    let text = toml::to_string(config).expect("test config should serialize");
    fs::write(path, text).expect("test config should be written");
}

#[gtk::test]
fn reloaded_panel_applies_copy_and_widget_density() {
    let mut state = state();
    let mut config = state.config.clone();
    config.panel.title = "Operations".to_string();
    config.panel.subtitle = "Live state".to_string();
    config.widgets.density = WidgetDensity::Compact;

    state.apply_reloaded_panel(&config);

    assert_eq!(state.panel.header_title.text(), "Operations");
    assert_eq!(state.panel.header_subtitle.text(), "Live state");
    assert!(state.panel.header_subtitle.get_visible());
    assert_eq!(state.panel.widget_stack.spacing(), 6);
}

#[gtk::test]
fn reloaded_list_applies_explicit_empty_alignment() {
    let mut state = state();
    let mut config = state.config.clone();
    config.panel.empty_text = "Nothing pending".to_string();
    config.panel.empty_alignment = EmptyStateAlignment::End;
    config.panel.empty_offset_top = 44;

    state.apply_list_config_after_reload(&config);

    assert_eq!(state.list.empty_text, "Nothing pending");
    assert_eq!(state.list.empty_overlay.valign(), gtk::Align::End);
    assert_eq!(state.list.empty_overlay.margin_top(), 0);
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
    state.panel.search_toggle.set_active(true);

    state.apply_reloaded_panel(&config);

    assert!(!state.panel.header_subtitle.get_visible());
    assert!(state.panel.search_revealer.reveals_child());
    assert!(!state.panel.header_action_row.get_visible());
    assert!(!state.panel.notification_header.get_visible());
    assert!(!state.panel.toggle_section_header.get_visible());
    assert_eq!(state.panel.stat_section_header.text(), "Resources");
    assert!(state.panel.stat_section_header.get_visible());
    assert!(state
        .panel
        .notification_container
        .has_css_class(unixnotis_core::hooks::panel_shell::RECENT_SECTION));
    assert!(!state.panel.scroller.vexpands());
    assert!(!state.panel.notification_container.vexpands());
    assert!(!state.panel.clear_action_button.get_visible());
    assert!(state.panel.clear_header_button.get_visible());
    assert!(!state.panel.footer_label.get_visible());

    let first = state
        .panel
        .widget_stack
        .first_child()
        .expect("widget stack should keep configured sections");
    assert!(same_widget(&first, &state.panel.card_container));

    let mut hidden_state = new_state();
    hidden_state.apply_reloaded_panel(&config);
    assert!(!hidden_state.panel.search_toggle.is_active());
    assert!(!hidden_state.panel.search_revealer.reveals_child());
}

#[gtk::test]
fn reload_config_applies_valid_file_and_rejects_malformed_replacement() {
    let mut state = state();
    let mut reloaded = state.config.clone();
    reloaded.panel.title = "Reloaded from disk".to_string();
    reloaded.panel.footer_label = "Ready".to_string();
    reloaded.panel.empty_alignment = EmptyStateAlignment::Auto;
    reloaded.panel.empty_offset_top = 44;
    reloaded.theme.base_css = "reloaded-base.css".to_string();
    reloaded.widgets.toggles = vec![ToggleWidgetConfig {
        enabled: true,
        kind: Some("test-toggle".to_string()),
        label: "Test Toggle".to_string(),
        ..ToggleWidgetConfig::default()
    }];
    write_config(&state.config_path, &reloaded);
    state.work_area = Some(Margins {
        top: 1,
        right: 2,
        bottom: 3,
        left: 4,
    });

    state.reload_config();

    assert_eq!(state.config.panel.title, "Reloaded from disk");
    assert_eq!(state.panel.header_title.text(), "Reloaded from disk");
    assert_eq!(state.panel.footer_label.text(), "Ready");
    assert!(state.panel.footer_label.get_visible());
    assert!(state.toggles.is_some());
    assert!(state.panel.toggle_container.first_child().is_some());
    assert_eq!(state.list.empty_overlay.valign(), gtk::Align::Start);
    assert_eq!(state.list.empty_overlay.margin_top(), 44);
    assert!(state.work_area.is_none());
    assert_eq!(
        state.css.theme_paths().base_css,
        state
            .config_path
            .parent()
            .expect("config path should have a parent")
            .join("reloaded-base.css")
    );

    state.widgets_collapsed = true;
    state.apply_list_config_after_reload(&reloaded);
    assert_eq!(state.list.empty_overlay.valign(), gtk::Align::Center);
    assert_eq!(state.list.empty_overlay.margin_top(), 0);

    fs::write(&state.config_path, "[panel\ntitle = broken")
        .expect("malformed config should be written");
    state.reload_config();
    assert_eq!(state.config.panel.title, "Reloaded from disk");
    assert_eq!(state.panel.header_title.text(), "Reloaded from disk");
}
