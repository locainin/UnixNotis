use super::super::parse::collect_geometry_from_contents;
use super::super::stock::baselines::stock_geometry_model;
use super::super::GeometryModel;
use unixnotis_core::{
    Config, MediaArtPosition, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
};

fn media_height_warning<'a>(warnings: &'a [String]) -> Option<&'a String> {
    warnings
        .iter()
        .find(|warning| warning.contains("media card shell"))
}

#[test]
fn warns_when_media_card_height_is_too_small_for_tall_top_art_shell() {
    let mut config = Config::default();
    config.media.layout = MediaLayout::Showcase;
    config.media.art_position = MediaArtPosition::Top;
    config.media.controls_position = MediaControlsPosition::Bottom;
    config.media.navigation_position = MediaNavigationPosition::WithControls;
    config.media.card_height_px = Some(126);
    config.media.art_size_px = 96;

    let css = r#"
        .unixnotis-media-card { padding: 12px 16px; border: 1px solid red; }
        .unixnotis-media-header { padding: 6px 0; }
        .unixnotis-media-main { padding: 4px 0; }
        .unixnotis-media-text { padding: 4px 0; }
        .unixnotis-media-meta { min-height: 18px; }
        .unixnotis-media-title { min-height: 24px; }
        .unixnotis-media-artist { min-height: 18px; }
        .unixnotis-media-art-frame { min-height: 146px; padding: 4px 0; border: 1px solid red; }
        .unixnotis-media-control-strip { padding: 8px 0; }
        .unixnotis-media-controls { min-height: 40px; }
        .unixnotis-media-nav-strip { min-height: 40px; }
        .unixnotis-media-button { min-height: 40px; }
        .unixnotis-media-nav { min-height: 40px; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    let warning = media_height_warning(&warnings)
        .expect("expected media height warning for undersized top-art shell");
    assert!(warning.contains("126px"));
}

#[test]
fn skips_media_height_warning_when_card_budget_matches_shell_height() {
    let mut config = Config::default();
    config.media.layout = MediaLayout::Showcase;
    config.media.art_position = MediaArtPosition::Top;
    config.media.controls_position = MediaControlsPosition::Bottom;
    config.media.navigation_position = MediaNavigationPosition::WithControls;
    config.media.card_height_px = Some(340);
    config.media.art_size_px = 96;

    let css = r#"
        .unixnotis-media-card { padding: 12px 16px; border: 1px solid red; }
        .unixnotis-media-header { padding: 6px 0; }
        .unixnotis-media-main { padding: 4px 0; }
        .unixnotis-media-text { padding: 4px 0; }
        .unixnotis-media-meta { min-height: 18px; }
        .unixnotis-media-title { min-height: 24px; }
        .unixnotis-media-artist { min-height: 18px; }
        .unixnotis-media-art-frame { min-height: 146px; padding: 4px 0; border: 1px solid red; }
        .unixnotis-media-control-strip { padding: 8px 0; }
        .unixnotis-media-controls { min-height: 40px; }
        .unixnotis-media-nav-strip { min-height: 40px; }
        .unixnotis-media-button { min-height: 40px; }
        .unixnotis-media-nav { min-height: 40px; }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());

    let warnings = model.finalize_warnings(&config);
    assert!(media_height_warning(&warnings).is_none());
}

#[test]
fn stock_theme_media_height_baseline_stays_quiet() {
    let config = Config::default();
    let model = stock_geometry_model();

    let warnings = model.finalize_warnings(&config);
    assert!(media_height_warning(&warnings).is_none());
}
