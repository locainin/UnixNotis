use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::{fs, path::Path};

use gtk::prelude::*;
use unixnotis_core::{
    Config, ConfigError, EmptyStateAlignment, Margins, ToggleWidgetConfig, WidgetDensity,
};
use unixnotis_ui::css::CssManager;

use super::super::super::{UiState, UiStateInit};
use super::{log_reload_rejection, ConfigReloadOutcome, ReloadFailure};
use crate::control::{UiCommand, UiEvent};

static APP_ID: AtomicUsize = AtomicUsize::new(0);

struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_poisoned| io::Error::other("captured log lock poisoned"))?
            .write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn reload_failure_kinds_remain_stable_for_structured_logs() {
    assert_eq!(
        ReloadFailure::Config(ConfigError::MissingHome).kind(),
        "config"
    );
    assert_eq!(
        ReloadFailure::ThemeBase("missing".to_string()).kind(),
        "theme-base"
    );
    assert_eq!(
        ReloadFailure::ThemePaths("invalid".to_string()).kind(),
        "theme-paths"
    );
}

#[test]
fn rejected_config_logs_never_include_private_parser_text() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let writer_output = Arc::clone(&output);
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || CapturedWriter(Arc::clone(&writer_output)))
        .finish();
    let failure = ReloadFailure::Config(ConfigError::ParseFailed(
        "private-center-parser-sentinel".to_string(),
    ));

    tracing::subscriber::with_default(subscriber, || log_reload_rejection(&failure));

    let rendered = String::from_utf8(output.lock().expect("lock captured center output").clone())
        .expect("center output should be UTF-8");
    assert!(rendered.contains("kind=\"config\""));
    assert!(rendered.contains("fingerprint="));
    assert!(!rendered.contains("private-center-parser-sentinel"));
}

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
        "unixnotis-config-reload-test-{}-{serial}",
        std::process::id(),
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

    let outcome = state.reload_config();

    assert!(matches!(outcome, ConfigReloadOutcome::Applied { .. }));

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
    let outcome = state.reload_config();
    assert!(matches!(outcome, ConfigReloadOutcome::Rejected { .. }));
    assert_eq!(state.config.panel.title, "Reloaded from disk");
    assert_eq!(state.panel.header_title.text(), "Reloaded from disk");
    assert!(state.panel.reload_notice_revealer.reveals_child());
    assert!(state
        .panel
        .reload_notice_label
        .text()
        .contains("previous configuration is still active"));
    assert!(!state
        .panel
        .reload_notice_label
        .text()
        .contains("title = broken"));
}

#[gtk::test]
fn accepted_reload_clears_rejected_config_notice() {
    let mut state = state();
    fs::write(&state.config_path, "[panel\ntitle = broken").expect("broken config");
    let _outcome = state.reload_config();
    assert!(state.panel.reload_notice_revealer.reveals_child());

    let valid = state.config.clone();
    write_config(&state.config_path, &valid);
    let theme_paths = valid
        .resolve_theme_paths_from(state.config_path.parent().expect("config parent"))
        .expect("theme paths");
    for path in [
        theme_paths.base_css,
        theme_paths.panel_css,
        theme_paths.widgets_css,
        theme_paths.media_css,
    ] {
        fs::write(path, "/* intentionally valid */").expect("theme css");
    }

    let outcome = state.reload_config();

    assert!(matches!(outcome, ConfigReloadOutcome::Applied { .. }));
    assert!(!state.panel.reload_notice_revealer.reveals_child());
}

#[gtk::test]
fn dismissed_reload_notice_stays_hidden_until_failure_fingerprint_changes() {
    let mut state = state();
    fs::write(&state.config_path, "[panel\ntitle = first").expect("first broken config");
    let _outcome = state.reload_config();
    assert!(state.panel.reload_notice_revealer.reveals_child());

    let close = state
        .panel
        .reload_notice_shell
        .last_child()
        .expect("reload notice close button")
        .downcast::<gtk::Button>()
        .expect("reload notice close widget");
    close.emit_clicked();
    assert!(!state.panel.reload_notice_revealer.reveals_child());

    let _same_outcome = state.reload_config();
    assert!(!state.panel.reload_notice_revealer.reveals_child());

    fs::write(&state.config_path, "config_version = 999").expect("distinct broken config");
    let _distinct_outcome = state.reload_config();
    assert!(state.panel.reload_notice_revealer.reveals_child());
}

#[gtk::test]
fn successful_css_only_reload_does_not_clear_config_rejection_notice() {
    let mut state = state();
    let theme_paths = state
        .config
        .resolve_theme_paths_from(state.config_path.parent().expect("config parent"))
        .expect("theme paths");
    for path in [
        theme_paths.base_css,
        theme_paths.panel_css,
        theme_paths.widgets_css,
        theme_paths.media_css,
    ] {
        fs::write(path, "/* valid reload css */").expect("theme css");
    }
    fs::write(&state.config_path, "[panel\ntitle = broken").expect("broken config");
    let _outcome = state.reload_config();
    let rejection = state.panel.reload_notice_label.text();

    let report = state.reload_css();

    assert_eq!(report.read_failures().count(), 0);
    assert!(state.panel.reload_notice_revealer.reveals_child());
    assert_eq!(state.panel.reload_notice_label.text(), rejection);
}

#[gtk::test]
fn css_failure_cannot_replace_an_active_config_rejection() {
    let mut state = state();
    fs::write(&state.config_path, "[panel\ntitle = broken").expect("broken config");
    let _outcome = state.reload_config();
    let rejection = state.panel.reload_notice_label.text();

    let report = state.reload_css();

    assert!(report.read_failures().count() > 0);
    assert!(state.panel.reload_notice_revealer.reveals_child());
    assert_eq!(state.panel.reload_notice_label.text(), rejection);
    assert!(state
        .panel
        .reload_notice_shell
        .has_css_class(unixnotis_core::css::hooks::panel_shell::RELOAD_NOTICE_ERROR));
}

#[gtk::test]
fn css_reload_notice_summarizes_multiple_unreadable_layers() {
    let mut state = state();
    let report = state.reload_css();

    assert!(report.read_failures().count() > 1);
    assert!(state.panel.reload_notice_revealer.reveals_child());
    assert!(state
        .panel
        .reload_notice_label
        .text()
        .contains("other layer"));
    assert!(state
        .panel
        .reload_notice_shell
        .has_css_class(unixnotis_core::css::hooks::panel_shell::RELOAD_NOTICE_WARNING));
}
