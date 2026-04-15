//! Geometry lint tests
//!
//! Keeps the root geometry entry file focused on runtime behavior

use super::parse::{
    collect_geometry_from_contents, collect_geometry_from_contents_with_properties,
};
use super::{collect_custom_property_scopes, GeometryModel};
use unixnotis_core::{build_modern_theme_custom_properties, gtk_css_features_for_version};
use unixnotis_core::{
    Config, MediaLayout, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

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
fn stock_theme_geometry_is_quiet() {
    let config = Config::default();
    let mut model = GeometryModel::default();
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

#[test]
fn geometry_can_follow_custom_property_lengths() {
    let mut config = Config::default();
    config.panel.width = 320;
    let css = r#"
        :root {
            --toggle-width: 104px;
            --toggle-pad: 12px;
        }

        .unixnotis-panel { padding: 16px; }
        .unixnotis-toggle {
            min-width: var(--toggle-width);
            padding: 10px var(--toggle-pad);
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty(), "{file_warnings:?}");

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
}

#[test]
fn geometry_can_follow_simple_calc_lengths() {
    let mut config = Config::default();
    config.panel.width = 320;
    let css = r#"
        .unixnotis-panel { padding: calc(8px + 8px); }
        .unixnotis-toggle {
            min-width: calc(96px + 8px);
            padding: 10px calc(8px + 4px);
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty(), "{file_warnings:?}");

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
}

#[test]
fn geometry_can_follow_selector_scoped_custom_properties() {
    let mut config = Config::default();
    config.panel.width = 320;
    let css = r#"
        :root { --toggle-pad: 10px; }
        .unixnotis-panel { padding: 16px; }
        .unixnotis-toggle {
            --toggle-min: calc(52px * 2);
            min-width: var(--toggle-min);
            padding: 10px calc(var(--toggle-pad) + 2px);
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty(), "{file_warnings:?}");

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
}

#[test]
fn geometry_can_follow_generated_modern_theme_tokens() {
    let mut config = Config::default();
    config.panel.width = 320;
    let css = format!(
        "{}\n.unixnotis-panel {{ padding: var(--unixnotis-panel-padding); }}\n.unixnotis-toggle {{ min-width: var(--unixnotis-toggle-min-width); padding: 10px calc(var(--unixnotis-panel-action-gap) * 2); border: 1px solid red; }}",
        build_modern_theme_custom_properties(
            &Config::default().theme,
            gtk_css_features_for_version(4, 16),
        )
    );

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(&css, &mut model);
    assert!(file_warnings.is_empty(), "{file_warnings:?}");

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
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
fn geometry_can_follow_cross_file_root_custom_properties() {
    let mut config = Config::default();
    config.panel.width = 320;

    let base_css = r#"
        :root {
            --toggle-width: 104px;
            --toggle-pad: 12px;
        }
    "#;
    let widgets_css = r#"
        .unixnotis-panel { padding: 16px; }
        .unixnotis-toggle {
            min-width: var(--toggle-width);
            padding: 10px var(--toggle-pad);
            border: 1px solid red;
        }
    "#;

    let shared_properties = collect_custom_property_scopes(&format!("{base_css}\n{widgets_css}"));
    let mut model = GeometryModel::default();
    let base_warnings =
        collect_geometry_from_contents_with_properties(base_css, &shared_properties, &mut model);
    let widget_warnings =
        collect_geometry_from_contents_with_properties(widgets_css, &shared_properties, &mut model);
    assert!(base_warnings.is_empty(), "{base_warnings:?}");
    assert!(widget_warnings.is_empty(), "{widget_warnings:?}");

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
}

#[test]
fn geometry_can_follow_compare_length_functions() {
    let mut config = Config::default();
    config.panel.width = 320;
    let css = r#"
        .unixnotis-panel { padding: clamp(8px, 12px, 16px); }
        .unixnotis-toggle {
            min-width: max(96px, 104px);
            padding: 10px min(12px, 16px);
            border: 1px solid red;
        }
    "#;

    let mut model = GeometryModel::default();
    let file_warnings = collect_geometry_from_contents(css, &mut model);
    assert!(file_warnings.is_empty(), "{file_warnings:?}");

    let warnings = model.finalize_warnings(&config);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("toggle grid")));
}
