use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gtk::gdk;
use unixnotis_core::{ThemeConfig, ThemePaths};

use super::*;

#[derive(Clone)]
struct RecordingProvider {
    label: &'static str,
    loaded: Rc<RefCell<Vec<(&'static str, String)>>>,
}

impl RecordingProvider {
    fn new(label: &'static str, loaded: Rc<RefCell<Vec<(&'static str, String)>>>) -> Self {
        Self { label, loaded }
    }
}

impl CssProviderBackend for RecordingProvider {
    fn load_css_data(&self, data: &str) {
        // Store the final CSS bytes after defaults, overrides, and URL rebasing are applied
        self.loaded
            .borrow_mut()
            .push((self.label, data.to_string()));
    }

    fn add_to_display(&self, _display: &gdk::Display, _priority: u32) {}
}

fn unique_theme_root(label: &str) -> PathBuf {
    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    // Process-local uniqueness is enough because these tests clean up their own directories
    let unique = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-ui-css-manager-{pid}-{label}-{unique}",
        pid = std::process::id(),
    ));
    fs::create_dir_all(&root).expect("create css manager test root");
    root
}

fn theme_paths(root: &Path) -> ThemePaths {
    ThemePaths {
        base_dir: root.to_path_buf(),
        base_css: root.join("base.css"),
        popup_css: root.join("popup.css"),
        panel_css: root.join("panel.css"),
        widgets_css: root.join("widgets.css"),
        media_css: root.join("media.css"),
        overrides_css: root.join("overrides.css"),
    }
}

fn write_theme(paths: &ThemePaths, marker: &str) {
    fs::write(&paths.base_css, format!(".base {{ color: {marker}; }}")).expect("base css");
    fs::write(&paths.panel_css, format!(".panel {{ color: {marker}; }}")).expect("panel css");
    fs::write(
        &paths.widgets_css,
        format!(".widgets {{ color: {marker}; }}"),
    )
    .expect("widgets css");
    fs::write(&paths.media_css, format!(".media {{ color: {marker}; }}")).expect("media css");
    fs::write(&paths.popup_css, format!(".popup {{ color: {marker}; }}")).expect("popup css");
    fs::write(
        &paths.overrides_css,
        format!(".override {{ color: {marker}; }}"),
    )
    .expect("overrides css");
}

fn panel_manager(
    paths: ThemePaths,
    loaded: Rc<RefCell<Vec<(&'static str, String)>>>,
) -> CssManagerInner<RecordingProvider> {
    CssManagerInner {
        theme_paths: paths,
        theme_config: ThemeConfig::default(),
        base: RecordingProvider::new("base", Rc::clone(&loaded)),
        panel: Some(RecordingProvider::new("panel", Rc::clone(&loaded))),
        widgets: Some(RecordingProvider::new("widgets", Rc::clone(&loaded))),
        media: Some(RecordingProvider::new("media", Rc::clone(&loaded))),
        popup: None,
        overrides: RecordingProvider::new("overrides", Rc::clone(&loaded)),
    }
}

#[test]
fn panel_reload_loads_base_panel_widgets_and_media_layers() {
    let root = unique_theme_root("reload-panel");
    let paths = theme_paths(&root);
    let loaded = Rc::new(RefCell::new(Vec::new()));
    write_theme(&paths, "green");
    let manager = panel_manager(paths, Rc::clone(&loaded));

    let layers = manager.reload(".fallback { color: red; }");

    assert_eq!(
        layers,
        vec![
            CssProviderLayer::Base,
            CssProviderLayer::Panel,
            CssProviderLayer::Widgets,
            CssProviderLayer::Media,
            CssProviderLayer::Overrides,
        ]
    );
    let loaded = loaded.borrow();
    let labels = loaded.iter().map(|(label, _)| *label).collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec!["base", "panel", "widgets", "media", "overrides"]
    );
    assert!(loaded.iter().all(|(_, css)| css.contains("green")));

    fs::remove_dir_all(root).expect("remove css manager test root");
}

#[test]
fn update_theme_changes_the_paths_used_by_the_next_reload() {
    let old_root = unique_theme_root("old-theme");
    let new_root = unique_theme_root("new-theme");
    let old_paths = theme_paths(&old_root);
    let new_paths = theme_paths(&new_root);
    let loaded = Rc::new(RefCell::new(Vec::new()));
    write_theme(&old_paths, "oldcolor");
    write_theme(&new_paths, "newcolor");
    let mut manager = panel_manager(old_paths, Rc::clone(&loaded));

    manager.update_theme(new_paths, ThemeConfig::default());
    let layers = manager.reload(".fallback { color: red; }");

    assert_eq!(layers.len(), 5);
    let loaded = loaded.borrow();
    assert!(loaded.iter().all(|(_, css)| css.contains("newcolor")));
    assert!(loaded.iter().all(|(_, css)| !css.contains("oldcolor")));

    fs::remove_dir_all(old_root).expect("remove old css manager test root");
    fs::remove_dir_all(new_root).expect("remove new css manager test root");
}
