use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::lint_geometry_css_files_with_config;
use super::super::parse::collect_geometry_from_contents;
use super::super::GeometryModel;
use unixnotis_core::{
    Config, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("unixnotis-geometry-{name}-{stamp}-{serial}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, contents).expect("write file");
        path
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn stock_theme_geometry_is_quiet() {
    let config = Config::default();
    let mut model = GeometryModel::default();
    // Stock CSS is the false-positive guard for the whole geometry checker
    // If this test breaks, the warning surface likely got broader than intended
    for css in [
        DEFAULT_BASE_CSS,
        DEFAULT_PANEL_CSS,
        DEFAULT_POPUP_CSS,
        DEFAULT_WIDGETS_CSS,
        DEFAULT_MEDIA_CSS,
    ] {
        let file_warnings = collect_geometry_from_contents(css, &mut model);
        assert!(file_warnings.is_empty(), "{file_warnings:?}");
    }

    let warnings = model.finalize_warnings(&config);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn shipped_default_theme_is_quiet_through_file_based_geometry_path() {
    let root = TempDirGuard::new("stock-default");
    let files = vec![
        root.write("base.css", DEFAULT_BASE_CSS),
        root.write("panel.css", DEFAULT_PANEL_CSS),
        root.write("popup.css", DEFAULT_POPUP_CSS),
        root.write("widgets.css", DEFAULT_WIDGETS_CSS),
        root.write("media.css", DEFAULT_MEDIA_CSS),
    ];

    let diagnostics = lint_geometry_css_files_with_config(
        &files,
        root.path(),
        "$XDG_CONFIG_HOME/unixnotis",
        "$XDG_CONFIG_HOME/unixnotis/config.toml",
        &Config::default(),
    )
    .expect("geometry diagnostics");

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn warns_for_unknown_unixnotis_size_selector() {
    let css = r#"
        .unixnotis-media-image { min-width: 80px; }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unknown UnixNotis class"));
}

#[test]
fn warns_for_complex_unixnotis_size_selector() {
    let css = r#"
        .unixnotis-media-button.primary { min-width: 44px; }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("complex UnixNotis selector"));
}

#[test]
fn warns_for_complex_known_unixnotis_hook_selector() {
    let css = r#"
        .unixnotis-panel-action.unixnotis-panel-action-primary {
            min-width: 96px;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("complex UnixNotis selector"));
}

#[test]
fn warns_for_stateful_unixnotis_size_selector() {
    // Stateful selectors used to stay quiet even though the model could not reason about them
    let css = r#"
        .unixnotis-panel-action:hover {
            min-width: 96px;
            padding: 8px 12px;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("complex UnixNotis selector"));
}

#[test]
fn warns_for_panel_action_width_override_on_unmodeled_hook() {
    let css = r#"
        .unixnotis-panel-action {
            min-width: 120px;
            padding: 8px 12px;
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not model its width yet"));
}

#[test]
fn warns_for_panel_search_width_override_on_known_class() {
    // Known live classes should still warn when they are real width owners but not modeled yet
    let css = r#"
        .unixnotis-panel-search {
            min-width: 240px;
            padding: 0 24px;
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not model its width yet"));
}

#[test]
fn warns_for_panel_count_width_override_on_known_class() {
    let css = r#"
        .unixnotis-panel-count {
            min-width: 120px;
            padding: 2px 20px;
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not model its width yet"));
}

#[test]
fn warns_for_quick_slider_width_override_on_known_class() {
    // Widget-side hooks need the same loud custom behavior as panel and popup hooks
    let css = r#"
        .unixnotis-quick-slider {
            padding: 12px 32px;
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not model its width yet"));
}

#[test]
fn warns_for_popup_icon_width_override_on_known_class() {
    let css = r#"
        .unixnotis-popup-icon {
            min-width: 64px;
            margin-right: 24px;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not model its width yet"));
}

#[test]
fn warns_for_group_row_width_override_on_known_hook() {
    let css = r#"
        .unixnotis-group-row {
            min-width: 280px;
            padding: 8px 20px;
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not model its width yet"));
}

#[test]
fn warns_for_popup_action_width_override_on_unmodeled_hook() {
    let css = r#"
        .unixnotis-popup-action {
            min-width: 140px;
            padding: 6px 12px;
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let warnings = collect_geometry_from_contents(css, &mut model);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not model its width yet"));
}
