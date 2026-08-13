//! Media configuration reload tests

use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gtk::prelude::*;
use unixnotis_core::{hooks, Config, MediaLayout};
use unixnotis_ui::css::CssManager;

use crate::control::{UiCommand, UiEvent};
use crate::media::{MediaCommand, MediaHandle, MediaInfo};
use crate::ui::{UiState, UiStateInit};

static APP_ID: AtomicUsize = AtomicUsize::new(0);

fn media_state() -> UiState {
    let serial = APP_ID.fetch_add(1, Ordering::Relaxed);
    let app = gtk::Application::builder()
        .application_id(format!("dev.unixnotis.media.config.test{serial}"))
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("test application should register");

    let mut config = Config::default();
    // Unrelated widgets stay disabled so this test owns only the media subtree
    config.widgets.volume.enabled = false;
    config.widgets.brightness.enabled = false;
    config.widgets.toggles.clear();
    config.widgets.stats.clear();
    config.widgets.cards.clear();

    let config_dir = std::env::temp_dir().join(format!(
        "unixnotis-media-config-test-{}-{serial}",
        std::process::id(),
    ));
    fs::create_dir_all(&config_dir).expect("test config directory should exist");
    let config_path = config_dir.join("config.toml");
    let theme_paths = config
        .resolve_theme_paths_from(&config_dir)
        .expect("test theme paths should resolve");
    let css = CssManager::new_panel(theme_paths, config.theme.clone());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel::<UiCommand>(8);
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(8);
    let runtime = Arc::new(tokio::runtime::Runtime::new().expect("test runtime should build"));
    let (media_tx, _media_rx) = tokio::sync::mpsc::channel::<MediaCommand>(8);
    let media_handle = MediaHandle::connected(media_tx, runtime.handle().clone());

    UiState::new(UiStateInit {
        app,
        config,
        config_path,
        command_tx,
        css,
        event_tx,
        media_handle: Some(media_handle),
        runtime,
    })
}

fn sample_media(title: &str) -> MediaInfo {
    MediaInfo {
        bus_name: "org.mpris.MediaPlayer2.test".to_string(),
        identity: "Test Player".to_string(),
        browser_family: None,
        owner_pid: None,
        source_pid_hint: None,
        title: title.to_string(),
        artist: "Artist".to_string(),
        playback_status: "Playing".to_string(),
        art_source: None,
        can_play: true,
        can_pause: true,
        can_next: true,
        can_prev: true,
    }
}

fn find_label_with_class(root: &gtk::Widget, class_name: &str) -> Option<gtk::Label> {
    if root.has_css_class(class_name) {
        return root.clone().downcast::<gtk::Label>().ok();
    }

    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(label) = find_label_with_class(&widget, class_name) {
            return Some(label);
        }
        child = widget.next_sibling();
    }
    None
}

#[gtk::test]
fn structural_media_reload_replaces_the_existing_shell() {
    let mut state = media_state();
    let original = state
        .panel
        .sections
        .media_container
        .first_child()
        .expect("initial media shell should exist");
    let mut config = state.config.clone();
    config.media.layout = MediaLayout::Inline;

    state.apply_media_config(&config);

    let replacement = state
        .panel
        .sections
        .media_container
        .first_child()
        .expect("replacement media shell should exist");
    assert_ne!(original, replacement);
    assert!(state
        .media
        .as_ref()
        .expect("media widget should remain active")
        .matches_layout(&config.media));
}

#[gtk::test]
fn light_media_reload_updates_limits_and_reduced_motion_without_rebuilding() {
    const LONG_TITLE: &str = "A title that must overflow the configured four character lane";

    let mut state = media_state();
    state
        .media
        .as_mut()
        .expect("initial media widget should exist")
        .update(&[sample_media(LONG_TITLE)]);
    let original = state
        .panel
        .sections
        .media_container
        .first_child()
        .expect("initial media shell should exist");
    let mut config = state.config.clone();
    config.media.title_char_limit = 4;
    config.panel.reduced_motion = true;

    state.apply_media_config(&config);

    let retained = state
        .panel
        .sections
        .media_container
        .first_child()
        .expect("media shell should remain attached");
    assert_eq!(original, retained);
    let title = find_label_with_class(retained.as_ref(), hooks::media_shell::TITLE)
        .expect("media title label should exist");
    assert_eq!(title.width_chars(), 4);
    assert_eq!(title.max_width_chars(), 4);
    assert_eq!(title.text(), LONG_TITLE);
}
