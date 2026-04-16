//! Geometry-aware lint rules for css-check

#[path = "main_css_check_geometry/model.rs"]
mod model;
#[path = "main_css_check_geometry/parse.rs"]
mod parse;
#[path = "main_css_check_geometry/stock/mod.rs"]
mod stock;
#[cfg(test)]
#[path = "main_css_check_geometry/tests/mod.rs"]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::{build_modern_theme_custom_properties, gtk_css_features_for_version, Config};

use self::model::GeometryModel;
use self::parse::collect_geometry_from_contents_with_properties;
pub(super) use self::parse::{
    can_model_horizontal_size_value, collect_custom_property_scopes, CssCustomPropertyScopes,
};
use super::main_css_check_files::format_display_path;
use super::main_css_check_report::{CssCheckCategory, CssCheckDiagnostic};
use super::main_css_check_runtime::display_config_path;

pub(super) fn lint_geometry_css_files(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
) -> Result<Vec<CssCheckDiagnostic>> {
    // Geometry warnings depend on the live panel width, so there is nothing useful to do
    // until the real config can be loaded
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
    let mut file_contents = Vec::new();
    for path in files {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read css file {}", path.display()))?;
        file_contents.push((path, contents));
    }

    // Runtime theme overrides inject modern tokens that may never appear in the css files
    let generated_tokens =
        build_modern_theme_custom_properties(&config.theme, gtk_css_features_for_version(4, 16));

    // Runtime tokens are stitched in before the file scan so token-only themes do not hide
    // width pressure from the checker
    // Theme tokens can be declared in one file and consumed in another
    let shared_custom_properties = collect_custom_property_scopes(
        &std::iter::once(generated_tokens.as_str())
            .chain(file_contents.iter().map(|(_, contents)| contents.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    for (path, contents) in file_contents {
        let display_path = format_display_path(config_dir, display_root, path);
        // Raw text is still parsed per file, but the token view now spans the whole theme set
        let report = collect_geometry_from_contents_with_properties(
            &contents,
            &shared_custom_properties,
            &mut model,
        );
        for warning in report {
            diagnostics.push(CssCheckDiagnostic::warning(
                CssCheckCategory::Geometry,
                display_path.clone(),
                warning,
            ));
        }
    }

    // Final warnings need the full css picture plus the live config
    for warning in model.finalize_warnings(&config) {
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Geometry,
            config_display.clone(),
            warning,
        ));
    }

    Ok(diagnostics)
}
