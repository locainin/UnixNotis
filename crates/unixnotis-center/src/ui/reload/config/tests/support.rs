use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gtk::prelude::*;
use unixnotis_core::Config;
use unixnotis_ui::css::CssManager;

use crate::control::{UiCommand, UiEvent};
use crate::ui::{UiState, UiStateInit};

static APP_ID: AtomicUsize = AtomicUsize::new(0);

pub(super) fn state() -> UiState {
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

pub(super) fn same_widget<W: IsA<gtk::Widget>>(left: &gtk::Widget, right: &W) -> bool {
    left == right.as_ref()
}

pub(super) fn write_config(path: &Path, config: &Config) {
    let text = toml::to_string(config).expect("test config should serialize");
    fs::write(path, text).expect("test config should be written");
}
