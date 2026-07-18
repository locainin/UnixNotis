use std::path::PathBuf;

use unixnotis_core::{ThemeConfig, ThemePaths};

use super::super::CssManager;

fn paths(root: &str) -> ThemePaths {
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
fn surface_constructors_keep_the_requested_theme_paths() {
    let panel_paths = paths("/tmp/unixnotis-panel-model");
    let popup_paths = paths("/tmp/unixnotis-popup-model");

    let panel = CssManager::new_panel(panel_paths.clone(), ThemeConfig::default());
    let popup = CssManager::new_popup(popup_paths.clone(), ThemeConfig::default());

    assert_eq!(panel.theme_paths().base_dir, panel_paths.base_dir);
    assert_eq!(popup.theme_paths().base_dir, popup_paths.base_dir);
}
