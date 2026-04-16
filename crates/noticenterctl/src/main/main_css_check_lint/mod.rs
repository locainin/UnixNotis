//! Lint rules for css-check

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use unixnotis_core::{build_modern_theme_custom_properties, gtk_css_features_for_version, Config};

use super::main_css_check_files::format_display_path;
use super::main_css_check_geometry::{collect_custom_property_scopes, CssCustomPropertyScopes};
use super::main_css_check_report::{CssCheckCategory, CssCheckDiagnostic};

#[path = "scan.rs"]
mod scan;
#[path = "values.rs"]
mod values;

#[derive(Debug)]
pub(super) struct CssCheckLintFinding {
    // Lint can point at the source when the scanner has a stable offset
    pub(super) line: Option<usize>,
    pub(super) column: Option<usize>,
    pub(super) message: String,
}

pub(super) fn lint_css_files(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
) -> Result<Vec<CssCheckDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut file_contents = Vec::new();
    for path in files {
        // Lint still reads every file directly because GTK parser callbacks do not cover
        // duplicate selectors, duplicate properties, or geometry-aware value hints
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read css file {}", path.display()))?;
        file_contents.push((path, contents));
    }

    // Modern tokens are generated at runtime, so lint needs the same token view even when
    // the physical css files only contain the consuming var() rules
    let generated_tokens = generated_theme_token_css().unwrap_or_default();
    let combined_custom_properties = collect_custom_property_scopes(
        &std::iter::once(generated_tokens.as_str())
            .chain(file_contents.iter().map(|(_, contents)| contents.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    for (path, contents) in file_contents {
        let display_path = format_display_path(config_dir, display_root, path);
        // GTK only reports parser failures, so lint reads the raw file too
        let report = lint_css_contents_with_properties(&contents, &combined_custom_properties);
        for finding in report {
            diagnostics.push(CssCheckDiagnostic::warning_at(
                CssCheckCategory::Lint,
                display_path.clone(),
                finding.line,
                finding.column,
                finding.message,
            ));
        }
    }
    Ok(diagnostics)
}

#[cfg(test)]
pub(super) fn lint_css_contents(contents: &str) -> Vec<CssCheckLintFinding> {
    lint_css_contents_with_properties(contents, &collect_custom_property_scopes(contents))
}

pub(super) fn lint_css_contents_with_properties(
    contents: &str,
    custom_properties: &CssCustomPropertyScopes,
) -> Vec<CssCheckLintFinding> {
    // Tests and file-based linting share one implementation so rule behavior does not drift
    scan::lint_css_contents_with_properties(contents, custom_properties)
}

fn generated_theme_token_css() -> Option<String> {
    // Runtime tokens are only available when the live config loads cleanly
    // Lint falls back to file-only analysis if the config is missing or broken
    let config_path = Config::default_config_path().ok()?;
    let config = Config::load_from_path(&config_path).ok()?;
    Some(build_modern_theme_custom_properties(
        &config.theme,
        gtk_css_features_for_version(4, 16),
    ))
}
