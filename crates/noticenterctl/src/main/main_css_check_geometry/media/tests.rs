use unixnotis_core::{Config, MediaControlsPosition, MediaLayout, MediaNavigationPosition};

use super::super::super::parse::collect_geometry_from_contents;
use super::super::GeometryModel;
use super::shell::ModeledMediaShell;
use super::width::media_text_reserve_px;

#[test]
fn larger_navigation_gap_increases_inline_reserve_and_pressure() {
    // This keeps the reserve math aligned with the runtime inline shell spacing rules
    let mut tight_gap = Config::default();
    tight_gap.panel.width = 380;
    tight_gap.media.layout = MediaLayout::Inline;
    tight_gap.media.controls_position = MediaControlsPosition::Inline;
    tight_gap.media.navigation_position = MediaNavigationPosition::WithControls;
    tight_gap.media.show_art = false;
    tight_gap.media.navigation_spacing_px = 6;

    let mut wide_gap = tight_gap.clone();
    wide_gap.media.navigation_spacing_px = 42;

    let tight_shell = ModeledMediaShell::from_config(&tight_gap.media);
    let wide_shell = ModeledMediaShell::from_config(&wide_gap.media);
    assert!(media_text_reserve_px(wide_shell) > media_text_reserve_px(tight_shell));

    let css = r#"
        .unixnotis-panel { padding: 12px; }
        .unixnotis-media-card { padding: 8px 10px; border: 1px solid red; }
        .unixnotis-media-body { padding: 0 4px; }
        .unixnotis-media-text { padding: 0 4px; }
        .unixnotis-media-nav { min-width: 20px; }
        .unixnotis-media-button { min-width: 26px; }
    "#;
    let mut model = GeometryModel::default();
    // Parsing has to stay clean before the width comparison means anything
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty());
    // Until the text lane hits its configured floor, wider nav gaps trade text room for spacing
    assert!(model.media_width_warning(&tight_gap).is_none());
    assert!(model.media_width_warning(&wide_gap).is_none());
}
