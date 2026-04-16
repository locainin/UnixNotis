use super::super::parse::collect_geometry_from_contents;
use super::super::GeometryModel;
use unixnotis_core::{Config, MediaLayout};

#[test]
fn warns_when_toggle_grid_outgrows_panel_budget() {
    let mut config = Config::default();
    config.panel.width = 320;
    let css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-toggle { min-width: 104px; padding: 10px 12px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
}

#[test]
fn skips_toggle_warning_when_budget_is_safe() {
    let mut config = Config::default();
    config.panel.width = 520;
    let css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-toggle { min-width: 80px; padding: 6px 8px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(!warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
}

#[test]
fn warns_when_media_row_budget_is_exceeded() {
    let mut config = Config::default();
    config.panel.width = 340;
    let css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-media-nav { min-width: 30px; padding: 6px; border: 1px solid red; }
        .unixnotis-media-card { padding: 12px 16px; border: 1px solid red; }
        .unixnotis-media-art-frame { min-width: 84px; padding: 4px; border: 1px solid red; }
        .unixnotis-media-button { min-width: 34px; padding: 6px 8px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(warnings.iter().any(|warning| warning.contains("media row")));
}

#[test]
fn warns_when_stacked_media_layout_outgrows_panel_budget() {
    let mut config = Config::default();
    config.panel.width = 340;
    config.media.layout = MediaLayout::Stacked;
    let css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-media-card { padding: 12px 16px; border: 1px solid red; }
        .unixnotis-media-main { padding: 0 18px; }
        .unixnotis-media-meta { padding: 0 20px; }
        .unixnotis-media-control-strip { padding: 0 14px; }
        .unixnotis-media-art-frame { min-width: 84px; padding: 4px; border: 1px solid red; }
        .unixnotis-media-button { min-width: 34px; padding: 6px 8px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(warnings.iter().any(|warning| warning.contains("media row")));
}

#[test]
fn warns_when_media_art_outgrows_its_frame() {
    let config = Config::default();
    let css = r#"
        .unixnotis-media-art { min-width: 82px; }
        .unixnotis-media-art-frame { min-width: 54px; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains(".unixnotis-media-art")));
}
