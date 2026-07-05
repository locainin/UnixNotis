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
fn stat_grid_warning_uses_configured_column_count() {
    let mut config = Config::default();
    config.panel.width = 360;
    config.widgets.stat_columns = 4;
    while config.widgets.stats.len() < 4 {
        let mut stat = config.widgets.stats[0].clone();
        stat.label = format!("stat {}", config.widgets.stats.len());
        config.widgets.stats.push(stat);
    }
    let css = r#"
        .unixnotis-panel { padding: 12px; }
        .unixnotis-stat-card { min-width: 96px; padding: 8px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(warnings.iter().any(|warning| warning.contains("stat grid")));
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
    config.panel.width = 260;
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
fn custom_media_geometry_is_accounted_for_in_width_budget() {
    let mut config = Config::default();
    config.panel.width = 300;
    config.media.layout = MediaLayout::Inline;
    config.media.art_size_px = 88;
    config.media.content_spacing_px = 16;
    let css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-media-card { padding: 12px 16px; border: 1px solid red; }
        .unixnotis-media-header { padding: 0 12px; }
        .unixnotis-media-body { padding: 0 12px; }
        .unixnotis-media-text { padding: 0 18px; }
        .unixnotis-media-art-frame { min-width: 92px; padding: 4px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(warnings.iter().any(|warning| warning.contains("media row")));
}

#[test]
fn hidden_media_art_and_controls_reduce_width_pressure() {
    let mut config = Config::default();
    config.panel.width = 340;
    config.media.layout = MediaLayout::Showcase;
    config.media.show_art = false;
    config.media.show_controls = false;
    config.media.show_navigation = false;
    let css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-media-card { padding: 10px 12px; border: 1px solid red; }
        .unixnotis-media-header { padding: 0 8px; }
        .unixnotis-media-body { padding: 0 8px; }
        .unixnotis-media-text { padding: 0 10px; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(!warnings.iter().any(|warning| warning.contains("media row")));
}

#[test]
fn player_layout_stays_quiet_with_compact_top_art_budget() {
    let mut config = Config::default();
    config.panel.width = 420;
    config.media.layout = MediaLayout::Player;
    let css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-media-card-player { padding: 8px 10px; border: 1px solid red; }
        .unixnotis-media-header { padding: 0 8px; }
        .unixnotis-media-body { padding: 0 8px; }
        .unixnotis-media-text { padding: 0 10px; }
        .unixnotis-media-art-frame { min-width: 64px; padding: 3px; border: 1px solid red; }
        .unixnotis-media-button { min-width: 33px; padding: 5px 7px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(!warnings.iter().any(|warning| warning.contains("media row")));
}

#[test]
fn compact_player_layout_stays_quiet_on_small_panel_widths() {
    let mut config = Config::default();
    config.panel.width = 320;
    config.media.layout = MediaLayout::Player;
    config.media.art_size_px = 40;
    config.media.text_width_floor_px = 92;
    config.media.card_height_px = Some(156);
    config.media.content_spacing_px = 4;
    config.media.control_spacing_px = 4;
    config.media.navigation_spacing_px = 4;
    let css = r#"
        .unixnotis-panel { padding: 14px; }
        .unixnotis-media-card-player { padding: 6px 8px; border: 1px solid red; }
        .unixnotis-media-header { padding: 0 4px; }
        .unixnotis-media-body { padding: 0 4px; }
        .unixnotis-media-text { padding: 0 6px; }
        .unixnotis-media-art-frame { min-width: 44px; padding: 2px; border: 1px solid red; }
        .unixnotis-media-button { min-width: 28px; padding: 4px 6px; border: 1px solid red; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(!warnings.iter().any(|warning| warning.contains("media row")));
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
