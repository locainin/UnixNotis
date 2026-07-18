use std::path::PathBuf;

use gtk::prelude::*;
use unixnotis_core::{Config, ThemePaths};
use unixnotis_ui::css::CssManager;

use super::super::UiState;

fn theme_paths(root: &str) -> ThemePaths {
    let root = PathBuf::from(root);
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
fn constructor_keeps_config_path_and_starts_with_empty_runtime_collections() {
    let app = gtk::Application::builder()
        .application_id("org.unixnotis.PopupStateTest")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register popup test application");
    let config = Config::default();
    let config_path = PathBuf::from("/tmp/unixnotis-popup-state/config.toml");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let css = CssManager::new_popup(
        theme_paths("/tmp/unixnotis-popup-state"),
        config.theme.clone(),
    );

    let state = UiState::new(&app, config, config_path.clone(), command_tx, css);

    assert_eq!(state.config_path, config_path);
    assert!(state.popups.is_empty());
    assert!(state.popup_order.is_empty());
    assert!(state.visible_popups.is_empty());
}
