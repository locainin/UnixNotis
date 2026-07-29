use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gtk::gdk;
use unixnotis_core::{ThemeConfig, ThemeMode, ThemePaths, THEME_API_VERSION};

use super::super::model::{CssManager, CssManagerInner};
use crate::css::manager::layers::CssProviderLayer;
use crate::css::manager::provider::CssProviderBackend;
use crate::css::manager::report::CssLayerSource;

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
        paths.manifest_path(),
        format!("api_version = {THEME_API_VERSION}\nname = \"Test theme\"\n"),
    )
    .expect("theme manifest");
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the helper transfers the complete path bundle into the manager fixture"
)]
fn panel_manager(
    paths: ThemePaths,
    loaded: Rc<RefCell<Vec<(&'static str, String)>>>,
) -> CssManagerInner<RecordingProvider> {
    let theme_config = ThemeConfig {
        mode: ThemeMode::Custom,
        ..ThemeConfig::default()
    };
    CssManagerInner {
        theme_paths: paths,
        theme_config,
        internal_structure: RecordingProvider::new("internal", Rc::clone(&loaded)),
        base: RecordingProvider::new("base", Rc::clone(&loaded)),
        panel: Some(RecordingProvider::new("panel", Rc::clone(&loaded))),
        widgets: Some(RecordingProvider::new("widgets", Rc::clone(&loaded))),
        media: Some(RecordingProvider::new("media", Rc::clone(&loaded))),
        motion_policy: Some(RecordingProvider::new("motion", Rc::clone(&loaded))),
        popup: None,
    }
}

#[test]
fn stock_mode_ignores_compatible_custom_theme_files() {
    let root = unique_theme_root("stock-mode");
    let paths = theme_paths(&root);
    let loaded = Rc::new(RefCell::new(Vec::new()));
    write_theme(&paths, "magenta");
    let mut manager = panel_manager(paths, Rc::clone(&loaded));
    manager.theme_config.mode = ThemeMode::Stock;

    let report = manager.reload(".fallback { color: red; }");

    assert!(report
        .layers
        .iter()
        .all(|layer| layer.source == CssLayerSource::EmbeddedStock));
    assert!(loaded
        .borrow()
        .iter()
        .all(|(_label, css)| !css.contains("magenta")));
    fs::remove_dir_all(root).expect("remove stock mode test root");
}

#[test]
fn incompatible_theme_uses_embedded_stock_without_reading_custom_css() {
    let root = unique_theme_root("incompatible-theme");
    let paths = theme_paths(&root);
    let loaded = Rc::new(RefCell::new(Vec::new()));
    write_theme(&paths, "magenta");
    fs::write(paths.manifest_path(), "api_version = 1\nname = \"Old\"\n").expect("old manifest");
    let manager = panel_manager(paths, Rc::clone(&loaded));

    let report = manager.reload(".fallback { color: red; }");

    assert!(report
        .layers
        .iter()
        .all(|layer| layer.source == CssLayerSource::EmbeddedStock));
    assert!(loaded
        .borrow()
        .iter()
        .filter(|(label, _)| matches!(*label, "base" | "panel" | "widgets" | "media"))
        .all(|(_, css)| !css.contains("magenta")));
    fs::remove_dir_all(root).expect("remove incompatible theme test root");
}

#[test]
fn panel_reload_loads_base_panel_widgets_and_media_layers() {
    let root = unique_theme_root("reload-panel");
    let paths = theme_paths(&root);
    let loaded = Rc::new(RefCell::new(Vec::new()));
    write_theme(&paths, "green");
    let manager = panel_manager(paths, Rc::clone(&loaded));

    let report = manager.reload(".fallback { color: red; }");

    assert_eq!(
        report
            .layers
            .iter()
            .map(|layer| layer.layer)
            .collect::<Vec<_>>(),
        vec![
            CssProviderLayer::Base,
            CssProviderLayer::Panel,
            CssProviderLayer::Widgets,
            CssProviderLayer::Media,
        ]
    );
    let loaded = loaded.borrow();
    let labels = loaded.iter().map(|(label, _)| *label).collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec!["internal", "base", "panel", "widgets", "media", "motion"]
    );
    assert!(loaded
        .iter()
        .filter(|(label, _)| matches!(*label, "base" | "panel" | "widgets" | "media"))
        .all(|(_, css)| css.contains("green")));
    assert!(loaded
        .iter()
        .find(|(label, _)| *label == "internal")
        .is_some_and(|(_, css)| css.contains(".unixnotis-reload-notice")));
    assert!(loaded
        .iter()
        .find(|(label, _)| *label == "motion")
        .is_some_and(|(_, css)| css.contains(".unixnotis-reduced-motion")));

    fs::remove_dir_all(root).expect("remove css manager test root");
}

#[test]
fn update_theme_changes_the_paths_used_by_the_next_reload() {
    let old_root = unique_theme_root("old-theme");
    let new_root = unique_theme_root("new-theme");
    let old_paths = theme_paths(&old_root);
    let new_paths = theme_paths(&new_root);
    let loaded = Rc::new(RefCell::new(Vec::new()));
    write_theme(&old_paths, "red");
    write_theme(&new_paths, "blue");
    let mut manager = panel_manager(old_paths, Rc::clone(&loaded));

    let theme = ThemeConfig {
        mode: ThemeMode::Custom,
        ..ThemeConfig::default()
    };
    manager.update_theme(new_paths, theme);
    let report = manager.reload(".fallback { color: red; }");

    assert_eq!(report.layers.len(), 4);
    let loaded = loaded.borrow();
    assert!(loaded
        .iter()
        .filter(|(label, _)| matches!(*label, "base" | "panel" | "widgets" | "media"))
        .all(|(_, css)| css.contains("blue")));
    assert!(loaded
        .iter()
        .filter(|(label, _)| matches!(*label, "base" | "panel" | "widgets" | "media"))
        .all(|(_, css)| !css.contains("red")));

    fs::remove_dir_all(old_root).expect("remove old css manager test root");
    fs::remove_dir_all(new_root).expect("remove new css manager test root");
}

#[gtk::test]
fn public_manager_reload_and_theme_update_report_the_applied_stack() {
    let old_root = unique_theme_root("public-old-theme");
    let new_root = unique_theme_root("public-new-theme");
    let old_paths = theme_paths(&old_root);
    let new_paths = theme_paths(&new_root);
    write_theme(&old_paths, "red");
    write_theme(&new_paths, "blue");
    let theme = ThemeConfig {
        mode: ThemeMode::Custom,
        ..ThemeConfig::default()
    };
    let mut manager = CssManager::new_panel(old_paths, theme.clone());

    manager.update_theme(new_paths.clone(), theme);
    let report = manager.reload(".fallback { color: red; }");

    assert_eq!(report.layers.len(), 4);
    assert_eq!(manager.inner.theme_paths.base_css, new_paths.base_css);

    fs::remove_dir_all(old_root).expect("remove public old css manager test root");
    fs::remove_dir_all(new_root).expect("remove public new css manager test root");
}

#[test]
fn reload_report_distinguishes_empty_and_unreadable_theme_files() {
    let root = unique_theme_root("fallback-sources");
    let paths = theme_paths(&root);
    let loaded = Rc::new(RefCell::new(Vec::new()));
    write_theme(&paths, "green");
    fs::write(&paths.panel_css, "  \n").expect("empty panel css");
    fs::remove_file(&paths.media_css).expect("remove media css");
    let manager = panel_manager(paths, loaded);

    let report = manager.reload(".fallback { color: red; }");

    let panel = report
        .layers
        .iter()
        .find(|layer| layer.layer == CssProviderLayer::Panel)
        .expect("panel layer");
    assert_eq!(panel.source, CssLayerSource::EmptyFallback);
    assert!(panel.error.is_none());
    let media = report
        .layers
        .iter()
        .find(|layer| layer.layer == CssProviderLayer::Media)
        .expect("media layer");
    assert_eq!(media.source, CssLayerSource::ReadFailureFallback);
    assert!(media.error.is_some());

    fs::remove_dir_all(root).expect("remove css fallback test root");
}
