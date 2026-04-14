//! Geometry-aware lint rules for css-check

#[path = "main_css_check_geometry/model.rs"]
mod model;
#[path = "main_css_check_geometry/parse.rs"]
mod parse;
#[path = "main_css_check_geometry/stock.rs"]
mod stock;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::Config;

use self::model::GeometryModel;
pub(super) use self::parse::{
    can_model_horizontal_size_value, collect_custom_property_scopes, CssCustomPropertyScopes,
};
use self::parse::collect_geometry_from_contents;
use super::main_css_check_files::format_display_path;
use super::main_css_check_report::{CssCheckCategory, CssCheckDiagnostic};
use super::main_css_check_runtime::display_config_path;

pub(super) fn lint_geometry_css_files(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
) -> Result<Vec<CssCheckDiagnostic>> {
    let config_path = Config::default_config_path()?;
    if !config_path.exists() {
        // Geometry lint needs the live config because panel width still matters
        return Ok(Vec::new());
    }

    let config = Config::load_from_path(&config_path)?;
    let config_display = display_config_path(config_dir, display_root, &config_path);

    // One shared model lets multiple files build one layout estimate
    let mut model = GeometryModel::default();
    let mut diagnostics = Vec::new();
    for path in files {
        let display_path = format_display_path(config_dir, display_root, path);
        // Raw text is needed here so rules from different files can be merged first
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read css file {}", path.display()))?;
        let report = collect_geometry_from_contents(&contents, &mut model);
        for warning in report {
            diagnostics.push(CssCheckDiagnostic::warning(
                CssCheckCategory::Geometry,
                "GEOM001",
                display_path.clone(),
                warning,
            ));
        }
    }

    // Final warnings need the full css picture plus the live config
    for warning in model.finalize_warnings(&config) {
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Geometry,
            "GEOM002",
            config_display.clone(),
            warning,
        ));
    }

    Ok(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::{collect_geometry_from_contents, GeometryModel};
    use unixnotis_core::{build_modern_theme_custom_properties, gtk_css_features_for_version};
    use unixnotis_core::{
        Config, MediaLayout, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS,
        DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS,
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
}
