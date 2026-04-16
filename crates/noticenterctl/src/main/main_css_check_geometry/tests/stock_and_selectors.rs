use super::super::parse::collect_geometry_from_contents;
use super::super::GeometryModel;
use unixnotis_core::{
    Config, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

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
