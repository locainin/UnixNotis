//! Geometry-aware lint rules for css-check

#[path = "main_css_check_geometry/model.rs"]
mod model;
#[path = "main_css_check_geometry/parse.rs"]
mod parse;
#[path = "main_css_check_geometry/stock.rs"]
mod stock;
#[cfg(test)]
#[path = "main_css_check_geometry/tests.rs"]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::Config;

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

    // Theme tokens can be declared in one file and consumed in another
    let shared_custom_properties = collect_custom_property_scopes(
        &file_contents
            .iter()
            .map(|(_, contents)| contents.as_str())
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
