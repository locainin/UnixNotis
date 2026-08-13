use std::path::Path;

use unixnotis_core::ThemePaths;

pub(super) fn theme_paths(root: &Path) -> ThemePaths {
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
