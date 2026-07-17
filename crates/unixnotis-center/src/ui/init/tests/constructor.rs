use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gtk::prelude::*;
use unixnotis_core::Config;
use unixnotis_ui::css::CssManager;

use super::super::{UiState, UiStateInit};
use crate::control::{UiCommand, UiEvent};

#[gtk::test]
fn constructor_builds_disabled_optional_sections_without_reserving_space() {
    static NEXT_APP: AtomicUsize = AtomicUsize::new(0);

    let serial = NEXT_APP.fetch_add(1, Ordering::Relaxed);
    let app = gtk::Application::builder()
        .application_id(format!("dev.unixnotis.init.constructor{serial}"))
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register test application");

    let mut config = Config::default();
    config.media.enabled = false;
    config.widgets.volume.enabled = false;
    config.widgets.brightness.enabled = false;
    config.widgets.toggles.clear();
    config.widgets.stats.clear();
    config.widgets.cards.clear();

    let root = std::env::temp_dir().join(format!(
        "unixnotis-init-constructor-{}-{serial}",
        std::process::id(),
    ));
    fs::create_dir_all(&root).expect("create constructor test directory");
    let config_path = root.join("config.toml");
    let theme_paths = config
        .resolve_theme_paths_from(&root)
        .expect("resolve constructor theme paths");
    let css = CssManager::new_panel(theme_paths, config.theme.clone());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel::<UiCommand>(4);
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(4);
    let runtime = Arc::new(tokio::runtime::Runtime::new().expect("build test runtime"));

    let state = UiState::new(UiStateInit {
        app,
        config,
        config_path,
        command_tx,
        css,
        event_tx,
        media_handle: None,
        runtime,
    });

    assert!(state.media.is_none());
    assert!(!state.panel.media_container.get_visible());
    assert!(state.volume.is_none());
    assert!(state.brightness.is_none());
    assert!(state.toggles.is_none());
    assert!(state.stats.is_none());
    assert!(state.cards.is_none());
    fs::remove_dir_all(root).expect("remove constructor test directory");
}
